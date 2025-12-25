//! ADSR (Attack, Decay, Sustain, Release) envelope generator
//!
//! Provides sample-accurate amplitude envelopes with exponential curves
//! for natural-sounding note attacks and releases.
//!
//! # Example
//! ```ignore
//! let params = AdsrParams::default();
//! let mut env = AdsrEnvelope::new(params, 44100.0);
//! env.trigger(); // Start attack
//!
//! // In audio callback:
//! let amplitude = env.next_sample();
//!
//! // When note should stop:
//! env.release(); // Start release phase
//! ```

/// ADSR envelope stages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeStage {
    /// Not active, output is 0
    Idle,
    /// Rising from 0 to peak (1.0)
    Attack,
    /// Falling from peak to sustain level
    Decay,
    /// Holding at sustain level while note is held
    Sustain,
    /// Falling from current level to 0 after note-off
    Release,
}

/// ADSR envelope parameters
///
/// - `attack`: Time in seconds to rise from 0 to peak (1.0)
/// - `decay`: Time in seconds to fall from peak to sustain level
/// - `sustain`: Level to hold while note is held (0.0-1.0, NOT time!)
/// - `release`: Time in seconds to fall from sustain to 0 after note-off
#[derive(Debug, Clone, Copy)]
pub struct AdsrParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl AdsrParams {
    /// Create custom ADSR parameters
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self {
            attack: attack.max(0.001), // Minimum 1ms to avoid clicks
            decay: decay.max(0.0),
            sustain: sustain.clamp(0.0, 1.0),
            release: release.max(0.001), // Minimum 1ms to avoid clicks
        }
    }

    /// Default envelope - smooth and musical
    /// Good for general use, slight attack to prevent clicks
    pub fn default_envelope() -> Self {
        Self::new(0.01, 0.1, 0.7, 0.2)
    }

    /// Pluck - fast attack, quick decay, no sustain
    /// Good for: plucked strings, pizzicato, synth plucks
    pub fn pluck() -> Self {
        Self::new(0.001, 0.15, 0.0, 0.1)
    }

    /// Pad - slow attack and release, high sustain
    /// Good for: pads, strings, ambient textures
    pub fn pad() -> Self {
        Self::new(0.3, 0.2, 0.8, 0.5)
    }

    /// Percussion - instant attack, fast decay, no sustain
    /// Good for: drums, percussion, staccato sounds
    pub fn perc() -> Self {
        Self::new(0.001, 0.2, 0.0, 0.05)
    }

    /// Organ - instant attack and release, full sustain
    /// Good for: organ sounds, sustained tones
    pub fn organ() -> Self {
        Self::new(0.005, 0.0, 1.0, 0.01)
    }
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self::default_envelope()
    }
}

/// Per-sample ADSR envelope generator
///
/// Uses exponential curves for natural-sounding amplitude changes.
/// Sample-rate independent - envelope times are specified in seconds.
pub struct AdsrEnvelope {
    params: AdsrParams,
    stage: EnvelopeStage,
    level: f32,
    sample_rate: f32,

    // Pre-computed coefficients for exponential curves
    attack_coeff: f32,
    decay_coeff: f32,
    release_coeff: f32,
}

impl AdsrEnvelope {
    /// Create a new ADSR envelope with the given parameters
    pub fn new(params: AdsrParams, sample_rate: f32) -> Self {
        let mut env = Self {
            params,
            stage: EnvelopeStage::Idle,
            level: 0.0,
            sample_rate,
            attack_coeff: 0.0,
            decay_coeff: 0.0,
            release_coeff: 0.0,
        };
        env.recalculate_coefficients();
        env
    }

