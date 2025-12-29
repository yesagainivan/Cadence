pub mod adsr;
pub mod audio;
pub mod clock;
pub mod drum_synth;
pub mod event_dispatcher;
pub mod midi;
pub mod oscillator;

// Deprecated modules moved to _deprecated/ directory:
// - playback_engine.rs (replaced by event_dispatcher)
// - scheduler.rs (replaced by event_dispatcher)

// Re-export common types from types::audio_config for backward compatibility
pub use crate::types::audio_config::{AdsrParams, QueueMode, Waveform};
