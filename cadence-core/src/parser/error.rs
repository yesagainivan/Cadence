use crate::parser::lexer::Span;
use std::fmt;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CadenceError {
    pub message: String,
    pub span: Span,
}

impl CadenceError {
    pub fn new(message: String, span: Span) -> Self {
        Self { message, span }
    }
}

impl fmt::Display for CadenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error at {}: {}", self.span, self.message)
    }
}

impl std::error::Error for CadenceError {}
