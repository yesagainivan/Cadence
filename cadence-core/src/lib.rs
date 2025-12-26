//! # Cadence Core
//!
//! WASM-compatible core library for the Cadence music programming language.
//! Provides types, parsing, and evaluation without audio/MIDI dependencies.
//!
//! ## Features
//!
//! - **serde**: Enable JSON serialization for web interop
//! - **wasm**: Enable WASM bindings via wasm-bindgen
//! - **colored**: Enable colored terminal output (disabled in WASM)
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
pub mod wasm;

// Re-export commonly used types
pub use types::{AdsrParams, Chord, Note, Pattern, QueueMode, Waveform};

// Re-export WASM functions when wasm feature is enabled
pub use wasm::{tokenize_for_highlighting, HighlightSpan};
