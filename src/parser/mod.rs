// src/parser/mod.rs
pub mod ast;
pub mod evaluator;
pub mod lexer;
pub mod parser;

pub use ast::{Expression, Value};
pub use evaluator::{Evaluator, eval};
pub use lexer::{Lexer, Token};
pub use parser::{Parser, parse};
