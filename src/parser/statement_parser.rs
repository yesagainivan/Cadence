//! Statement parser for scripting constructs
//!
//! Parses statements like:
//! - `let prog = ii_V_I(C)`
//! - `play prog loop`
//! - `tempo 120`
//! - `if condition { ... } else { ... }`
//! - `loop { ... }`
//! - `repeat 4 { ... }`

use crate::parser::ast::{Expression, Program, Statement};
use crate::parser::lexer::{Lexer, Span, SpannedToken, Token};
use anyhow::{Result, anyhow};

/// Parses statements and programs (sequences of statements)
pub struct StatementParser {
    tokens: Vec<SpannedToken>,
    position: usize,
}

impl StatementParser {
    /// Create a new statement parser from input string
    pub fn new(input: &str) -> Result<Self> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize_spanned()?;

        Ok(StatementParser {
            tokens,
            position: 0,
        })
    }

    /// Current token
    fn current(&self) -> &Token {
        self.tokens
            .get(self.position)
            .map(|st| &st.token)
            .unwrap_or(&Token::Eof)
    }

    /// Current span (position in source)
    fn current_span(&self) -> Span {
        self.tokens
            .get(self.position)
            .map(|st| st.span)
            .unwrap_or_default()
    }

    /// Peek at the next token (unused but may be needed for future lookahead)
    #[allow(dead_code)]
    fn peek(&self) -> &Token {
        self.tokens
            .get(self.position + 1)
            .map(|st| &st.token)
            .unwrap_or(&Token::Eof)
    }

    /// Advance to the next token
    fn advance(&mut self) {
        if self.position < self.tokens.len() {
            self.position += 1;
        }
    }

    /// Expect a specific token
    fn expect(&mut self, expected: &Token) -> Result<()> {
        if self.current() == expected {
            self.advance();
            Ok(())
        } else {
            let span = self.current_span();
            Err(anyhow!(
                "at {}: Expected {:?}, found {:?}",
                span,
                expected,
                self.current()
            ))
        }
    }

    /// Check if current token matches (without consuming)
    fn check(&self, token: &Token) -> bool {
        self.current() == token
    }

    /// Parse a complete program (sequence of statements)
    pub fn parse_program(&mut self) -> Result<Program> {
        let mut program = Program::new();

        while !self.check(&Token::Eof) {
            // Skip semicolons and newlines between statements
            while self.check(&Token::Semicolon) || self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::Eof) {
                break;
            }

            let stmt = self.parse_statement()?;
            program.push(stmt);
        }

        Ok(program)
    }

    /// Parse a single statement
    pub fn parse_statement(&mut self) -> Result<Statement> {
        match self.current().clone() {
            Token::Let => self.parse_let_statement(),
            Token::Play => self.parse_play_statement(),
            Token::Stop => {
                self.advance();
                Ok(Statement::Stop)
            }
            Token::Tempo => self.parse_tempo_statement(),
            Token::Volume => self.parse_volume_statement(),
            Token::Load => self.parse_load_statement(),
            Token::Track => self.parse_track_statement(),
            Token::Loop => self.parse_loop_statement(),
            Token::Repeat => self.parse_repeat_statement(),
            Token::If => self.parse_if_statement(),
            Token::Break => {
                self.advance();
                Ok(Statement::Break)
            }
            Token::Continue => {
                self.advance();
                Ok(Statement::Continue)
            }
            Token::Return => self.parse_return_statement(),
            Token::LeftBrace => self.parse_block_statement(),
            Token::Identifier(name) => {
                // Check if this is an assignment (identifier = expr)
                // Use peek to see if next token is Equals
                if matches!(self.peek(), Token::Equals) {
                    self.advance(); // consume identifier
                    self.advance(); // consume =
                    let value = self.parse_expression()?;
                    Ok(Statement::Assign { name, value })
                } else {
                    // Expression statement
                    let expr = self.parse_expression()?;
                    Ok(Statement::Expression(expr))
                }
            }
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    /// Parse: let <name> = <expression>
    fn parse_let_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Let)?;

        let name = match self.current().clone() {
            Token::Identifier(name) => name,
            _ => return Err(anyhow!("Expected identifier after 'let'")),
        };
        self.advance();

        self.expect(&Token::Equals)?;

        let value = self.parse_expression()?;

        Ok(Statement::Let { name, value })
    }

    /// Parse: play <expression> [loop] [queue] [duration <n>]
    fn parse_play_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Play)?;

        let target = self.parse_expression()?;

        let mut looping = false;
        let mut queue = false;
        let mut duration = None;

        // Parse modifiers
        loop {
            match self.current() {
                Token::Loop => {
                    self.advance();
                    looping = true;
                }
                Token::Queue => {
                    self.advance();
                    queue = true;
                }
                Token::Identifier(name) if name == "duration" => {
                    self.advance();
                    match self.current() {
                        Token::Float(f) => {
                            duration = Some(*f);
                            self.advance();
                        }
                        Token::Number(n) => {
                            duration = Some(*n as f32);
                            self.advance();
                        }
                        _ => return Err(anyhow!("Expected number after 'duration'")),
                    }
                }
                _ => break,
            }
        }

        Ok(Statement::Play {
            target,
            looping,
            queue,
            duration,
        })
    }

    /// Parse: tempo <number>
    fn parse_tempo_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Tempo)?;

        let bpm = match self.current() {
            Token::Float(f) => *f,
            Token::Number(n) => *n as f32,
            _ => return Err(anyhow!("Expected number after 'tempo'")),
        };
        self.advance();

        Ok(Statement::Tempo(bpm))
    }

    /// Parse: volume <number>
    fn parse_volume_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Volume)?;

        let vol = match self.current() {
            Token::Float(f) => *f,
            Token::Number(n) => *n as f32 / 100.0, // Assume 0-100 range
            _ => return Err(anyhow!("Expected number after 'volume'")),
        };
        self.advance();

        Ok(Statement::Volume(vol))
    }

    /// Parse: load "path/to/file.cadence"
    fn parse_load_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Load)?;

        let path = match self.current().clone() {
            Token::StringLiteral(s) => s,
            _ => return Err(anyhow!("Expected string after 'load'")),
        };
        self.advance();

        Ok(Statement::Load(path))
    }

    /// Parse: track <n> <statement> (or block)
    fn parse_track_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Track)?;

        let id = match self.current() {
            Token::Number(n) if *n > 0 => *n as usize,
            Token::Number(_) => return Err(anyhow!("Track ID must be positive")),
            _ => return Err(anyhow!("Expected track number after 'track'")),
        };
        self.advance();

        let body = self.parse_statement()?;

        Ok(Statement::Track {
            id,
            body: Box::new(body),
        })
    }

    /// Parse: loop { statements }
    fn parse_loop_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Loop)?;
        let body = self.parse_block()?;
        Ok(Statement::Loop { body })
    }

    /// Parse: repeat <n> { statements }
    fn parse_repeat_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Repeat)?;

        let count = match self.current() {
            Token::Number(n) if *n >= 0 => *n as u32,
            _ => return Err(anyhow!("Expected positive number after 'repeat'")),
        };
        self.advance();

        let body = self.parse_block()?;

        Ok(Statement::Repeat { count, body })
    }

    /// Parse: if <condition> { statements } [else { statements }]
    fn parse_if_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::If)?;

        let condition = self.parse_expression()?;
        let then_body = self.parse_block()?;

        let else_body = if self.check(&Token::Else) {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Statement::If {
            condition,
            then_body,
            else_body,
        })
    }

    /// Parse: return [expression]
    fn parse_return_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Return)?;

        // Check if there's an expression following
        if self.check(&Token::Semicolon)
            || self.check(&Token::Newline)
            || self.check(&Token::RightBrace)
            || self.check(&Token::Eof)
        {
            Ok(Statement::Return(None))
        } else {
            let expr = self.parse_expression()?;
            Ok(Statement::Return(Some(expr)))
        }
    }

    /// Parse: { statements }
    fn parse_block_statement(&mut self) -> Result<Statement> {
        let body = self.parse_block()?;
        Ok(Statement::Block(body))
    }

    /// Parse a block: { statement* }
    fn parse_block(&mut self) -> Result<Vec<Statement>> {
        self.expect(&Token::LeftBrace)?;

        let mut statements = Vec::new();

        while !self.check(&Token::RightBrace) && !self.check(&Token::Eof) {
            // Skip semicolons between statements
            while self.check(&Token::Semicolon) || self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::RightBrace) {
                break;
            }

            statements.push(self.parse_statement()?);
        }

        self.expect(&Token::RightBrace)?;

        Ok(statements)
    }

    // =========================================================================
    // Expression Parsing (integrated - no string reconstruction)
    // =========================================================================

    /// Parse an expression (handles operator precedence)
    /// Grammar: expression = set_expr
    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_set_expression()
    }

    /// Parse set operations (&, |, ^) - lowest precedence
    /// Grammar: set_expr = additive_expr (('&' | '|' | '^') additive_expr)*
    fn parse_set_expression(&mut self) -> Result<Expression> {
        let mut left = self.parse_additive_expression()?;

        while matches!(
            self.current(),
            Token::Ampersand | Token::Pipe | Token::Caret
        ) {
            let op = self.current().clone();
            self.advance();
            let right = self.parse_additive_expression()?;

            left = match op {
                Token::Ampersand => Expression::intersection(left, right),
                Token::Pipe => Expression::union(left, right),
                Token::Caret => Expression::difference(left, right),
                _ => unreachable!(),
            };
        }

        Ok(left)
    }

    /// Parse additive operations (+, -) - higher precedence than sets
    /// Grammar: additive_expr = primary_expr (('+' | '-') number)?
    fn parse_additive_expression(&mut self) -> Result<Expression> {
        let mut expr = self.parse_primary_expression()?;

        if matches!(self.current(), Token::Plus | Token::Minus) {
            let is_plus = matches!(self.current(), Token::Plus);
            self.advance();

            if let Token::Number(semitones) = self.current() {
                let semitones = *semitones;
                self.advance();
                let semitones = if is_plus {
                    semitones as i8
                } else {
                    -(semitones as i8)
                };
                expr = Expression::transpose(expr, semitones);
            } else {
                return Err(anyhow!("Expected number after +/- operator"));
            }
        }

        Ok(expr)
    }

    /// Parse primary expressions (notes, chords, progressions, function calls)
    fn parse_primary_expression(&mut self) -> Result<Expression> {
        match self.current().clone() {
            Token::Note(note_str) => {
                let note: crate::types::Note = note_str
                    .parse()
                    .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
                self.advance();
                Ok(Expression::Note(note))
            }

            Token::LeftBracket => self.parse_expr_chord(),

            Token::LeftDoubleBracket => self.parse_expr_progression(),

            Token::Number(num) => {
                let name = num.to_string();
                self.advance();
                // If followed by LeftParen, it's a function call
                if matches!(self.current(), Token::LeftParen) {
                    self.parse_function_call(name)
                } else {
                    Err(anyhow!(
                        "Unexpected number: {} (did you mean {}(key)?)",
                        name,
                        name
                    ))
                }
            }

            Token::Identifier(name) => {
                self.advance();
                // Check if this is a function call (has parentheses) or variable
                if matches!(self.current(), Token::LeftParen) {
                    self.parse_function_call(name)
                } else {
                    Ok(Expression::Variable(name))
                }
            }

            Token::LeftParen => {
                self.advance(); // consume '('
                let expr = self.parse_expression()?;
                self.expect(&Token::RightParen)?;
                Ok(expr)
            }

            token => {
                let span = self.current_span();
                Err(anyhow!(
                    "at {}: Unexpected token in expression: {:?}",
                    span,
                    token
                ))
            }
        }
    }

    /// Parse a chord literal: [C, E, G]
    fn parse_expr_chord(&mut self) -> Result<Expression> {
        self.expect(&Token::LeftBracket)?;

        let mut notes = Vec::new();

        // Parse first note
        if let Token::Note(note_str) = self.current().clone() {
            let note: crate::types::Note = note_str
                .parse()
                .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
            notes.push(note);
            self.advance();
        } else {
            return Err(anyhow!(
                "Expected note in chord, found {:?}",
                self.current()
            ));
        }

        // Parse remaining notes
        while matches!(self.current(), Token::Comma) {
            self.advance(); // consume ','

            if let Token::Note(note_str) = self.current().clone() {
                let note: crate::types::Note = note_str
                    .parse()
                    .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
                notes.push(note);
                self.advance();
            } else {
                return Err(anyhow!(
                    "Expected note after comma, found {:?}",
                    self.current()
                ));
            }
        }

        self.expect(&Token::RightBracket)?;
        Ok(Expression::Chord(crate::types::Chord::from_notes(notes)))
    }

    /// Parse a progression literal: [[C, E, G], [F, A, C]]
    fn parse_expr_progression(&mut self) -> Result<Expression> {
        self.expect(&Token::LeftDoubleBracket)?;

        let mut chords = Vec::new();

        // Parse first chord contents directly (after [[)
        let first_chord = self.parse_chord_contents()?;
        chords.push(first_chord);

        // Parse remaining chords
        while matches!(self.current(), Token::Comma) {
            self.advance(); // consume ','
            self.expect(&Token::LeftBracket)?;
            let chord = self.parse_chord_contents()?;
            chords.push(chord);
        }

        self.expect(&Token::RightDoubleBracket)?;
        Ok(Expression::Progression(
            crate::types::Progression::from_chords(chords),
        ))
    }

    /// Parse chord contents (notes only, no brackets)
    fn parse_chord_contents(&mut self) -> Result<crate::types::Chord> {
        let mut notes = Vec::new();

        // Parse first note
        if let Token::Note(note_str) = self.current().clone() {
            let note: crate::types::Note = note_str
                .parse()
                .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
            notes.push(note);
            self.advance();
        } else {
            return Err(anyhow!(
                "Expected note in chord, found {:?}",
                self.current()
            ));
        }

        // Parse remaining notes
        while matches!(self.current(), Token::Comma) {
            self.advance();

            if let Token::Note(note_str) = self.current().clone() {
                let note: crate::types::Note = note_str
                    .parse()
                    .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
                notes.push(note);
                self.advance();
            } else {
                return Err(anyhow!(
                    "Expected note after comma, found {:?}",
                    self.current()
                ));
            }
        }

        // Handle ] or ]] end
        if matches!(self.current(), Token::RightBracket) {
            self.advance();
        }
        // If it's RightDoubleBracket, don't consume - let parse_expr_progression handle it

        Ok(crate::types::Chord::from_notes(notes))
    }

    /// Parse a function call: invert([C, E, G]) or ii_V_I(C)
    fn parse_function_call(&mut self, name: String) -> Result<Expression> {
        self.expect(&Token::LeftParen)?;

        let mut args = Vec::new();

        // Handle empty argument list
        if matches!(self.current(), Token::RightParen) {
            self.advance();
            return Ok(Expression::function_call(name, args));
        }

        // Parse first argument
        args.push(self.parse_expression()?);

        // Parse remaining arguments
        while matches!(self.current(), Token::Comma) {
            self.advance(); // consume ','
            args.push(self.parse_expression()?);
        }

        self.expect(&Token::RightParen)?;
        Ok(Expression::function_call(name, args))
    }
}

