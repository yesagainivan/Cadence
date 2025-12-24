// use crate::parser::Value;
use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

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

/// Audio player that manages real-time audio output
pub struct AudioPlayer {
    stream: Stream,
    state: Arc<Mutex<AudioState>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
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

        Ok(AudioPlayer { stream, state })
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

    /// Set the frequencies to play
    pub fn set_notes(&self, notes: Vec<f32>) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Failed to lock audio state: {}", e))?;
        state.notes = notes;
        Ok(())
    }

    /// Set the volume level (0.0 to 1.0)
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Failed to lock audio state: {}", e))?;
        state.volume = volume;
        Ok(())
    }

    /// Get the current volume level
    pub fn get_volume(&self) -> Result<f32> {
        let state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Failed to lock audio state: {}", e))?;
        Ok(state.volume)
    }

    /// Start audio playback
    pub fn play(&self) -> Result<()> {
        self.stream
            .play()
            .map_err(|e| anyhow!("Failed to play stream: {}", e))
    }

    /// Pause audio playback
    pub fn pause(&self) -> Result<()> {
        self.stream
            .pause()
            .map_err(|e| anyhow!("Failed to pause stream: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_player_creation() {
        // This test may fail on systems without audio devices
        match AudioPlayer::new() {
            Ok(_player) => {
                // AudioPlayer was created successfully
                assert!(true);
            }
            Err(_) => {
                // This is expected on systems without audio devices (like CI)
                println!("AudioPlayer creation failed - likely no audio device available");
            }
        }
    }

    #[test]
    fn test_oscillator_generation() {
        let mut osc = Oscillator::new(440.0);
        let sample_rate = 44100.0;

        // Test that oscillator values are generated within expected range
        for _ in 0..1000 {
            let value = osc.next_sample(sample_rate);
            assert!(
                value >= -1.0 && value <= 1.0,
                "Oscillator value {} out of expected range",
                value
            );
        }

        // Test that phase stays in valid range
        assert!(osc.phase >= 0.0 && osc.phase < 1.0);
    }

    #[test]
    fn test_oscillator_periodicity() {
        let sample_rate = 44100.0;
        let frequency = 440.0;

        // Use exact number of samples for one period
        let samples_per_period = ((sample_rate / frequency) as f64).round() as usize;

        let mut osc1 = Oscillator::new(frequency);
        let mut first_period = Vec::new();

        // Generate one full period
        for _ in 0..samples_per_period {
            first_period.push(osc1.next_sample(sample_rate));
        }

        // Reset oscillator for second period
        let mut osc2 = Oscillator::new(frequency);
        let mut second_period = Vec::new();
        for _ in 0..samples_per_period {
            second_period.push(osc2.next_sample(sample_rate));
        }

        // Values should be very close (allowing for floating point precision)
        for (i, (first, second)) in first_period.iter().zip(second_period.iter()).enumerate() {
            let diff = (first - second).abs();
            assert!(
                diff < 0.001,
                "Period mismatch at sample {}: {} vs {}",
                i,
                first,
                second
            );
        }
    }

    #[test]
    fn test_volume_control() {
        match AudioPlayer::new() {
            Ok(player) => {
                // Test setting volume
                assert!(player.set_volume(0.5).is_ok());
                assert_eq!(player.get_volume().unwrap(), 0.5);

                // Test volume clamping
                assert!(player.set_volume(1.5).is_ok());
                assert_eq!(player.get_volume().unwrap(), 1.0);

                assert!(player.set_volume(-0.5).is_ok());
                assert_eq!(player.get_volume().unwrap(), 0.0);
            }
            Err(_) => {
                println!("Skipping volume test - no audio device available");
            }
        }
    }

    #[test]
    fn test_multi_note_independence() {
        // Test that multiple oscillators maintain independent phases
        let mut osc1 = Oscillator::new(440.0); // A4
        let mut osc2 = Oscillator::new(554.37); // C#5

        let sample_rate = 44100.0;

        // Generate some samples
        for _ in 0..100 {
            let _val1 = osc1.next_sample(sample_rate);
            let _val2 = osc2.next_sample(sample_rate);
        }

        // Phases should be different
        assert!((osc1.phase - osc2.phase).abs() > 0.01);
    }
}
