//! Audio configuration types for WASM compatibility
//!
//! These pure data types carry no synthesis/playback logic, making them safe
//! for compilation to WebAssembly. They define waveforms, envelopes, and
//! scheduling modes that can be used by both the native audio engine and
//! web-based editors.

/// Available waveform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Waveform {
    #[default]
    Sine,
    Saw,
    Square,
    Triangle,
}

impl Waveform {
    /// Parse waveform from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Waveform> {
        match s.to_lowercase().as_str() {
            "sine" | "sin" => Some(Waveform::Sine),
            "saw" | "sawtooth" => Some(Waveform::Saw),
            "square" | "sq" => Some(Waveform::Square),
            "triangle" | "tri" => Some(Waveform::Triangle),
            _ => None,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            Waveform::Sine => "sine",
            Waveform::Saw => "saw",
            Waveform::Square => "square",
            Waveform::Triangle => "triangle",
        }
    }
}

/// ADSR envelope parameters (pure data, no sample generation)
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

/// When to start a queued progression
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum QueueMode {
    /// Start at the next beat boundary (default)
    #[default]
    Beat,
    /// Start at the next bar boundary (beat 0, 4, 8, 12... in 4/4 time)
    Bar,
    /// Start when current pattern completes its cycle
    Cycle,
    /// Start after exactly N beats from now
    Beats(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_parsing() {
        assert_eq!(Waveform::from_str("sine"), Some(Waveform::Sine));
        assert_eq!(Waveform::from_str("SAW"), Some(Waveform::Saw));
        assert_eq!(Waveform::from_str("Square"), Some(Waveform::Square));
        assert_eq!(Waveform::from_str("tri"), Some(Waveform::Triangle));
        assert_eq!(Waveform::from_str("invalid"), None);
    }

    #[test]
    fn test_default_waveform_is_sine() {
        assert_eq!(Waveform::default(), Waveform::Sine);
    }

    #[test]
    fn test_adsr_params_clamping() {
        let params = AdsrParams::new(0.0, 0.0, 1.5, -1.0);
        assert!(params.attack >= 0.001);
        assert!(params.release >= 0.001);
        assert!(params.sustain <= 1.0);
    }

    #[test]
    fn test_queue_mode_default() {
        assert_eq!(QueueMode::default(), QueueMode::Beat);
    }
}