/// Convenience function to parse a string into statements
pub fn parse_statements(input: &str) -> Result<Program> {
    let mut parser = StatementParser::new(input)?;
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_let_statement() {
        let program = parse_statements("let prog = [C, E, G]").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Let { name, value: _ } => {
                assert_eq!(name, "prog");
            }
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_parse_tempo_statement() {
        let program = parse_statements("tempo 120").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Tempo(bpm) => {
                assert_eq!(*bpm, 120.0);
            }
            _ => panic!("Expected Tempo statement"),
        }
    }

    #[test]
    fn test_parse_stop_statement() {
        let program = parse_statements("stop").unwrap();
        assert_eq!(program.statements.len(), 1);
        assert!(matches!(&program.statements[0], Statement::Stop));
    }

    #[test]
    fn test_parse_expression_statement() {
        let program = parse_statements("[C, E, G]").unwrap();
        assert_eq!(program.statements.len(), 1);

        assert!(matches!(&program.statements[0], Statement::Expression(_)));
    }

    #[test]
    fn test_parse_multiple_statements() {
        let program = parse_statements("tempo 120; [C, E, G]").unwrap();
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn test_parse_load_statement() {
        let program = parse_statements("load \"song.cadence\"").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Load(path) => {
                assert_eq!(path, "song.cadence");
            }
            _ => panic!("Expected Load statement"),
        }
    }
}
