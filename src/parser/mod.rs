// src/parser/mod.rs
// Re-export core parser modules from cadence-core
// Keep interpreter locally (it has audio/MIDI dependencies)

// Re-export modules
pub use cadence_core::parser::ast;
pub use cadence_core::parser::environment;
pub use cadence_core::parser::evaluator;
pub use cadence_core::parser::lexer;
pub use cadence_core::parser::statement_parser;

// Re-export commonly used types
pub use cadence_core::parser::{
    eval, parse_statements, Environment, Evaluator, Expression, Lexer, Program, SharedEnvironment,
    Statement, StatementParser, Token, Value,
};

// Re-export parse function (aliased from parse_expression)
pub use cadence_core::parser::statement_parser::parse_expression as parse;

// Interpreter stays in cadence (has audio dependencies)
pub mod interpreter;
pub use interpreter::{Interpreter, InterpreterAction};