    /// Recalculate exponential coefficients based on current params and sample rate
    fn recalculate_coefficients(&mut self) {
        // Exponential envelope formula: level = level + (target - level) * coeff
        // To reach ~99.9% of target in `time` seconds:
        // coeff = 1 - exp(-6.9 / (time * sample_rate))
        // Using -6.9 because exp(-6.9) ≈ 0.001 (reaches 99.9% of target)

        let time_constant = 6.9; // ln(1000) ≈ 6.9 for 99.9% convergence

        self.attack_coeff = if self.params.attack > 0.0 {
            1.0 - (-time_constant / (self.params.attack * self.sample_rate)).exp()
        } else {
            1.0 // Instant attack
        };

        self.decay_coeff = if self.params.decay > 0.0 {
            1.0 - (-time_constant / (self.params.decay * self.sample_rate)).exp()
        } else {
            1.0 // Instant decay
        };

        self.release_coeff = if self.params.release > 0.0 {
            1.0 - (-time_constant / (self.params.release * self.sample_rate)).exp()
        } else {
            1.0 // Instant release
        };
    }

    /// Trigger the envelope (start attack phase)
    /// Call this when a note starts
    pub fn trigger(&mut self) {
        self.stage = EnvelopeStage::Attack;
        // Don't reset level to 0 - allows retriggering during release
        // for smoother behavior
    }

    /// Release the envelope (start release phase)
    /// Call this when a note ends
    pub fn release(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage = EnvelopeStage::Release;
        }
    }

    /// Force immediate stop (for emergencies, may click!)
    pub fn force_stop(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.level = 0.0;
    }

    /// Get the current envelope stage
    pub fn stage(&self) -> EnvelopeStage {
        self.stage
    }

    /// Get the current envelope level
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Check if envelope has finished (released and faded out)
    pub fn is_finished(&self) -> bool {
        self.stage == EnvelopeStage::Idle
            || (self.stage == EnvelopeStage::Release && self.level < 0.0001)
    }

    /// Check if envelope is active (not idle or finished)
    pub fn is_active(&self) -> bool {
        !self.is_finished()
    }

    /// Generate the next sample of the envelope
    /// Returns amplitude value between 0.0 and 1.0
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            EnvelopeStage::Idle => {
                self.level = 0.0;
            }

            EnvelopeStage::Attack => {
                // Exponential rise toward 1.0
                self.level += (1.0 - self.level) * self.attack_coeff;

                // Transition to Decay when we're close enough to peak
                if self.level >= 0.999 {
                    self.level = 1.0;
                    self.stage = EnvelopeStage::Decay;
                }
            }

            EnvelopeStage::Decay => {
                // Exponential fall toward sustain level
                let target = self.params.sustain;
                self.level += (target - self.level) * self.decay_coeff;

                // Transition to Sustain when we're close enough
                if (self.level - target).abs() < 0.001 {
                    self.level = target;
                    self.stage = EnvelopeStage::Sustain;
                }
            }

            EnvelopeStage::Sustain => {
                // Hold at sustain level
                self.level = self.params.sustain;
                // Stay here until release() is called
            }

            EnvelopeStage::Release => {
                // Exponential fall toward 0
                self.level += (0.0 - self.level) * self.release_coeff;

                // Transition to Idle when we're essentially silent
                if self.level < 0.0001 {
                    self.level = 0.0;
                    self.stage = EnvelopeStage::Idle;
                }
            }
        }

        self.level
    }

    /// Update envelope parameters (takes effect on next note)
    pub fn set_params(&mut self, params: AdsrParams) {
        self.params = params;
        self.recalculate_coefficients();
    }

    /// Update sample rate (e.g., if audio device changes)
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.recalculate_coefficients();
    }
}

