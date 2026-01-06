//! Pattern type for TidalCycles-inspired mini-notation
//!
//! Enables cycle-based patterns like `"C E G _"` where all steps fit into one cycle,
//! with support for rests, repetition, and grouping.

mod core;
mod euclidean;
mod event;
mod every;
mod parser;
mod step;

#[cfg(test)]
mod tests;

// Re-export public types
pub use core::Pattern;
pub use euclidean::bjorklund;
pub use event::{NoteInfo, PlaybackEvent};
pub use every::EveryPattern;
pub use step::PatternStep;
