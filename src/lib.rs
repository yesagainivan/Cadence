//! # Cadence
//!
//! Cadence is a Rust library for musical theory and composition, providing tools
//! for representing and manipulating musical notes, chords, and progressions.
//! It includes a parser and evaluator for a custom musical expression language,
//! as well as functionalities for voice leading analysis, Roman numeral analysis,
//! and generating common chord progressions.
//!
//! The library aims to provide a programmatic interface for exploring and
//! generating harmonic structures.
//!
//! ## Modules
//!
//! - `parser`: Contains the lexer, parser, and abstract syntax tree (AST) for
//!   the Cadence expression language. It also includes the evaluator responsible
//!   for interpreting expressions.
//! - `repl`: Provides the Read-Eval-Print Loop for interactive use of the Cadence language.
//! - `types`: Defines the core data structures for musical concepts like notes,
//!   chords, progressions, and Roman numerals, along with their associated
//!   logic and operations.

pub mod audio;
pub mod commands;
pub mod parser;
pub mod repl;
pub mod types;

// Re-export commonly used types and functions for convenience
pub use crate::parser::{Expression, Value, eval};
pub use crate::types::{Chord, CommonProgressions, Note, Progression, RomanNumeral, VoiceLeading};
