use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::oscillator::{EnvelopedOscillator, Waveform};

/// State for a single audio track
#[derive(Clone, Debug)]
pub struct TrackState {
    /// List of frequencies to play (in Hz)
    pub notes: Vec<f32>,
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
    /// Whether this specific track is playing (not currently used for master pause)
    pub is_playing: bool,
    /// Optional custom ADSR envelope (attack, decay, sustain, release)
    pub envelope: Option<(f32, f32, f32, f32)>,
    /// Waveform type for this track
    pub waveform: Waveform,
}

impl Default for TrackState {
    fn default() -> Self {
        TrackState {
            notes: Vec::new(),
            volume: 1.0, // Individual tracks default to full volume (master mixer handles global)
            is_playing: true,
            envelope: None,                // Use default ADSR
            waveform: Waveform::default(), // Sine by default
        }
    }
}

/// Shared audio state protected by Mutex for thread-safe access
#[derive(Clone)]
pub struct AudioState {
    /// Map of track ID to track state
    pub tracks: HashMap<usize, TrackState>,
    /// Master volume level (0.0 to 1.0)
    pub volume: f32,
    /// Master playback status
    pub is_playing: bool,
}

impl Default for AudioState {
    fn default() -> Self {
        AudioState {
            tracks: HashMap::new(),
            volume: 0.2,       // Default to 20% master volume
            is_playing: false, // Start paused
        }
    }
}

// EnvelopedOscillator is now in oscillator.rs

/// Commands that can be sent to the audio player thread
#[derive(Debug, Clone)]
pub enum AudioPlayerCommand {
    SetTrackNotes(usize, Vec<f32>),
    SetTrackVolume(usize, f32),
    SetTrackEnvelope(usize, Option<(f32, f32, f32, f32)>),
    SetTrackWaveform(usize, Waveform),
    SetMasterVolume(f32),
    Play,
    Pause,
    Quit,
}

/// Internal audio player that owns the cpal::Stream
struct AudioPlayerInternal {
    stream: Stream,
    state: Arc<Mutex<AudioState>>,
}

impl AudioPlayerInternal {
    fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No output device available"))?;
        let config = device.default_output_config()?;

        let sample_format = config.sample_format();
        let config: StreamConfig = config.into();

        let state = Arc::new(Mutex::new(AudioState::default()));
        let stream = match sample_format {
            SampleFormat::F32 => Self::build_stream::<f32>(&device, &config, state.clone())?,
            SampleFormat::I16 => Self::build_stream::<i16>(&device, &config, state.clone())?,
            SampleFormat::U16 => Self::build_stream::<u16>(&device, &config, state.clone())?,
            _ => return Err(anyhow!("Unsupported sample format: {:?}", sample_format)),
        };

