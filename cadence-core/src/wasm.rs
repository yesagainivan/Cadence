//! WASM bindings for cadence-core
//!
//! Provides JavaScript-accessible functions for tokenization and parsing.

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::parser::lexer::{Lexer, SpannedToken, Token};

/// A highlight span for syntax highlighting in the editor
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HighlightSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub token_type: String,
    pub text: String,
}

impl HighlightSpan {
    pub fn from_spanned_token(token: &SpannedToken, _source: &str) -> Self {
        let token_type = Self::classify_token(&token.token);
        let text = Self::extract_text(&token.token);

        // Estimate end position based on text length
        let end_col = token.span.column + text.len();

        HighlightSpan {
            start_line: token.span.line,
            start_col: token.span.column,
            end_line: token.span.line, // Single-line tokens for now
            end_col,
            token_type,
            text,
        }
    }

    fn classify_token(token: &Token) -> String {
        match token {
            // Keywords
            Token::Let
            | Token::Play
            | Token::Stop
            | Token::Loop
            | Token::Repeat
            | Token::If
            | Token::Else
            | Token::Break
            | Token::Continue
            | Token::Return
            | Token::Track
            | Token::Load
            | Token::Fn
            | Token::On => "keyword".to_string(),

            // Control keywords
            Token::Tempo | Token::Volume | Token::Waveform | Token::Queue => {
                "keyword.control".to_string()
            }

            // Notes (musical)
            Token::Note(_) => "constant.note".to_string(),

            // Numbers
            Token::Number(_) | Token::Float(_) => "constant.numeric".to_string(),

            // Strings
            Token::StringLiteral(_) => "string".to_string(),

            // Operators
            Token::Plus | Token::Minus | Token::Ampersand | Token::Pipe | Token::Caret => {
                "operator".to_string()
            }

            // Comparison
            Token::DoubleEquals | Token::NotEquals => "operator.comparison".to_string(),

            // Assignment
            Token::Equals => "operator.assignment".to_string(),

            // Punctuation
            Token::LeftParen
            | Token::RightParen
            | Token::LeftBracket
            | Token::RightBracket
            | Token::LeftDoubleBracket
            | Token::RightDoubleBracket
            | Token::LeftBrace
            | Token::RightBrace
            | Token::Comma
            | Token::Semicolon
            | Token::Dot => "punctuation".to_string(),

            // Identifiers (function names, variables)
            Token::Identifier(_) => "variable".to_string(),

            // Booleans
            Token::Boolean(_) => "constant.boolean".to_string(),

            // Comments
            Token::Comment(_) => "comment".to_string(),

            // Newline (not visible)
            Token::Newline => "".to_string(),

            // EOF
            Token::Eof => "".to_string(),
        }
    }

    fn extract_text(token: &Token) -> String {
        match token {
            Token::Note(s) | Token::Identifier(s) => s.clone(),
            // String literals need quotes for correct span length
            Token::StringLiteral(s) => format!("\"{}\"", s),
            Token::Number(n) => n.to_string(),
            Token::Float(f) => f.to_string(),
            Token::Boolean(b) => b.to_string(),
            Token::Let => "let".to_string(),
            Token::Play => "play".to_string(),
            Token::Stop => "stop".to_string(),
            Token::Loop => "loop".to_string(),
            Token::Repeat => "repeat".to_string(),
            Token::If => "if".to_string(),
            Token::Else => "else".to_string(),
            Token::Break => "break".to_string(),
            Token::Continue => "continue".to_string(),
            Token::Return => "return".to_string(),
            Token::Track => "track".to_string(),
            Token::Load => "load".to_string(),
            Token::Fn => "fn".to_string(),
            Token::On => "on".to_string(),
            Token::Tempo => "tempo".to_string(),
            Token::Volume => "volume".to_string(),
            Token::Waveform => "waveform".to_string(),
            Token::Queue => "queue".to_string(),
            Token::Plus => "+".to_string(),
            Token::Minus => "-".to_string(),
            Token::Ampersand => "&".to_string(),
            Token::Pipe => "|".to_string(),
            Token::Caret => "^".to_string(),
            Token::LeftParen => "(".to_string(),
            Token::RightParen => ")".to_string(),
            Token::LeftBracket => "[".to_string(),
            Token::RightBracket => "]".to_string(),
            Token::LeftDoubleBracket => "[[".to_string(),
            Token::RightDoubleBracket => "]]".to_string(),
            Token::LeftBrace => "{".to_string(),
            Token::RightBrace => "}".to_string(),
            Token::Comma => ",".to_string(),
            Token::Semicolon => ";".to_string(),
            Token::Dot => ".".to_string(),
            Token::Equals => "=".to_string(),
            Token::DoubleEquals => "==".to_string(),
            Token::NotEquals => "!=".to_string(),
            Token::Newline => "\n".to_string(),
            Token::Comment(s) => format!("//{}\n", s), // Include // prefix
            Token::Eof => "".to_string(),
        }
    }
}

