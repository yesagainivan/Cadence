// src/parser/mod.rs
pub mod ast;
pub mod environment;
pub mod evaluator;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod statement_parser;

#[cfg(test)]
mod evaluator_tests;

pub use ast::{Expression, Program, Statement, Value};
pub use environment::{Environment, SharedEnvironment};
pub use evaluator::{Evaluator, eval};
pub use interpreter::{Interpreter, InterpreterAction};
pub use lexer::{Lexer, Token};
pub use parser::{Parser, parse};
pub use statement_parser::{StatementParser, parse_statements};