        Ok(AudioPlayerInternal { stream, state })
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &StreamConfig,
        state: Arc<Mutex<AudioState>>,
    ) -> Result<Stream>
    where
        T: Sample + SizedSample + Send + 'static + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0 as f32;

        let mut oscillators: Vec<EnvelopedOscillator> = Vec::new();
        // Track current frequencies per track: Map<TrackId, Vec<Freq>>
        let mut track_frequencies: HashMap<usize, Vec<f32>> = HashMap::new();
        // Track current waveforms per track to detect changes
        let mut track_waveforms: HashMap<usize, Waveform> = HashMap::new();

        let mut master_amplitude: f32 = 0.0;
        // Master fade rate should match or exceed ADSR release time (200ms default)
        // to allow envelopes to complete their release phase gracefully
        let master_fade_rate = 1.0 / (0.25 * sample_rate); // 250ms for smooth master fade

        let err_fn = |err| eprintln!("Audio stream error: {:?}", err);

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let state = match state.lock() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("Failed to lock audio state: {}", e);
                            for sample in data.iter_mut() {
                                *sample = T::from_sample(0.0);
                            }
                            return;
                        }
                    };

                    let master_volume = state.volume;
                    let is_playing = state.is_playing;

                    // 1. Sync oscillators with state
                    // Check for changes in each track
                    for (track_id, track_state) in &state.tracks {
                        let current = track_frequencies.entry(*track_id).or_default();
                        let current_waveform = track_waveforms
                            .entry(*track_id)
                            .or_insert(Waveform::default());

                        // If notes changed OR waveform changed for this track
                        let notes_changed = current.len() != track_state.notes.len()
                            || current
                                .iter()
                                .zip(track_state.notes.iter())
                                .any(|(a, b)| (a - b).abs() > 0.01);
                        let waveform_changed = *current_waveform != track_state.waveform;

                        if notes_changed || waveform_changed {
                            // Fade out old oscillators for this track
                            for osc in oscillators.iter_mut().filter(|o| o.track_id == *track_id) {
                                osc.start_fade_out();
                            }

                            // Add new oscillators with track's envelope settings
                            for &freq in &track_state.notes {
                                oscillators.push(EnvelopedOscillator::with_envelope(
                                    freq,
                                    sample_rate,
                                    *track_id,
                                    track_state.envelope,
                                    track_state.waveform,
                                ));
                            }

                            // Update cache
                            *current = track_state.notes.clone();
                            *current_waveform = track_state.waveform;
                        }
                    }

                    // 2. Generate audio
                    for frame in data.chunks_mut(channels) {
                        if is_playing {
                            master_amplitude = (master_amplitude + master_fade_rate).min(1.0);
                        } else {
                            master_amplitude = (master_amplitude - master_fade_rate).max(0.0);
                        }

                        let mut mixed_value = 0.0;
                        let mut active_count = 0;

                        // Sum all oscillators
                        // Note: We need to consider track volume here too
                        for oscillator in oscillators.iter_mut() {
                            let track_vol = state
                                .tracks
                                .get(&oscillator.track_id)
                                .map(|t| t.volume)
                                .unwrap_or(1.0);

                            let sample = oscillator.next_sample();
                            if sample.abs() > 0.0001 {
                                mixed_value += sample * track_vol;
                                active_count += 1;
                            }
                        }

                        // Normalize?
                        // Simple normalization strategy:
                        // Use tanh or soft clipping to handle summing,
                        // or just scale down by a constant factor assuming max polyphony.
                        // Let's try soft clipping for now (tanh-like behavior) to prevent harsh digital clipping
                        // but allow loudness.
                        // Actually, let's stick to safe scaling for now:
                        if active_count > 0 {
                            // Rough heuristic: assume typical play is 3-6 notes.
                            // Don't divide linearly by count or it gets too quiet with many notes.
                            // Divide by sqrt(count) for better power preservation, or just a fixed headroom.
                            mixed_value *= 0.3;
                        }

                        // Hard limiter just in case
                        mixed_value = mixed_value.clamp(-1.0, 1.0);

                        mixed_value *= master_volume * master_amplitude;

                        let value: T = cpal::Sample::from_sample(mixed_value);
                        for sample in frame.iter_mut() {
                            *sample = value;
                        }
                    }

                    oscillators.retain(|osc| !osc.is_finished());
                },
                err_fn,
                None,
            )
            .map_err(|e| anyhow!("Failed to build output stream: {}", e))?;

        Ok(stream)
    }

    fn set_track_notes(&mut self, track_id: usize, notes: Vec<f32>) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let track = state.tracks.entry(track_id).or_default();
        track.notes = notes;
        Ok(())
    }

    fn set_track_volume(&mut self, track_id: usize, volume: f32) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let track = state.tracks.entry(track_id).or_default();
        track.volume = volume.clamp(0.0, 1.0);
        Ok(())
    }

    fn set_track_envelope(
        &mut self,
        track_id: usize,
        envelope: Option<(f32, f32, f32, f32)>,
    ) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let track = state.tracks.entry(track_id).or_default();
        track.envelope = envelope;
        Ok(())
    }

    fn set_track_waveform(&mut self, track_id: usize, waveform: Waveform) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let track = state.tracks.entry(track_id).or_default();
        track.waveform = waveform;
        Ok(())
    }

    fn set_master_volume(&mut self, volume: f32) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        state.volume = volume.clamp(0.0, 1.0);
        Ok(())
    }

    fn play(&mut self) -> Result<()> {
        self.stream
            .play()
            .map_err(|e| anyhow!("Failed to play: {}", e))?;
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        state.is_playing = true;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        state.is_playing = false;
        Ok(())
    }
}

/// Thread-safe handle to the audio player
/// Uses internal channels to communicate with the audio thread
pub struct AudioPlayerHandle {
    command_tx: Sender<AudioPlayerCommand>,
    _thread: JoinHandle<()>,
}

impl AudioPlayerHandle {
    /// Create a new audio player handle
    /// Spawns a dedicated audio thread that owns the cpal::Stream
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel();

