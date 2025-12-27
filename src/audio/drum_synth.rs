//! Drum synthesizer module
//!
//! Provides `DrumOscillator` for synthesized percussion sounds including
//! kick, snare, hi-hat, clap, and other drum machine sounds.

use cadence_core::types::DrumSound;
use std::f32::consts::PI;

/// Simple xorshift PRNG for noise generation (no external dependency needed)
struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    fn new(seed: u32) -> Self {
        Self { state: seed.max(1) } // Ensure non-zero
    }

    /// Generate next random u32 using xorshift algorithm
    fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    /// Generate random f32 in range [0.0, 1.0)
    fn random_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }

    /// Generate random f32 in range [-1.0, 1.0) (for audio noise)
    fn noise(&mut self) -> f32 {
        self.random_f32() * 2.0 - 1.0
    }
}

/// A one-shot drum oscillator that synthesizes percussion sounds
pub struct DrumOscillator {
    /// The type of drum sound to produce
    sound: DrumSound,
    /// Sample rate in Hz
    sample_rate: f32,
    /// Current sample count (for time calculation)
    sample_count: usize,
    /// Maximum duration in samples (after which the sound is finished)
    max_samples: usize,
    /// Which track this oscillator belongs to
    pub track_id: usize,
    /// Random number generator for noise-based sounds
    rng: SimpleRng,
    /// Cached noise value for consistent noise across calls
    last_noise: f32,
    /// High-pass filter state for hi-hat
    hp_state: f32,
}

impl DrumOscillator {
    /// Create a new drum oscillator
    pub fn new(sound: DrumSound, sample_rate: f32, track_id: usize) -> Self {
        // Maximum duration depends on the sound type
        let max_duration_ms = match sound {
            DrumSound::Kick => 300.0,
            DrumSound::Snare => 200.0,
            DrumSound::HiHat => 80.0,
            DrumSound::OpenHiHat => 400.0,
            DrumSound::Clap => 150.0,
            DrumSound::Tom => 250.0,
            DrumSound::Crash => 800.0,
            DrumSound::Ride => 600.0,
            DrumSound::Rim => 100.0,
            DrumSound::Cowbell => 200.0,
        };
        let max_samples = (max_duration_ms * sample_rate / 1000.0) as usize;

        // Seed based on track_id and drum type for variety
        let seed = (track_id as u32 * 31337) ^ (sound.midi_note() as u32 * 7919);

        Self {
            sound,
            sample_rate,
            sample_count: 0,
            max_samples,
            track_id,
            rng: SimpleRng::new(seed.max(1)),
            last_noise: 0.0,
            hp_state: 0.0,
        }
    }

    /// Get the current time in seconds
    #[inline]
    fn time(&self) -> f32 {
        self.sample_count as f32 / self.sample_rate
    }

    /// Check if the drum sound has finished
    pub fn is_finished(&self) -> bool {
        self.sample_count >= self.max_samples
    }

    /// Generate the next sample
    pub fn next_sample(&mut self) -> f32 {
        if self.is_finished() {
            return 0.0;
        }

        let sample = match self.sound {
            DrumSound::Kick => self.kick(),
            DrumSound::Snare => self.snare(),
            DrumSound::HiHat => self.hihat(false),
            DrumSound::OpenHiHat => self.hihat(true),
            DrumSound::Clap => self.clap(),
            DrumSound::Tom => self.tom(),
            DrumSound::Crash => self.crash(),
            DrumSound::Ride => self.ride(),
            DrumSound::Rim => self.rim(),
            DrumSound::Cowbell => self.cowbell(),
        };

        self.sample_count += 1;
        sample
    }

    /// Kick drum: sine wave with pitch sweep
    fn kick(&self) -> f32 {
        let t = self.time();

        // Pitch sweep from ~150Hz down to ~50Hz
        let pitch = 150.0 * (-t * 25.0).exp() + 50.0;

        // Amplitude envelope with fast attack, moderate decay
        let amp = (-t * 10.0).exp();

        // Add a bit of click at the start
        let click = if t < 0.005 {
            (2.0 * PI * 2000.0 * t).sin() * (1.0 - t / 0.005)
        } else {
            0.0
        };

        (2.0 * PI * pitch * t).sin() * amp * 0.8 + click * 0.2
    }

    /// Snare drum: noise + tone body
    fn snare(&mut self) -> f32 {
        let t = self.time();

        // Tonal component (body of the drum)
        let body_freq = 200.0;
        let body = (2.0 * PI * body_freq * t).sin();
        let body_env = (-t * 30.0).exp();

        // Noise component (snare wires)
        let noise = self.rng.noise();
        let noise_env = (-t * 15.0).exp();

        // Mix body and noise
        body * body_env * 0.3 + noise * noise_env * 0.7
    }

