use anyhow::{Result, anyhow};
use std::fmt;

/// Represents different types of tokens in the Cadence language
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Note(String), // C, F#, Bb
    Number(i8),   // 2, -5, 12

    // Delimiters
    LeftBracket,        // [
    RightBracket,       // ]
    LeftDoubleBracket,  // [[
    RightDoubleBracket, // ]]
    LeftParen,          // (
    RightParen,         // )
    Comma,              // ,

    // Operators
    Plus,      // +
    Minus,     // -
    Ampersand, // &
    Pipe,      // |
    Caret,     // ^

    // Identifiers (for function names)
    Identifier(String), // invert, transpose, etc.

    // End of input
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Note(note) => write!(f, "{}", note),
            Token::Number(num) => write!(f, "{}", num),
            Token::LeftBracket => write!(f, "["),
            Token::RightBracket => write!(f, "]"),
            Token::LeftDoubleBracket => write!(f, "[["),
            Token::RightDoubleBracket => write!(f, "]]"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Ampersand => write!(f, "&"),
            Token::Pipe => write!(f, "|"),
            Token::Caret => write!(f, "^"),
            Token::Identifier(name) => write!(f, "{}", name),
            Token::Eof => write!(f, "EOF"),
        }
    }
}

/// Tokenizes input strings into tokens
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
}

impl Lexer {
    /// Create a new lexer for the given input
    pub fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let current_char = chars.get(0).copied();

