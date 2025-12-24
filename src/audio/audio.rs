use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Shared audio state protected by Mutex for thread-safe access
#[derive(Clone)]
pub struct AudioState {
    /// List of frequencies to play (in Hz)
    pub notes: Vec<f32>,
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
}

impl Default for AudioState {
    fn default() -> Self {
        AudioState {
            notes: vec![440.0], // Default to A4
            volume: 0.2,        // Default to 20% volume
        }
    }
}

/// Per-note oscillator state for independent phase tracking
struct Oscillator {
    frequency: f32,
    phase: f32,
}

impl Oscillator {
    fn new(frequency: f32) -> Self {
        Self {
            frequency,
            phase: 0.0,
        }
    }

    /// Generate next sample value using sine wave synthesis
    fn next_sample(&mut self, sample_rate: f32) -> f32 {
        let value = (2.0 * std::f32::consts::PI * self.phase).sin();

        // Update phase for next sample
        self.phase += self.frequency / sample_rate;

        // Keep phase in [0, 1) range to prevent floating point drift
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        value
    }
}

/// Commands that can be sent to the audio player thread
#[derive(Debug, Clone)]
pub enum AudioPlayerCommand {
    SetNotes(Vec<f32>),
    SetVolume(f32),
    Play,
    Pause,
    Quit,
}

/// Internal audio player that owns the cpal::Stream (stays in audio thread)
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

        // Per-note oscillators for independent phase tracking
        let mut oscillators: Vec<Oscillator> = Vec::new();

        let err_fn = |err| eprintln!("Audio stream error: {:?}", err);

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    // Try to lock the state, fallback to silence on error
                    let state = match state.lock() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("Failed to lock audio state: {}", e);
                            // Output silence on error
                            for sample in data.iter_mut() {
                                *sample = T::from_sample(0.0);
                            }
                            return;
                        }
                    };

                    let frequencies = &state.notes;
                    let volume = state.volume;

                    // Update oscillators if frequencies changed
                    if oscillators.len() != frequencies.len()
                        || oscillators
                            .iter()
                            .zip(frequencies.iter())
                            .any(|(osc, &freq)| (osc.frequency - freq).abs() > 0.01)
                    {
                        oscillators = frequencies
                            .iter()
                            .map(|&freq| Oscillator::new(freq))
                            .collect();
                    }

                    // Generate audio samples
                    for frame in data.chunks_mut(channels) {
                        let mut mixed_value = 0.0;

                        // Mix all oscillators together
                        for oscillator in oscillators.iter_mut() {
                            mixed_value += oscillator.next_sample(sample_rate);
                        }

                        // Normalize by number of notes to prevent clipping
                        if !oscillators.is_empty() {
                            mixed_value /= oscillators.len() as f32;
                        }

                        // Apply volume
                        mixed_value *= volume;

                        // Convert to target sample format
                        let value: T = cpal::Sample::from_sample(mixed_value);

                        // Write to all channels (mono to stereo/multi-channel)
                        for sample in frame.iter_mut() {
                            *sample = value;
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| anyhow!("Failed to build output stream: {}", e))?;

        Ok(stream)
    }

    fn set_notes(&mut self, notes: Vec<f32>) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Failed to lock audio state: {}", e))?;
        state.notes = notes;
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Failed to lock audio state: {}", e))?;
        state.volume = volume;
        Ok(())
    }

    fn play(&self) -> Result<()> {
        self.stream
            .play()
            .map_err(|e| anyhow!("Failed to play stream: {}", e))
    }

    fn pause(&self) -> Result<()> {
        self.stream
            .pause()
            .map_err(|e| anyhow!("Failed to pause stream: {}", e))
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
                    AudioPlayerCommand::SetNotes(notes) => {
                        if let Err(e) = player.set_notes(notes) {
                            eprintln!("Failed to set notes: {}", e);
                        }
                    }
                    AudioPlayerCommand::SetVolume(vol) => {
                        if let Err(e) = player.set_volume(vol) {
                            eprintln!("Failed to set volume: {}", e);
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

    /// Set the frequencies to play
    pub fn set_notes(&self, notes: Vec<f32>) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetNotes(notes))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }

    /// Set the volume level (0.0 to 1.0)
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.command_tx
            .send(AudioPlayerCommand::SetVolume(volume))
            .map_err(|e| anyhow!("Failed to send command: {}", e))
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
        let mut osc = Oscillator::new(440.0);
        let sample_rate = 44100.0;

        for _ in 0..1000 {
            let value = osc.next_sample(sample_rate);
            assert!(
                value >= -1.0 && value <= 1.0,
                "Oscillator value {} out of expected range",
                value
            );
        }

        assert!(osc.phase >= 0.0 && osc.phase < 1.0);
    }

    #[test]
    fn test_commands() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                assert!(handle.set_notes(vec![440.0, 554.37]).is_ok());
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