/// Tokenize input and return highlight spans
pub fn tokenize_for_highlighting(input: &str) -> Vec<HighlightSpan> {
    let mut lexer = Lexer::new(input);

    // Handle tokenization errors gracefully
    let tokens = match lexer.tokenize_spanned() {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    tokens
        .iter()
        .filter(|t| !matches!(t.token, Token::Eof | Token::Newline))
        .map(|t| HighlightSpan::from_spanned_token(t, input))
        .collect()
}

// ============================================================================
// WASM Bindings
// ============================================================================

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn tokenize(input: &str) -> JsValue {
    let spans = tokenize_for_highlighting(input);
    serde_wasm_bindgen::to_value(&spans).unwrap_or(JsValue::NULL)
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn parse_and_check(input: &str) -> JsValue {
    use crate::parser::parse_statements;

    match parse_statements(input) {
        Ok(_) => serde_wasm_bindgen::to_value(&ParseResult {
            success: true,
            error: None,
        })
        .unwrap_or(JsValue::NULL),
        Err(e) => serde_wasm_bindgen::to_value(&ParseResult {
            success: false,
            error: Some(e.to_string()),
        })
        .unwrap_or(JsValue::NULL),
    }
}

#[cfg(feature = "wasm")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ParseResult {
    success: bool,
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_for_highlighting() {
        let input = "let x = [C, E, G]";
        let spans = tokenize_for_highlighting(input);

        assert!(!spans.is_empty());

        // Check first token is 'let' keyword
        assert_eq!(spans[0].token_type, "keyword");
        assert_eq!(spans[0].text, "let");

        // Find notes
        let notes: Vec<_> = spans
            .iter()
            .filter(|s| s.token_type == "constant.note")
            .collect();
        assert_eq!(notes.len(), 3); // C, E, G
    }

    #[test]
    fn test_token_positions_in_chord() {
        // Test that all tokens in a chord have correct positions
        let input = "[C, E, G]";
        let spans = tokenize_for_highlighting(input);

        // Print spans for debugging
        for (i, span) in spans.iter().enumerate() {
            println!(
                "Span {}: {} at col {} (text: '{}')",
                i, span.token_type, span.start_col, span.text
            );
        }

        // Should have: [ C , E , G ]
        assert_eq!(spans.len(), 7, "Expected 7 tokens: [ C , E , G ]");

        // Check positions are strictly increasing
        assert_eq!(spans[0].start_col, 1); // [
        assert_eq!(spans[0].text, "[");

        assert_eq!(spans[1].start_col, 2); // C
        assert_eq!(spans[1].text, "C");
        assert_eq!(spans[1].token_type, "constant.note");

        assert_eq!(spans[2].start_col, 3); // ,

        assert_eq!(spans[3].start_col, 5); // E (after ", ")
        assert_eq!(spans[3].text, "E");
        assert_eq!(spans[3].token_type, "constant.note");

        assert_eq!(spans[4].start_col, 6); // ,

        assert_eq!(spans[5].start_col, 8); // G
        assert_eq!(spans[5].text, "G");
        assert_eq!(spans[5].token_type, "constant.note");

        assert_eq!(spans[6].start_col, 9); // ]
    }

    #[test]
    fn test_tokenize_pattern() {
        let input = r#"play "C E G _" loop"#;
        let spans = tokenize_for_highlighting(input);

        // Should have play keyword
        assert!(spans
            .iter()
            .any(|s| s.token_type == "keyword" && s.text == "play"));

        // Should have string
        assert!(spans.iter().any(|s| s.token_type == "string"));

        // Should have loop keyword
        assert!(spans
            .iter()
            .any(|s| s.token_type == "keyword" && s.text == "loop"));
    }

    #[test]
    fn test_classify_all_tokens() {
        // Test various token types to ensure classification works
        let test_cases = vec![
            ("tempo 120", vec!["keyword.control", "constant.numeric"]),
            (
                "[C, E, G]",
                vec![
                    "punctuation",
                    "constant.note",
                    "punctuation",
                    "constant.note",
                    "punctuation",
                    "constant.note",
                    "punctuation",
                ],
            ),
            (
                "x = 5",
                vec!["variable", "operator.assignment", "constant.numeric"],
            ),
        ];

        for (input, expected_types) in test_cases {
            let spans = tokenize_for_highlighting(input);
            let types: Vec<_> = spans.iter().map(|s| s.token_type.as_str()).collect();
            assert_eq!(types, expected_types, "Failed for input: {}", input);
        }
    }
}
