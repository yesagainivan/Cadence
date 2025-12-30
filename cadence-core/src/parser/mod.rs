// cadence-core/src/parser/mod.rs

pub mod ast;
pub mod binder;
pub mod builtins;
pub mod environment;
pub mod error;
pub mod evaluator;
pub mod interpreter;
pub mod lexer;
pub mod statement_parser;
pub mod symbols;

#[cfg(test)]
mod evaluator_tests;

pub use ast::{Expression, Program, Statement, Value};
pub use environment::{Environment, SharedEnvironment};
pub use error::CadenceError;
pub use evaluator::{eval, Evaluator};
pub use interpreter::{ControlFlow, Interpreter, InterpreterAction};
pub use lexer::{Lexer, Token};
pub use statement_parser::{parse_expression as parse, parse_statements, StatementParser};
