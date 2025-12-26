//! Oscillator module with multiple waveform support
//!
//! Provides `EnvelopedOscillator` for sample generation with ADSR envelopes
//! and support for sine, saw, square, and triangle waveforms.

use super::adsr::AdsrEnvelope;
use crate::types::audio_config::{AdsrParams, Waveform};
use std::f32::consts::PI;

/// Per-note oscillator state with ADSR amplitude envelope
pub struct EnvelopedOscillator {
    frequency: f32,
    phase: f32,
    sample_rate: f32,
    envelope: AdsrEnvelope,
    waveform: Waveform,
    /// Which track this oscillator belongs to
    pub track_id: usize,
}

impl EnvelopedOscillator {
    /// Create a new oscillator with default ADSR envelope and sine waveform
    #[allow(dead_code)]
    pub fn new(frequency: f32, sample_rate: f32, track_id: usize) -> Self {
        Self::with_params(frequency, sample_rate, track_id, None, Waveform::Sine)
    }

    /// Create a new oscillator with custom ADSR envelope and waveform
    pub fn with_params(
        frequency: f32,
        sample_rate: f32,
        track_id: usize,
        envelope_params: Option<(f32, f32, f32, f32)>,
        waveform: Waveform,
    ) -> Self {
        let params = match envelope_params {
            Some((a, d, s, r)) => AdsrParams::new(a, d, s, r),
            None => AdsrParams::default(),
        };
        let mut envelope = AdsrEnvelope::new(params, sample_rate);
        envelope.trigger(); // Start the envelope immediately

        Self {
            frequency,
            phase: 0.0,
            sample_rate,
            envelope,
            waveform,
            track_id,
        }
    }

    /// Backward-compatible constructor (used by audio.rs)
    pub fn with_envelope(
        frequency: f32,
        sample_rate: f32,
        track_id: usize,
        envelope_params: Option<(f32, f32, f32, f32)>,
        waveform: Waveform,
    ) -> Self {
        Self::with_params(frequency, sample_rate, track_id, envelope_params, waveform)
    }

    /// Start fade out (begin release phase)
    pub fn start_fade_out(&mut self) {
        self.envelope.release();
    }

    /// Check if envelope has finished
    pub fn is_finished(&self) -> bool {
        self.envelope.is_finished()
    }

    /// Generate the next sample
    pub fn next_sample(&mut self) -> f32 {
        // Generate waveform based on type
        let value = self.generate_waveform();

        // Advance phase
        self.phase += self.frequency / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        // Apply ADSR envelope
        let amplitude = self.envelope.next_sample();
        value * amplitude
    }

    /// Generate raw waveform value based on current phase (0.0 to 1.0)
    fn generate_waveform(&self) -> f32 {
        match self.waveform {
            Waveform::Sine => self.sine(),
            Waveform::Saw => self.saw(),
            Waveform::Square => self.square(),
            Waveform::Triangle => self.triangle(),
        }
    }

    /// Sine wave: smooth, pure tone
    #[inline]
    fn sine(&self) -> f32 {
        (2.0 * PI * self.phase).sin()
    }

    /// Sawtooth wave: bright, buzzy - all harmonics
    /// Ramps from -1 to 1, then resets
    #[inline]
    fn saw(&self) -> f32 {
        2.0 * self.phase - 1.0
    }

    /// Square wave: hollow, woody - odd harmonics only
    /// Alternates between -1 and 1
    #[inline]
    fn square(&self) -> f32 {
        if self.phase < 0.5 { 1.0 } else { -1.0 }
    }

    /// Triangle wave: mellow, flute-like - odd harmonics, quieter
    /// Linear ramp up then down
    #[inline]
    fn triangle(&self) -> f32 {
        if self.phase < 0.5 {
            4.0 * self.phase - 1.0
        } else {
            3.0 - 4.0 * self.phase
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    #[test]
    fn test_waveform_parsing() {
        assert_eq!(Waveform::from_str("sine"), Some(Waveform::Sine));
        assert_eq!(Waveform::from_str("SAW"), Some(Waveform::Saw));
        assert_eq!(Waveform::from_str("Square"), Some(Waveform::Square));
        assert_eq!(Waveform::from_str("tri"), Some(Waveform::Triangle));
        assert_eq!(Waveform::from_str("invalid"), None);
    }

    #[test]
    fn test_sine_range() {
        let mut osc = EnvelopedOscillator::with_params(440.0, SAMPLE_RATE, 1, None, Waveform::Sine);
        for _ in 0..1000 {
            let sample = osc.next_sample();
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Sine out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_saw_range() {
        let mut osc = EnvelopedOscillator::with_params(440.0, SAMPLE_RATE, 1, None, Waveform::Saw);
        for _ in 0..1000 {
            let sample = osc.next_sample();
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Saw out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_square_range() {
        let mut osc =
            EnvelopedOscillator::with_params(440.0, SAMPLE_RATE, 1, None, Waveform::Square);
        for _ in 0..1000 {
            let sample = osc.next_sample();
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Square out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_triangle_range() {
        let mut osc =
            EnvelopedOscillator::with_params(440.0, SAMPLE_RATE, 1, None, Waveform::Triangle);
        for _ in 0..1000 {
            let sample = osc.next_sample();
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Triangle out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_default_waveform_is_sine() {
        assert_eq!(Waveform::default(), Waveform::Sine);
    }
}
