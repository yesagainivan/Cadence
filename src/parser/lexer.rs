use anyhow::{Result, anyhow};
use std::fmt;

/// Represents different types of tokens in the Cadence language
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Note(String),          // C, F#, Bb
    Number(i32),           // 2, -5, 140, etc. (i32 for tempo support)
    Float(f32),            // 120.0 for tempo
    StringLiteral(String), // "path/to/file.cadence"
    Boolean(bool),         // true, false

    // Delimiters
    LeftBracket,        // [
    RightBracket,       // ]
    LeftDoubleBracket,  // [[
    RightDoubleBracket, // ]]
    LeftParen,          // (
    RightParen,         // )
    LeftBrace,          // {
    RightBrace,         // }
    Comma,              // ,
    Semicolon,          // ;
    Newline,            // significant newline (for statement separation)

    // Operators
    Plus,         // +
    Minus,        // -
    Ampersand,    // &
    Pipe,         // |
    Caret,        // ^
    Equals,       // =
    DoubleEquals, // ==
    NotEquals,    // !=

    // Keywords
    Let,      // let
    Loop,     // loop
    Repeat,   // repeat
    If,       // if
    Else,     // else
    Break,    // break
    Continue, // continue
    Return,   // return
    Play,     // play
    Stop,     // stop
    Tempo,    // tempo
    Volume,   // volume
    Queue,    // queue
    Load,     // load
    Track,    // track
    On,       // on (alias for track)

    // Identifiers (for function names and variables)
    Identifier(String), // invert, transpose, prog, etc.

    // End of input
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Note(note) => write!(f, "{}", note),
            Token::Number(num) => write!(f, "{}", num),
            Token::Float(num) => write!(f, "{}", num),
            Token::StringLiteral(s) => write!(f, "\"{}\"", s),
            Token::Boolean(b) => write!(f, "{}", b),
            Token::LeftBracket => write!(f, "["),
            Token::RightBracket => write!(f, "]"),
            Token::LeftDoubleBracket => write!(f, "[["),
            Token::RightDoubleBracket => write!(f, "]]"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::LeftBrace => write!(f, "{{"),
            Token::RightBrace => write!(f, "}}"),
            Token::Comma => write!(f, ","),
            Token::Semicolon => write!(f, ";"),
            Token::Newline => write!(f, "\\n"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Ampersand => write!(f, "&"),
            Token::Pipe => write!(f, "|"),
            Token::Caret => write!(f, "^"),
            Token::Equals => write!(f, "="),
            Token::DoubleEquals => write!(f, "=="),
            Token::NotEquals => write!(f, "!="),
            Token::Let => write!(f, "let"),
            Token::Loop => write!(f, "loop"),
            Token::Repeat => write!(f, "repeat"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Break => write!(f, "break"),
            Token::Continue => write!(f, "continue"),
            Token::Return => write!(f, "return"),
            Token::Play => write!(f, "play"),
            Token::Stop => write!(f, "stop"),
            Token::Tempo => write!(f, "tempo"),
            Token::Volume => write!(f, "volume"),
            Token::Queue => write!(f, "queue"),
            Token::Load => write!(f, "load"),
            Token::Track => write!(f, "track"),
            Token::On => write!(f, "on"),
            Token::Identifier(name) => write!(f, "{}", name),
            Token::Eof => write!(f, "EOF"),
        }
    }
}

/// Position in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(line: usize, column: usize) -> Self {
        Span { line, column }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// A token with its source position
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

impl SpannedToken {
    pub fn new(token: Token, span: Span) -> Self {
        SpannedToken { token, span }
    }
}