    /// Hi-hat: filtered noise
    fn hihat(&mut self, open: bool) -> f32 {
        let t = self.time();

        // Decay rate depends on open/closed
        let decay = if open { 5.0 } else { 50.0 };
        let amp = (-t * decay).exp();

        // Generate noise
        let noise = self.rng.noise();

        // Simple high-pass filter for metallic sound
        let hp_cutoff = 0.8;
        self.hp_state = hp_cutoff * (self.hp_state + noise - self.last_noise);
        self.last_noise = noise;

        self.hp_state * amp * 0.5
    }

    /// Hand clap: multiple noise bursts
    fn clap(&mut self) -> f32 {
        let t = self.time();

        // Multiple "hits" with slight delays
        let mut signal = 0.0;

        for i in 0..3 {
            let offset = i as f32 * 0.015; // 15ms between hits
            if t >= offset {
                let t_hit = t - offset;
                let noise = self.rng.noise();
                let env = (-t_hit * 20.0).exp();
                signal += noise * env * 0.4;
            }
        }

        signal
    }

    /// Tom drum: sine with pitch sweep (lower than kick)
    fn tom(&self) -> f32 {
        let t = self.time();

        // Pitch sweep
        let pitch = 120.0 * (-t * 15.0).exp() + 80.0;
        let amp = (-t * 12.0).exp();

        (2.0 * PI * pitch * t).sin() * amp * 0.8
    }

    /// Crash cymbal: noise with shimmer
    fn crash(&mut self) -> f32 {
        let t = self.time();

        // Slow decay for crash
        let amp = (-t * 3.0).exp();

        // Noise with slight tonal component
        let noise = self.rng.noise();
        let shimmer = (2.0 * PI * 5000.0 * t).sin() * 0.1;

        // High-pass filter
        self.hp_state = 0.9 * (self.hp_state + noise - self.last_noise);
        self.last_noise = noise;

        (self.hp_state + shimmer) * amp * 0.4
    }

    /// Ride cymbal: higher pitched, more bell-like
    fn ride(&mut self) -> f32 {
        let t = self.time();

        let amp = (-t * 4.0).exp();

        // Bell-like tone
        let bell = (2.0 * PI * 800.0 * t).sin() * 0.3
            + (2.0 * PI * 1200.0 * t).sin() * 0.2
            + (2.0 * PI * 2400.0 * t).sin() * 0.1;

        // Add some noise
        let noise = self.rng.noise();

        (bell + noise * 0.2) * amp * 0.5
    }

    /// Rim shot / side stick
    fn rim(&self) -> f32 {
        let t = self.time();

        // Very short, high frequency click
        let amp = (-t * 80.0).exp();

        let click = (2.0 * PI * 1500.0 * t).sin() + (2.0 * PI * 2000.0 * t).sin() * 0.5;

        click * amp * 0.6
    }

    /// Cowbell
    fn cowbell(&self) -> f32 {
        let t = self.time();

        let amp = (-t * 8.0).exp();

        // Two slightly detuned frequencies for metallic sound
        let tone1 = (2.0 * PI * 560.0 * t).sin();
        let tone2 = (2.0 * PI * 845.0 * t).sin();

        (tone1 * 0.6 + tone2 * 0.4) * amp * 0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kick_synthesis() {
        let mut osc = DrumOscillator::new(DrumSound::Kick, 44100.0, 1);

        // Generate some samples
        let mut samples = Vec::new();
        for _ in 0..1000 {
            samples.push(osc.next_sample());
        }

        // Sample a few in should be non-zero (sin(0) = 0, so check sample 10)
        assert!(samples[10].abs() > 0.0, "sample[10] should be non-zero");

        // Should have some energy in the attack phase
        let max_early: f32 = samples[0..100].iter().map(|s| s.abs()).fold(0.0, f32::max);
        assert!(max_early > 0.1, "should have attack energy");

        // Should decay over time
        let max_late: f32 = samples[900..1000]
            .iter()
            .map(|s| s.abs())
            .fold(0.0, f32::max);
        assert!(max_late < max_early, "should decay over time");
    }

    #[test]
    fn test_drum_finishes() {
        let mut osc = DrumOscillator::new(DrumSound::HiHat, 44100.0, 1);

        // Hi-hat should finish within its max duration
        while !osc.is_finished() {
            osc.next_sample();
        }

        // After finishing, should return 0
        assert_eq!(osc.next_sample(), 0.0);
    }

    #[test]
    fn test_all_drum_sounds() {
        let drums = [
            DrumSound::Kick,
            DrumSound::Snare,
            DrumSound::HiHat,
            DrumSound::OpenHiHat,
            DrumSound::Clap,
            DrumSound::Tom,
            DrumSound::Crash,
            DrumSound::Ride,
            DrumSound::Rim,
            DrumSound::Cowbell,
        ];

        for drum in drums {
            let mut osc = DrumOscillator::new(drum, 44100.0, 1);

            // Should produce some output
            let sample = osc.next_sample();
            // Note: some drums might start with 0 due to timing
            // Just ensure no panics
            assert!(sample.is_finite());
        }
    }
}
