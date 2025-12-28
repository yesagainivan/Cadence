pub mod adsr;
pub mod audio;
pub mod clock;
pub mod drum_synth;
pub mod event_dispatcher;
pub mod midi;
pub mod oscillator;
pub mod playback_engine;
pub mod scheduler;

// Re-export common types from types::audio_config for backward compatibility
pub use crate::types::audio_config::{AdsrParams, QueueMode, Waveform};
