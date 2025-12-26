//! # Cadence Core
//!
//! WASM-compatible core library for the Cadence music programming language.
//! Provides types, parsing, and evaluation without audio/MIDI dependencies.
//!
//! ## Features
//!
//! - **serde**: Enable JSON serialization for web interop
//!
//! ## Example
//!
//! ```ignore
//! use cadence_core::types::{Note, Chord, Pattern};
//! use cadence_core::parser::{Lexer, parse_statements};
//!
//! let pattern = Pattern::parse("C E G _")?;
//! println!("Steps: {}", pattern.len());
//! ```

pub mod parser;
pub mod types;

// Re-export commonly used types
pub use types::{AdsrParams, Chord, Note, Pattern, QueueMode, Waveform};
