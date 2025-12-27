pub mod adsr;
pub mod audio;
pub mod clock;
pub mod drum_synth;
pub mod midi;
pub mod oscillator;
pub mod playback_engine;

// Re-export common types from types::audio_config for backward compatibility
pub use crate::types::audio_config::{AdsrParams, QueueMode, Waveform};
