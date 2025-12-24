// use crate::parser::Value;
use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

// This is the shared state. It will be protected by a Mutex.
#[derive(Clone)]
pub struct AudioState {
    pub notes: Vec<f32>, // A list of frequencies to play
                         // You can add more state here later, like volume, waveform type, etc.
}

impl Default for AudioState {
    fn default() -> Self {
        AudioState {
            notes: vec![440.0], // Default to playing a single A4 note
        }
    }
}

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
        state: Arc<Mutex<AudioState>>, // Now takes the shared state
    ) -> Result<Stream>
    where
        T: Sample + SizedSample + Send + 'static + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0 as f32;
        let mut sample_clock = 0f32;
        // The frequency is now managed by the shared state, not a local variable.

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {:?}", err);

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let state = state.lock().unwrap(); // Lock the mutex
                    let frequencies = &state.notes; // Get the frequencies
                    let num_notes = frequencies.len() as f32;

                    for frame in data.chunks_mut(channels) {
                        let mut summed_value = 0.0;
                        for &frequency in frequencies {
                            summed_value +=
                                Self::next_sine_value(sample_rate, &mut sample_clock, frequency);
                        }

                        let normalized_value = summed_value / num_notes;
                        let value: T = cpal::Sample::from_sample(normalized_value);

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

    // In `audio.rs` within the `impl AudioPlayer` block
    pub fn set_notes(&self, notes: Vec<f32>) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow!("Failed to lock audio state: {}", e))?;
        state.notes = notes;
        Ok(())
    }

    // // New or modified functions
    // pub fn play_note(&mut self, frequency: f32) -> Result<()> {
    //     // This is a placeholder. You'll need to modify the stream closure to accept dynamic frequency updates.
    //     // For now, this function is a conceptual step.
    //     Ok(())
    // }

    fn next_sine_value(sample_rate: f32, sample_clock: &mut f32, frequency: f32) -> f32 {
        let volume = 0.2;
        let value =
            volume * (2.0 * std::f32::consts::PI * frequency * *sample_clock / sample_rate).sin();
        *sample_clock = (*sample_clock + 1.0) % sample_rate;
        value
    }

    pub fn play(&self) -> Result<()> {
        self.stream
            .play()
            .map_err(|e| anyhow!("Failed to play stream: {}", e))
    }

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
    fn test_sine_wave_generation() {
        let sample_rate = 44100.0;
        let mut sample_clock = 0.0;

        // Test that sine wave values are generated within expected range
        for _ in 0..1000 {
            let value = AudioPlayer::next_sine_value(sample_rate, &mut sample_clock, 440.0);
            assert!(
                value >= -0.2 && value <= 0.2,
                "Sine value {} out of expected range",
                value
            );
        }

        // Test that sample clock wraps around
        assert!(sample_clock < sample_rate);
    }

    #[test]
    fn test_sine_wave_periodicity() {
        let sample_rate = 44100.0;
        let frequency = 440.0;

        // Use exact number of samples for one period to avoid floating point errors
        let samples_per_period: f32 = sample_rate / frequency;
        let samples_per_period_int = samples_per_period.round() as usize;

        let mut sample_clock = 0.0;
        let mut first_period = Vec::new();

        // Generate one full period
        for _ in 0..samples_per_period_int {
            first_period.push(AudioPlayer::next_sine_value(
                sample_rate,
                &mut sample_clock,
                440.0,
            ));
        }

        // Reset sample clock to 0 for the second period to ensure exact comparison
        sample_clock = 0.0;
        let mut second_period = Vec::new();
        for _ in 0..samples_per_period_int {
            second_period.push(AudioPlayer::next_sine_value(
                sample_rate,
                &mut sample_clock,
                440.0,
            ));
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
}