        let thread = thread::spawn(move || {
            // Create audio player in this thread
            let mut player = match AudioPlayerInternal::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to create audio player: {}", e);
                    return;
                }
            };

            // Process commands until quit
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioPlayerCommand::SetTrackNotes(track_id, notes) => {
                        if let Err(e) = player.set_track_notes(track_id, notes) {
                            eprintln!("Failed to set track notes: {}", e);
                        }
                    }
                    AudioPlayerCommand::SetTrackVolume(track_id, vol) => {
                        if let Err(e) = player.set_track_volume(track_id, vol) {
                            eprintln!("Failed to set track volume: {}", e);
                        }
                    }
                    AudioPlayerCommand::SetTrackEnvelope(track_id, envelope) => {
                        if let Err(e) = player.set_track_envelope(track_id, envelope) {
                            eprintln!("Failed to set track envelope: {}", e);
                        }
                    }
                    AudioPlayerCommand::SetTrackWaveform(track_id, waveform) => {
                        if let Err(e) = player.set_track_waveform(track_id, waveform) {
                            eprintln!("Failed to set track waveform: {}", e);
                        }
                    }
                    AudioPlayerCommand::SetMasterVolume(vol) => {
                        if let Err(e) = player.set_master_volume(vol) {
                            eprintln!("Failed to set master volume: {}", e);
                        }
                    }
                    AudioPlayerCommand::Play => {
                        if let Err(e) = player.play() {
                            eprintln!("Failed to play: {}", e);
                        }
                    }
                    AudioPlayerCommand::Pause => {
                        if let Err(e) = player.pause() {
                            eprintln!("Failed to pause: {}", e);
                        }
                    }
                    AudioPlayerCommand::Quit => break,
                }
            }
        });

        Ok(AudioPlayerHandle {
            command_tx: tx,
            _thread: thread,
        })
    }

    /// Set the frequencies to play for a specific track
    pub fn set_track_notes(&self, track_id: usize, notes: Vec<f32>) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetTrackNotes(track_id, notes))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Set the frequencies to play (default track 1)
    pub fn set_notes(&self, notes: Vec<f32>) -> Result<()> {
        self.set_track_notes(1, notes)
    }

    /// Set the volume level for a specific track
    pub fn set_track_volume(&self, track_id: usize, volume: f32) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetTrackVolume(track_id, volume))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Set the ADSR envelope for a specific track
    pub fn set_track_envelope(
        &self,
        track_id: usize,
        envelope: Option<(f32, f32, f32, f32)>,
    ) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetTrackEnvelope(track_id, envelope))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Set the waveform for a specific track
    pub fn set_track_waveform(&self, track_id: usize, waveform: Waveform) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetTrackWaveform(track_id, waveform))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Set master volume
    pub fn set_master_volume(&self, volume: f32) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetMasterVolume(volume))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Set the volume level (global/master for backward compatibility)
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.set_master_volume(volume)
    }

    /// Start audio playback
    pub fn play(&self) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::Play)
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Pause audio playback
    pub fn pause(&self) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::Pause)
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }
}

impl Drop for AudioPlayerHandle {
    fn drop(&mut self) {
        // Send quit command when handle is dropped
        let _ = self.command_tx.send(AudioPlayerCommand::Quit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_player_handle_creation() {
        match AudioPlayerHandle::new() {
            Ok(_handle) => {
                // Successfully created
                assert!(true);
            }
            Err(_) => {
                println!("AudioPlayer creation failed - no audio device available");
            }
        }
    }

    #[test]
    fn test_oscillator_generation() {
        let sample_rate = 44100.0;
        let mut osc = EnvelopedOscillator::new(440.0, sample_rate, 1);

        for _ in 0..1000 {
            let value = osc.next_sample();
            assert!(
                value >= -1.0 && value <= 1.0,
                "Oscillator value {} out of expected range",
                value
            );
        }
        // Phase check removed - field is now private in oscillator module
    }

    #[test]
    fn test_commands() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                assert!(handle.set_notes(vec![440.0, 554.37]).is_ok());
                assert!(handle.set_track_notes(2, vec![330.0]).is_ok());
                assert!(handle.set_volume(0.5).is_ok());
                assert!(handle.play().is_ok());
                std::thread::sleep(std::time::Duration::from_millis(100));
                assert!(handle.pause().is_ok());
            }
            Err(_) => {
                println!("Skipping command test - no audio device");
            }
        }
    }
}