/// Tokenizes input strings into tokens
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
    /// Current line number (1-indexed)
    line: usize,
    /// Current column number (1-indexed)
    column: usize,
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
            line: 1,
            column: 1,
        }
    }

    /// Get the current span (position in source)
    fn current_span(&self) -> Span {
        Span::new(self.line, self.column)
    }

    /// Advance to the next character
    fn advance(&mut self) {
        // Track newlines for line/column counting
        if self.current_char == Some('\n') {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        self.position += 1;
        self.current_char = self.input.get(self.position).copied();
    }

    /// Peek at the next character without advancing
    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    /// Skip horizontal whitespace (spaces and tabs, NOT newlines)
    fn skip_horizontal_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Skip a single-line comment (// to end of line)
    fn skip_single_line_comment(&mut self) {
        // Skip //
        self.advance();
        self.advance();
        // Skip until newline or EOF
        while let Some(ch) = self.current_char {
            if ch == '\n' {
                break; // Don't consume newline - let next_token handle it
            }
            self.advance();
        }
    }

    /// Skip a multi-line comment (/* to */)
    fn skip_multi_line_comment(&mut self) {
        // Skip /*
        self.advance();
        self.advance();
        // Skip until */
        while let Some(ch) = self.current_char {
            if ch == '*' && self.peek() == Some('/') {
                self.advance(); // consume *
                self.advance(); // consume /
                return;
            }
            self.advance();
        }
        // Note: Unterminated comment - we just hit EOF
    }

    /// Read a number (can be negative)
    fn read_number(&mut self) -> Result<i32> {
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
            .parse::<i32>()
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
        if s.is_empty() {
            return false;
        }

        let mut chars = s.chars().peekable();

        // 1. Check note name (A-G)
        let first = chars.next().unwrap();
        if !matches!(first, 'A'..='G') {
            return false;
        }

        // 2. Check optional accidental
        if let Some(&c) = chars.peek() {
            if matches!(c, '#' | 'b' | 's') {
                chars.next();
            }
        }

        // 3. Check optional octave (can be negative)
        // If nothing left, it's a note (default octave)
        if chars.peek().is_none() {
            return true;
        }

        // Check for minus sign
        if let Some(&c) = chars.peek() {
            if c == '-' {
                chars.next();
                // Must be followed by digit
                if chars.peek().is_none() {
                    return false;
                }
            }
        }

        // The rest must be digits
        for c in chars {
            if !c.is_ascii_digit() {
                return false;
            }
        }

        true
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Token> {
        loop {
            self.skip_horizontal_whitespace();

            match self.current_char {
                None => return Ok(Token::Eof),

                // Newline is significant for statement separation
                Some('\n') => {
                    self.advance();
                    return Ok(Token::Newline);
                }

                // Comments
                Some('/') => {
                    match self.peek() {
                        Some('/') => {
                            self.skip_single_line_comment();
                            continue; // Loop back to get next token
                        }
                        Some('*') => {
                            self.skip_multi_line_comment();
                            continue; // Loop back to get next token
                        }
                        _ => {
                            return Err(anyhow!("Unexpected character: '/'"));
                        }
                    }
                }

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

                Some('{') => {
                    self.advance();
                    return Ok(Token::LeftBrace);
                }

                Some('}') => {
                    self.advance();
                    return Ok(Token::RightBrace);
                }

                Some(';') => {
                    self.advance();
                    return Ok(Token::Semicolon);
                }

                Some('=') => {
                    self.advance();
                    if self.current_char == Some('=') {
                        self.advance();
                        return Ok(Token::DoubleEquals);
                    }
                    return Ok(Token::Equals);
                }

                Some('!') => {
                    self.advance();
                    if self.current_char == Some('=') {
                        self.advance();
                        return Ok(Token::NotEquals);
                    }
                    return Err(anyhow!("Expected '=' after '!'"));
                }

                Some('"') => {
                    self.advance(); // consume opening quote
                    let mut s = String::new();
                    while let Some(ch) = self.current_char {
                        if ch == '"' {
                            self.advance(); // consume closing quote
                            return Ok(Token::StringLiteral(s));
                        }
                        s.push(ch);
                        self.advance();
                    }
                    return Err(anyhow!("Unterminated string literal"));
                }

                // Handle any alphanumeric character (including digits)
                Some(ch) if ch.is_alphanumeric() || ch == '_' => {
                    let identifier = self.read_identifier();

                    // Check if it's a pure number (possibly float)
                    if identifier.chars().all(|c| c.is_ascii_digit() || c == '.') {
                        if identifier.contains('.') {
                            if let Ok(num) = identifier.parse::<f32>() {
                                return Ok(Token::Float(num));
                            }
                        } else if let Ok(num) = identifier.parse::<i32>() {
                            return Ok(Token::Number(num));
                        } else {
                            // Number too large, treat as identifier
                            return Ok(Token::Identifier(identifier));
                        }
                    }

                    // Check for keywords
                    let token = match identifier.as_str() {
                        "let" => Token::Let,
                        "loop" => Token::Loop,
                        "repeat" => Token::Repeat,
                        "if" => Token::If,
                        "else" => Token::Else,
                        "break" => Token::Break,
                        "continue" => Token::Continue,
                        "return" => Token::Return,
                        "play" => Token::Play,
                        "stop" => Token::Stop,
                        "tempo" => Token::Tempo,
                        "volume" => Token::Volume,
                        "queue" => Token::Queue,
                        "load" => Token::Load,
                        "track" => Token::Track,
                        "on" => Token::On,
                        "true" => Token::Boolean(true),
                        "false" => Token::Boolean(false),
                        _ => {
                            // Check if it's a note
                            if Self::is_note(&identifier) {
                                Token::Note(identifier)
                            } else {
                                Token::Identifier(identifier)
                            }
                        }
                    };
                    return Ok(token);
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

    /// Get the next token with its span (position info)
    pub fn next_spanned_token(&mut self) -> Result<SpannedToken> {
        // Capture span before consuming any characters
        let span = self.current_span();
        let token = self.next_token()?;
        Ok(SpannedToken::new(token, span))
    }

    /// Tokenize the entire input into a vector of spanned tokens
    pub fn tokenize_spanned(&mut self) -> Result<Vec<SpannedToken>> {
        let mut tokens = Vec::new();

        loop {
            let spanned = self.next_spanned_token()?;
            let is_eof = matches!(spanned.token, Token::Eof);
            tokens.push(spanned);

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
    fn test_octave_notes() {
        let mut lexer = Lexer::new("C4 F#3 Bb2 A-1");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Note("C4".to_string()),
                Token::Note("F#3".to_string()),
                Token::Note("Bb2".to_string()),
                Token::Note("A-1".to_string()),
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

    #[test]
    fn test_single_line_comment() {
        let mut lexer = Lexer::new("C // this is a comment\nE");
        let tokens = lexer.tokenize().unwrap();

        // Should have C, Newline, E, Eof - comment is skipped
        assert_eq!(tokens[0], Token::Note("C".to_string()));
        assert_eq!(tokens[1], Token::Newline);
        assert_eq!(tokens[2], Token::Note("E".to_string()));
        assert_eq!(tokens[3], Token::Eof);
    }

    #[test]
    fn test_multi_line_comment() {
        let mut lexer = Lexer::new("C /* skip\nall\nthis */ E");
        let tokens = lexer.tokenize().unwrap();

        // Should have C, E, Eof - multi-line comment is skipped
        assert_eq!(tokens[0], Token::Note("C".to_string()));
        assert_eq!(tokens[1], Token::Note("E".to_string()));
        assert_eq!(tokens[2], Token::Eof);
    }

    #[test]
    fn test_comment_at_end_of_line() {
        let mut lexer = Lexer::new("[C, E, G] // C major chord");
        let tokens = lexer.tokenize().unwrap();

        // Comment at end is just skipped
        assert!(tokens.contains(&Token::LeftBracket));
        assert!(tokens.contains(&Token::RightBracket));
        assert_eq!(tokens.last(), Some(&Token::Eof));
    }
}