        Lexer {
            input: chars,
            position: 0,
            current_char,
        }
    }

    /// Advance to the next character
    fn advance(&mut self) {
        self.position += 1;
        self.current_char = self.input.get(self.position).copied();
    }

    /// Peek at the next character without advancing
    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    /// Skip whitespace characters
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Read a number (can be negative)
    fn read_number(&mut self) -> Result<i8> {
        let mut result = String::new();

        // Handle negative numbers
        if self.current_char == Some('-') {
            result.push('-');
            self.advance();
        }

        // Read digits
        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if result.is_empty() || result == "-" {
            return Err(anyhow!("Invalid number"));
        }

        result
            .parse::<i8>()
            .map_err(|_| anyhow!("Number out of range: {}", result))
    }

    /// Read an identifier or note name (now supports digits, dashes, underscores)
    fn read_identifier(&mut self) -> String {
        let mut result = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_alphanumeric() || ch == '#' || ch == 'b' || ch == '_' || ch == '-' {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        result
    }

    fn _read_identifier(&mut self) -> String {
        let mut result = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_alphabetic()
                || ch == '#'
                || ch == 'b'
                || ch == '_'
                || ch == '-'
                || ch.is_ascii_digit()
            {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        result
    }

    /// Determine if a string is a note name
    fn is_note(s: &str) -> bool {
        if s.len() < 1 || s.len() > 2 {
            return false;
        }

        let first_char = s.chars().next().unwrap();
        if !matches!(first_char, 'A'..='G') {
            return false;
        }

        if s.len() == 2 {
            let second_char = s.chars().nth(1).unwrap();
            matches!(second_char, '#' | 'b')
        } else {
            true
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Token> {
        loop {
            self.skip_whitespace();

            match self.current_char {
                None => return Ok(Token::Eof),

                Some('[') => {
                    // Check if this is [[ (double bracket for progressions)
                    if self.peek() == Some('[') {
                        self.advance(); // consume first [
                        self.advance(); // consume second [
                        return Ok(Token::LeftDoubleBracket);
                    } else {
                        self.advance();
                        return Ok(Token::LeftBracket);
                    }
                }

                Some(']') => {
                    // Look ahead to see if this is ]]
                    if self.peek() == Some(']') {
                        // Only consume both if it's actually ]]
                        self.advance(); // consume first ]
                        self.advance(); // consume second ]
                        return Ok(Token::RightDoubleBracket);
                    } else {
                        // Just a single ]
                        self.advance();
                        return Ok(Token::RightBracket);
                    }
                }

                Some('(') => {
                    self.advance();
                    return Ok(Token::LeftParen);
                }

                Some(')') => {
                    self.advance();
                    return Ok(Token::RightParen);
                }

                Some(',') => {
                    self.advance();
                    return Ok(Token::Comma);
                }

                Some('+') => {
                    self.advance();
                    return Ok(Token::Plus);
                }

                Some('-') => {
                    // Check if this is part of an identifier (like "12-bar-blues")
                    if let Some(next_ch) = self.peek() {
                        if next_ch.is_alphanumeric() {
                            // Check if there's a previous character that's alphanumeric
                            let is_continuation = if self.position > 0 {
                                self.input[self.position - 1].is_alphanumeric()
                            } else {
                                false
                            };

                            if is_continuation {
                                // It's part of an identifier, read the whole thing
                                // We need to back up and re-read from the start of the identifier
                                // This is a bit tricky, so let's use a different approach
                                // Just read from current position as part of identifier
                                let identifier = self.read_identifier();
                                return Ok(Token::Identifier(identifier));
                            }
                        }

                        // Check if it's a negative number
                        if next_ch.is_ascii_digit() {
                            return Ok(Token::Number(self.read_number()?));
                        }
                    }

                    // It's a minus operator
                    self.advance();
                    return Ok(Token::Minus);
                }

                Some('&') => {
                    self.advance();
                    return Ok(Token::Ampersand);
                }

                Some('|') => {
                    self.advance();
                    return Ok(Token::Pipe);
                }

                Some('^') => {
                    self.advance();
                    return Ok(Token::Caret);
                }

                // Handle any alphanumeric character (including digits)
                Some(ch) if ch.is_alphanumeric() => {
                    let identifier = self.read_identifier();

                    // Check if it's a pure number
                    if identifier.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(num) = identifier.parse::<i8>() {
                            return Ok(Token::Number(num));
                        } else {
                            // Number too large, treat as identifier
                            return Ok(Token::Identifier(identifier));
                        }
                    }

                    // Check if it's a note
                    if Self::is_note(&identifier) {
                        return Ok(Token::Note(identifier));
                    } else {
                        return Ok(Token::Identifier(identifier));
                    }
                }

                Some(ch) => {
                    return Err(anyhow!("Unexpected character: '{}'", ch));
                }
            }
        }
    }

    /// Tokenize the entire input into a vector of tokens
    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token, Token::Eof);
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lexer = Lexer::new("[](),+-&|^");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftBracket,
                Token::RightBracket,
                Token::LeftParen,
                Token::RightParen,
                Token::Comma,
                Token::Plus,
                Token::Minus,
                Token::Ampersand,
                Token::Pipe,
                Token::Caret,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_double_brackets() {
        let mut lexer = Lexer::new("[[]]");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftDoubleBracket,
                Token::RightDoubleBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_mixed_brackets() {
        let mut lexer = Lexer::new("[ [[ ]] ]");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftBracket,
                Token::LeftDoubleBracket,
                Token::RightDoubleBracket,
                Token::RightBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_notes() {
        let mut lexer = Lexer::new("C F# Bb A");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Note("C".to_string()),
                Token::Note("F#".to_string()),
                Token::Note("Bb".to_string()),
                Token::Note("A".to_string()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_numbers() {
        let mut lexer = Lexer::new("2 -5 12");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Number(2),
                Token::Number(-5),
                Token::Number(12),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_chord_literal() {
        let mut lexer = Lexer::new("[C, E, G]");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_progression_literal() {
        let mut lexer = Lexer::new("[[C, E, G], [F, A, C]]");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftDoubleBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::Comma,
                Token::LeftBracket,
                Token::Note("F".to_string()),
                Token::Comma,
                Token::Note("A".to_string()),
                Token::Comma,
                Token::Note("C".to_string()),
                Token::RightDoubleBracket, // Changed: Remove RightBracket here
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_transpose_expression() {
        let mut lexer = Lexer::new("[C, E, G] + 2");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::Plus,
                Token::Number(2),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_progression_transpose() {
        let mut lexer = Lexer::new("[[C, E, G], [F, A, C]] + 2");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftDoubleBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::Comma,
                Token::LeftBracket,
                Token::Note("F".to_string()),
                Token::Comma,
                Token::Note("A".to_string()),
                Token::Comma,
                Token::Note("C".to_string()),
                // Token::RightBracket,
                Token::RightDoubleBracket,
                Token::Plus,
                Token::Number(2),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_function_call() {
        let mut lexer = Lexer::new("invert([C, E, G])");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Identifier("invert".to_string()),
                Token::LeftParen,
                Token::LeftBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::RightParen,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_progression_function_call() {
        let mut lexer = Lexer::new("map(invert, [[C, E, G], [F, A, C]])");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens[0..3],
            [
                Token::Identifier("map".to_string()),
                Token::LeftParen,
                Token::Identifier("invert".to_string()),
            ]
        );

        // Check that we have double brackets in the token stream
        assert!(tokens.contains(&Token::LeftDoubleBracket));
        assert!(tokens.contains(&Token::RightDoubleBracket));
    }

    #[test]
    fn test_set_operations() {
        let mut lexer = Lexer::new("[C, E, G] & [A, C, E]");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::Ampersand,
                Token::LeftBracket,
                Token::Note("A".to_string()),
                Token::Comma,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::RightBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_whitespace_handling() {
        let mut lexer = Lexer::new("  [[ C , E , G ] , [ F , A , C ]]  + 2  ");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::LeftDoubleBracket);
        assert_eq!(tokens[1], Token::Note("C".to_string()));
        assert!(tokens.contains(&Token::RightDoubleBracket));
        assert!(tokens.contains(&Token::Plus));
        assert!(tokens.contains(&Token::Number(2)));
    }

    #[test]
    fn test_minus_vs_negative() {
        let mut lexer = Lexer::new("[C, E, G] - 5");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::LeftBracket,
                Token::Note("C".to_string()),
                Token::Comma,
                Token::Note("E".to_string()),
                Token::Comma,
                Token::Note("G".to_string()),
                Token::RightBracket,
                Token::Minus,
                Token::Number(5),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_invalid_character() {
        let mut lexer = Lexer::new("C @ E");
        let result = lexer.tokenize();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unexpected character")
        );
    }
}