impl Clone for AdsrEnvelope {
    fn clone(&self) -> Self {
        Self {
            params: self.params,
            stage: self.stage,
            level: self.level,
            sample_rate: self.sample_rate,
            attack_coeff: self.attack_coeff,
            decay_coeff: self.decay_coeff,
            release_coeff: self.release_coeff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    #[test]
    fn test_envelope_idle_start() {
        let env = AdsrEnvelope::new(AdsrParams::default(), SAMPLE_RATE);
        assert_eq!(env.stage(), EnvelopeStage::Idle);
        assert_eq!(env.level(), 0.0);
    }

    #[test]
    fn test_envelope_trigger_starts_attack() {
        let mut env = AdsrEnvelope::new(AdsrParams::default(), SAMPLE_RATE);
        env.trigger();
        assert_eq!(env.stage(), EnvelopeStage::Attack);
    }

    #[test]
    fn test_envelope_attack_rises() {
        let params = AdsrParams::new(0.01, 0.1, 0.7, 0.2);
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        let initial = env.level();

        // Generate some samples
        for _ in 0..100 {
            env.next_sample();
        }

        assert!(env.level() > initial, "Level should rise during attack");
    }

    #[test]
    fn test_envelope_reaches_peak() {
        let params = AdsrParams::new(0.01, 0.1, 0.7, 0.2);
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        // Generate enough samples to complete attack (10ms at 44100Hz = 441 samples)
        for _ in 0..1000 {
            env.next_sample();
        }

        // Should be in Decay or Sustain by now
        assert!(
            env.stage() == EnvelopeStage::Decay || env.stage() == EnvelopeStage::Sustain,
            "Should have completed attack, got {:?}",
            env.stage()
        );
    }

    #[test]
    fn test_envelope_sustain_level() {
        let sustain_level = 0.6;
        let params = AdsrParams::new(0.001, 0.01, sustain_level, 0.1);
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        // Generate enough samples to reach sustain
        for _ in 0..5000 {
            env.next_sample();
        }

        assert_eq!(env.stage(), EnvelopeStage::Sustain);
        assert!((env.level() - sustain_level).abs() < 0.01);
    }

    #[test]
    fn test_envelope_release_from_sustain() {
        let params = AdsrParams::new(0.001, 0.01, 0.7, 0.1);
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        // Reach sustain
        for _ in 0..5000 {
            env.next_sample();
        }

        // Trigger release
        env.release();
        assert_eq!(env.stage(), EnvelopeStage::Release);

        let level_at_release = env.level();

        // Generate more samples
        for _ in 0..1000 {
            env.next_sample();
        }

        assert!(
            env.level() < level_at_release,
            "Level should fall during release"
        );
    }

    #[test]
    fn test_envelope_finishes() {
        let params = AdsrParams::new(0.001, 0.01, 0.5, 0.01);
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        // Reach sustain
        for _ in 0..2000 {
            env.next_sample();
        }

        env.release();

        // Let it finish releasing
        for _ in 0..5000 {
            env.next_sample();
        }

        assert!(env.is_finished(), "Envelope should be finished");
    }

    #[test]
    fn test_envelope_release_from_attack() {
        // Test that note-off during attack works smoothly
        let params = AdsrParams::new(0.1, 0.1, 0.7, 0.1);
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        // Only 10 samples into attack
        for _ in 0..10 {
            env.next_sample();
        }

        let level_before_release = env.level();
        env.release();

        assert_eq!(env.stage(), EnvelopeStage::Release);
        // Level should be same immediately after release
        assert!((env.level() - level_before_release).abs() < 0.01);
    }

    #[test]
    fn test_preset_pluck() {
        let params = AdsrParams::pluck();
        assert!(params.attack < 0.01);
        assert!(params.sustain < 0.01);
    }

    #[test]
    fn test_preset_pad() {
        let params = AdsrParams::pad();
        assert!(params.attack > 0.1);
        assert!(params.sustain > 0.5);
    }

    #[test]
    fn test_output_range() {
        let params = AdsrParams::default();
        let mut env = AdsrEnvelope::new(params, SAMPLE_RATE);
        env.trigger();

        for _ in 0..10000 {
            let sample = env.next_sample();
            assert!(
                sample >= 0.0 && sample <= 1.0,
                "Sample {} out of range",
                sample
            );
        }

        env.release();

        for _ in 0..10000 {
            let sample = env.next_sample();
            assert!(
                sample >= 0.0 && sample <= 1.0,
                "Sample {} out of range",
                sample
            );
        }
    }
}
