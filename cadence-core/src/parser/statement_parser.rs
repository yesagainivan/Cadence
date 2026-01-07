//! Statement parser for scripting constructs
//!
//! Parses statements like:
//! - `let prog = ii_V_I(C)`
//! - `play prog loop`
//! - `tempo 120`
//! - `if condition { ... } else { ... }`
//! - `loop { ... }`
//! - `repeat 4 { ... }`

use crate::parser::ast::{
    ComparisonOp, Expression, Program, SpannedProgram, SpannedStatement, Statement,
};
use crate::parser::error::CadenceError;
use crate::parser::lexer::{Lexer, Span, SpannedToken, Token};
// use anyhow::Result; // Removed anyhow dependency

/// Parses statements and programs (sequences of statements)
pub struct StatementParser {
    tokens: Vec<SpannedToken>,
    position: usize,
}

impl StatementParser {
    /// Create a new statement parser from input string
    pub fn new(input: &str) -> Result<Self, CadenceError> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize_spanned().map_err(|e| {
            // Map lexer error to CadenceError (using default span for now)
            CadenceError::new(e.to_string(), Span::default())
        })?;

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

    /// Check if current token should be skipped (whitespace-like tokens)
    fn is_skippable(&self) -> bool {
        matches!(
            self.current(),
            Token::Semicolon | Token::Newline | Token::Comment(_)
        )
    }

    /// Advance to the next token
    fn advance(&mut self) {
        if self.position < self.tokens.len() {
            self.position += 1;
        }
    }

    /// Get the end offset of the previous token (for accurate span tracking)
    /// Returns the offset AFTER the last character of the previous token
    fn previous_token_end(&self) -> usize {
        if self.position == 0 {
            return 0;
        }
        if let Some(prev_token) = self.tokens.get(self.position - 1) {
            // Use span.end() which is calculated from actual lexing
            prev_token.span.end()
        } else {
            self.current_span().offset
        }
    }

    /// Get the UTF-16 end offset of the previous token
    fn previous_token_utf16_end(&self) -> usize {
        if self.position == 0 {
            return 0;
        }
        if let Some(prev_token) = self.tokens.get(self.position - 1) {
            prev_token.span.utf16_offset + prev_token.span.utf16_len
        } else {
            self.current_span().utf16_offset
        }
    }

    /// Get approximate text length of a token (for span calculation)
    /// Note: Not currently used since Span now has exact `len` field from lexer,
    /// but kept for potential debugging/alternative implementations.
    #[allow(dead_code)]
    fn token_text_len(token: &Token) -> usize {
        match token {
            Token::Let => 3,
            Token::Play => 4,
            Token::Stop => 4,
            Token::Loop => 4,
            Token::Repeat => 6,
            Token::If => 2,
            Token::Else => 4,
            Token::Break => 5,
            Token::Continue => 8,
            Token::Return => 6,
            Token::Track => 5,
            Token::On => 2,
            Token::Tempo => 5,
            Token::Volume => 6,
            Token::Waveform => 8,
            Token::Load => 4,
            Token::Fn => 2,
            Token::Queue => 5,
            Token::Identifier(s) => s.len(),
            Token::Number(n) => n.to_string().len(),
            Token::Float(f) => format!("{}", f).len(),
            Token::Note(s) => s.len(),
            Token::StringLiteral(s) => s.len() + 2, // Include quotes
            Token::Comment(s) => s.len() + 2,       // Include //
            Token::LeftParen | Token::RightParen => 1,
            Token::LeftBracket | Token::RightBracket => 1,
            Token::LeftBrace | Token::RightBrace => 1,
            Token::LeftDoubleBracket | Token::RightDoubleBracket => 2,
            Token::Comma | Token::Dot => 1,
            Token::Equals
            | Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent => 1,
            Token::Ampersand | Token::Pipe | Token::Caret => 1,
            Token::Semicolon | Token::Newline => 1,
            Token::Boolean(_) => 5,                      // "true" or "false"
            Token::DoubleEquals | Token::NotEquals => 2, // == or !=
            Token::Less | Token::Greater | Token::Not => 1, // <, >, !
            Token::LessEqual
            | Token::GreaterEqual
            | Token::And
            | Token::Or
            | Token::DotDot
            | Token::Arrow
            | Token::In => 2, // <=, >=, &&, ||, .., ->, in
            Token::For => 3,
            Token::Wait => 4,
            Token::Use => 3,
            Token::From => 4,
            Token::As => 2,
            Token::Eof => 0,
        }
    }

    /// Expect a specific token
    fn expect(&mut self, expected: &Token) -> Result<(), CadenceError> {
        if self.current() == expected {
            self.advance();
            Ok(())
        } else {
            let span = self.current_span();
            Err(CadenceError::new(
                format!("Expected {:?}, found {:?}", expected, self.current()),
                span,
            ))
        }
    }

    /// Check if current token matches (without consuming)
    fn check(&self, token: &Token) -> bool {
        self.current() == token
    }

    /// Parse a complete program (sequence of statements)
    pub fn parse_program(&mut self) -> Result<Program, CadenceError> {
        let mut program = Program::new();

        while !self.check(&Token::Eof) {
            // Skip semicolons, newlines, and comments between statements
            while self.is_skippable() {
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

    /// Parse a complete program with source location tracking for each statement
    pub fn parse_spanned_program(&mut self) -> Result<SpannedProgram, CadenceError> {
        let mut program = SpannedProgram::new();

        while !self.check(&Token::Eof) {
            // Collect doc comments (/// lines) before the statement
            let mut doc_lines: Vec<String> = Vec::new();

            // Skip semicolons, newlines, and regular comments; collect doc comments
            loop {
                match self.current() {
                    Token::Semicolon | Token::Newline => {
                        self.advance();
                    }
                    Token::Comment(text) => {
                        // Doc comments start with / (making ///)
                        if let Some(stripped) = text.strip_prefix('/') {
                            // Strip the leading / and optional space
                            let doc_text = stripped.trim_start();
                            doc_lines.push(doc_text.to_string());
                        }
                        // Skip all comments (doc or regular)
                        self.advance();
                    }
                    _ => break,
                }
            }

            if self.check(&Token::Eof) {
                break;
            }

            // Record start position (both char and UTF-16)
            let start_span = self.current_span();
            let start = start_span.offset;
            let utf16_start = start_span.utf16_offset;

            let stmt = self.parse_statement()?;

            // Record end position as the end of the last consumed token
            // This ensures the span covers all characters of the statement
            let end = self.previous_token_end();
            let utf16_end = self.previous_token_utf16_end();

            // Build doc comment from collected lines (join with newlines)
            let doc_comment = if doc_lines.is_empty() {
                None
            } else {
                Some(doc_lines.join("\n"))
            };

            program.push(
                SpannedStatement::with_utf16(stmt, start, end, utf16_start, utf16_end)
                    .with_doc_comment(doc_comment),
            );
        }

        Ok(program)
    }

    /// Parse a single statement
    pub fn parse_statement(&mut self) -> Result<Statement, CadenceError> {
        match self.current().clone() {
            Token::Let => self.parse_let_statement(),
            Token::Play => self.parse_play_statement(),
            Token::Stop => {
                self.advance();
                Ok(Statement::Stop)
            }
            Token::Tempo => self.parse_tempo_statement(),
            Token::Volume => self.parse_volume_statement(),
            Token::Waveform => self.parse_waveform_statement(),
            Token::Load => self.parse_load_statement(),
            Token::Use => self.parse_use_statement(),
            Token::Fn => self.parse_function_def(),
            Token::Track => self.parse_track_statement(),
            Token::On => self.parse_track_statement(), // 'on N' is alias for 'track N'
            Token::Loop => self.parse_loop_statement(),
            Token::Repeat => self.parse_repeat_statement(),
            Token::For => self.parse_for_statement(),
            Token::Wait => self.parse_wait_statement(),
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
    fn parse_let_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Let)?;

        let name = match self.current().clone() {
            Token::Identifier(name) => name,
            _ => {
                return Err(CadenceError::new(
                    "Expected identifier after 'let'".to_string(),
                    self.current_span(),
                ))
            }
        };
        self.advance();

        self.expect(&Token::Equals)?;

        let value = self.parse_expression()?;

        Ok(Statement::Let { name, value })
    }

    /// Parse: play <expression> [loop] [queue [beat|bar|cycle]] [duration <n>]
    fn parse_play_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Play)?;

        let target = self.parse_expression()?;

        let mut looping = false;
        let mut queue_mode = None;
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
                    // Check for optional mode: beat, bar, cycle, or number (beats:N)
                    match self.current() {
                        Token::Identifier(mode) => {
                            match mode.as_str() {
                                "beat" | "bar" | "cycle" => {
                                    queue_mode = Some(mode.clone());
                                    self.advance();
                                }
                                _ => {
                                    // Not a mode, default to beat
                                    queue_mode = Some("beat".to_string());
                                }
                            }
                        }
                        Token::Number(n) => {
                            // queue N means wait N beats
                            queue_mode = Some(format!("beats:{}", n));
                            self.advance();
                        }
                        _ => {
                            // No mode specified, default to beat
                            queue_mode = Some("beat".to_string());
                        }
                    }
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
                        _ => {
                            return Err(CadenceError::new(
                                "Expected number after 'duration'".to_string(),
                                self.current_span(),
                            ))
                        }
                    }
                }
                _ => break,
            }
        }

        Ok(Statement::Play {
            target,
            looping,
            queue_mode,
            duration,
        })
    }

    /// Parse: tempo <expression>
    fn parse_tempo_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Tempo)?;
        let expr = self.parse_expression()?;
        Ok(Statement::Tempo(expr))
    }

    /// Parse: volume <expression>
    fn parse_volume_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Volume)?;
        let expr = self.parse_expression()?;
        Ok(Statement::Volume(expr))
    }

    /// Parse: waveform "sine" | "saw" | "square" | "triangle"
    fn parse_waveform_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Waveform)?;

        let name = match self.current().clone() {
            Token::StringLiteral(s) => s,
            Token::Identifier(s) => s, // Also allow: waveform sine (without quotes)
            _ => {
                return Err(CadenceError::new(
                    "Expected waveform name (sine, saw, square, triangle)".to_string(),
                    self.current_span(),
                ));
            }
        };
        self.advance();

        Ok(Statement::Waveform(name))
    }

    /// Parse: load "path/to/file.cadence"
    fn parse_load_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Load)?;

        let path = match self.current().clone() {
            Token::StringLiteral(s) => s,
            _ => {
                return Err(CadenceError::new(
                    "Expected string after 'load'".to_string(),
                    self.current_span(),
                ))
            }
        };
        self.advance();

        Ok(Statement::Load(path))
    }

    /// Parse use statement variants:
    /// - use "path/to/file.cadence"
    /// - use "path/to/file.cadence" as alias
    /// - use { name1, name2 } from "path/to/file.cadence"
    /// - use { name1, name2 } from "path/to/file.cadence" as alias
    fn parse_use_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Use)?;

        // Check if it starts with { (selective imports)
        if self.check(&Token::LeftBrace) {
            // use { name1, name2 } from "path"
            self.advance(); // consume {

            let mut imports = Vec::new();

            // Parse first import name
            if !self.check(&Token::RightBrace) {
                match self.current().clone() {
                    Token::Identifier(name) => {
                        imports.push(name);
                        self.advance();
                    }
                    _ => {
                        return Err(CadenceError::new(
                            "Expected identifier in import list".to_string(),
                            self.current_span(),
                        ))
                    }
                }

                // Parse remaining imports
                while self.check(&Token::Comma) {
                    self.advance(); // consume ,
                    match self.current().clone() {
                        Token::Identifier(name) => {
                            imports.push(name);
                            self.advance();
                        }
                        _ => {
                            return Err(CadenceError::new(
                                "Expected identifier after ',' in import list".to_string(),
                                self.current_span(),
                            ))
                        }
                    }
                }
            }

            self.expect(&Token::RightBrace)?;
            self.expect(&Token::From)?;

            // Parse path
            let path = match self.current().clone() {
                Token::StringLiteral(s) => s,
                _ => {
                    return Err(CadenceError::new(
                        "Expected module path string after 'from'".to_string(),
                        self.current_span(),
                    ))
                }
            };
            self.advance();

            // Check for optional alias
            let alias = if self.check(&Token::As) {
                self.advance();
                match self.current().clone() {
                    Token::Identifier(name) => {
                        self.advance();
                        Some(name)
                    }
                    _ => {
                        return Err(CadenceError::new(
                            "Expected identifier after 'as'".to_string(),
                            self.current_span(),
                        ))
                    }
                }
            } else {
                None
            };

            Ok(Statement::Use {
                path,
                imports: Some(imports),
                alias,
            })
        } else {
            // use "path" or use "path" as alias
            let path = match self.current().clone() {
                Token::StringLiteral(s) => s,
                _ => {
                    return Err(CadenceError::new(
                        "Expected module path string after 'use'".to_string(),
                        self.current_span(),
                    ))
                }
            };
            self.advance();

            // Check for optional alias
            let alias = if self.check(&Token::As) {
                self.advance();
                match self.current().clone() {
                    Token::Identifier(name) => {
                        self.advance();
                        Some(name)
                    }
                    _ => {
                        return Err(CadenceError::new(
                            "Expected identifier after 'as'".to_string(),
                            self.current_span(),
                        ))
                    }
                }
            } else {
                None
            };

            Ok(Statement::Use {
                path,
                imports: None,
                alias,
            })
        }
    }

    /// Parse: fn name(param1, param2, ...) { body }
    fn parse_function_def(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Fn)?;

        // Parse function name
        let name = match self.current().clone() {
            Token::Identifier(name) => name,
            _ => {
                return Err(CadenceError::new(
                    "Expected function name after 'fn'".to_string(),
                    self.current_span(),
                ))
            }
        };
        self.advance();

        // Parse parameter list
        self.expect(&Token::LeftParen)?;
        let mut params = Vec::new();

        // Handle empty parameter list
        if !matches!(self.current(), Token::RightParen) {
            // Parse first parameter
            match self.current().clone() {
                Token::Identifier(param) => {
                    params.push(param);
                    self.advance();
                }
                _ => {
                    return Err(CadenceError::new(
                        "Expected parameter name".to_string(),
                        self.current_span(),
                    ))
                }
            }

            // Parse remaining parameters
            while matches!(self.current(), Token::Comma) {
                self.advance(); // consume ','
                match self.current().clone() {
                    Token::Identifier(param) => {
                        params.push(param);
                        self.advance();
                    }
                    _ => {
                        return Err(CadenceError::new(
                            "Expected parameter name after ','".to_string(),
                            self.current_span(),
                        ))
                    }
                }
            }
        }

        self.expect(&Token::RightParen)?;

        // Parse optional return type annotation: -> Type
        let return_type = if matches!(self.current(), Token::Arrow) {
            self.advance(); // consume ->
            match self.current().clone() {
                Token::Identifier(type_name) => {
                    self.advance();
                    Some(type_name)
                }
                _ => {
                    return Err(CadenceError::new(
                        "Expected type name after '->'".to_string(),
                        self.current_span(),
                    ))
                }
            }
        } else {
            None
        };

        // Parse function body
        let body = self.parse_block()?;

        Ok(Statement::FunctionDef {
            name,
            params,
            body,
            return_type,
        })
    }

    /// Parse: track <n> <statement> (or block)
    /// Also handles: on <n> <statement> (alias syntax)
    fn parse_track_statement(&mut self) -> Result<Statement, CadenceError> {
        // Accept either 'track' or 'on' as the prefix
        if self.check(&Token::Track) || self.check(&Token::On) {
            self.advance();
        } else {
            return Err(CadenceError::new(
                "Expected 'track' or 'on' keyword".to_string(),
                self.current_span(),
            ));
        }

        let id = match self.current() {
            Token::Number(n) if *n > 0 => *n as usize,
            Token::Number(_) => {
                return Err(CadenceError::new(
                    "Track ID must be positive".to_string(),
                    self.current_span(),
                ))
            }
            _ => {
                return Err(CadenceError::new(
                    "Expected track number after 'track' or 'on'".to_string(),
                    self.current_span(),
                ))
            }
        };
        self.advance();

        let body = self.parse_statement()?;

        Ok(Statement::Track {
            id,
            body: Box::new(body),
        })
    }

    /// Parse: loop { statements }
    fn parse_loop_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Loop)?;
        let body = self.parse_block()?;
        Ok(Statement::Loop { body })
    }

    /// Parse: repeat <n> { statements }
    fn parse_repeat_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Repeat)?;

        let count = match self.current() {
            Token::Number(n) if *n >= 0 => *n as u32,
            _ => {
                return Err(CadenceError::new(
                    "Expected positive number after 'repeat'".to_string(),
                    self.current_span(),
                ))
            }
        };
        self.advance();

        let body = self.parse_block()?;

        Ok(Statement::Repeat { count, body })
    }

    /// Parse: for <var> in <start>..<end> { statements }
    fn parse_for_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::For)?;

        // Get iteration variable name
        let var = match self.current() {
            Token::Identifier(name) => name.clone(),
            _ => {
                return Err(CadenceError::new(
                    "Expected identifier after 'for'".to_string(),
                    self.current_span(),
                ))
            }
        };
        self.advance();

        self.expect(&Token::In)?;

        // Parse start value
        let start = self.parse_expression()?;

        self.expect(&Token::DotDot)?;

        // Parse end value
        let end = self.parse_expression()?;

        let body = self.parse_block()?;

        Ok(Statement::For {
            var,
            start,
            end,
            body,
        })
    }

    /// Parse: wait <expression>
    /// Advances virtual time by the specified number of beats
    fn parse_wait_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::Wait)?;
        let beats = self.parse_expression()?;
        Ok(Statement::Wait { beats })
    }

    /// Parse: if <condition> { statements } [else if ... | else { statements }]
    fn parse_if_statement(&mut self) -> Result<Statement, CadenceError> {
        self.expect(&Token::If)?;

        let condition = self.parse_expression()?;
        let then_body = self.parse_block()?;

        // Skip newlines/semicolons before checking for else
        while self.is_skippable() {
            self.advance();
        }

        let else_body = if self.check(&Token::Else) {
            self.advance();
            if self.check(&Token::If) {
                // else if → parse as nested if statement
                Some(vec![self.parse_if_statement()?])
            } else {
                Some(self.parse_block()?)
            }
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
    fn parse_return_statement(&mut self) -> Result<Statement, CadenceError> {
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
    fn parse_block_statement(&mut self) -> Result<Statement, CadenceError> {
        let body = self.parse_block()?;
        Ok(Statement::Block(body))
    }

    /// Parse a block: { statement* }
    fn parse_block(&mut self) -> Result<Vec<Statement>, CadenceError> {
        self.expect(&Token::LeftBrace)?;

        let mut statements = Vec::new();

        while !self.check(&Token::RightBrace) && !self.check(&Token::Eof) {
            // Skip semicolons, newlines, and comments between statements
            while self.is_skippable() {
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
    /// Grammar: expression = logical_or_expr
    fn parse_expression(&mut self) -> Result<Expression, CadenceError> {
        self.parse_logical_or_expression()
    }

    /// Parse logical OR (||) - lowest precedence
    /// Grammar: logical_or_expr = logical_and_expr ('||' logical_and_expr)*
    fn parse_logical_or_expression(&mut self) -> Result<Expression, CadenceError> {
        let mut left = self.parse_logical_and_expression()?;

        while matches!(self.current(), Token::Or) {
            self.advance();
            let right = self.parse_logical_and_expression()?;
            left = Expression::LogicalOr {
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse logical AND (&&)
    /// Grammar: logical_and_expr = set_expr ('&&' set_expr)*
    fn parse_logical_and_expression(&mut self) -> Result<Expression, CadenceError> {
        let mut left = self.parse_set_expression()?;

        while matches!(self.current(), Token::And) {
            self.advance();
            let right = self.parse_set_expression()?;
            left = Expression::LogicalAnd {
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse set operations (&, |, ^) - between logical AND and comparison
    /// Grammar: set_expr = comparison_expr (('&' | '|' | '^') comparison_expr)*
    fn parse_set_expression(&mut self) -> Result<Expression, CadenceError> {
        let mut left = self.parse_comparison_expression()?;

        while matches!(
            self.current(),
            Token::Ampersand | Token::Pipe | Token::Caret
        ) {
            let op = self.current().clone();
            self.advance();
            let right = self.parse_comparison_expression()?;

            left = match op {
                Token::Ampersand => Expression::intersection(left, right),
                Token::Pipe => Expression::union(left, right),
                Token::Caret => Expression::difference(left, right),
                _ => unreachable!(),
            };
        }

        Ok(left)
    }

    /// Parse comparison operations (==, !=, <, >, <=, >=)
    /// Grammar: comparison_expr = additive_expr (('==' | '!=' | '<' | '>' | '<=' | '>=') additive_expr)?
    fn parse_comparison_expression(&mut self) -> Result<Expression, CadenceError> {
        let left = self.parse_additive_expression()?;

        if matches!(
            self.current(),
            Token::DoubleEquals
                | Token::NotEquals
                | Token::Less
                | Token::Greater
                | Token::LessEqual
                | Token::GreaterEqual
        ) {
            let op = match self.current() {
                Token::DoubleEquals => ComparisonOp::Equal,
                Token::NotEquals => ComparisonOp::NotEqual,
                Token::Less => ComparisonOp::Less,
                Token::Greater => ComparisonOp::Greater,
                Token::LessEqual => ComparisonOp::LessEqual,
                Token::GreaterEqual => ComparisonOp::GreaterEqual,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_additive_expression()?;

            Ok(Expression::Comparison {
                left: Box::new(left),
                right: Box::new(right),
                operator: op,
            })
        } else {
            Ok(left)
        }
    }

    /// Parse additive operations (+, -) - higher precedence than comparison
    /// Grammar: additive_expr = multiplicative_expr (('+' | '-') multiplicative_expr)*
    ///
    /// This handles two modes:
    /// 1. For notes/chords followed by +/- number: transposition (C + 2 → D)
    /// 2. For numbers: regular arithmetic (3 + 4 → 7)
    fn parse_additive_expression(&mut self) -> Result<Expression, CadenceError> {
        use crate::parser::ast::ArithmeticOp;

        let mut left = self.parse_multiplicative_expression()?;

        while matches!(self.current(), Token::Plus | Token::Minus) {
            let op = match self.current() {
                Token::Plus => ArithmeticOp::Add,
                Token::Minus => ArithmeticOp::Subtract,
                _ => unreachable!(),
            };
            self.advance();

            let right = self.parse_multiplicative_expression()?;

            // Check if this is a transposition case:
            // If left is note/chord/pattern/function-result and right is a simple number
            let is_transposition = match (&left, &right) {
                (
                    Expression::Note(_)
                    | Expression::Pattern(_)
                    | Expression::Variable(_)
                    | Expression::FunctionCall { .. }
                    | Expression::Transpose { .. },
                    Expression::Number(n),
                ) => {
                    // Transposition: note/pattern + number
                    let semitones = match op {
                        ArithmeticOp::Add => *n as i8,
                        ArithmeticOp::Subtract => -(*n as i8),
                        _ => unreachable!(),
                    };
                    left = Expression::transpose(left, semitones);
                    true
                }
                (Expression::Array(_), Expression::Number(n)) => {
                    // Chord transposition: [C, E, G] + 2
                    let semitones = match op {
                        ArithmeticOp::Add => *n as i8,
                        ArithmeticOp::Subtract => -(*n as i8),
                        _ => unreachable!(),
                    };
                    left = Expression::transpose(left, semitones);
                    true
                }
                _ => false,
            };

            if !is_transposition {
                // Regular arithmetic
                left = Expression::BinaryOp {
                    left: Box::new(left),
                    right: Box::new(right),
                    operator: op,
                };
            }
        }

        Ok(left)
    }

    /// Parse multiplicative operations (*, /, %) - higher precedence than additive
    /// Grammar: multiplicative_expr = postfix_expr (('*' | '/' | '%') postfix_expr)*
    fn parse_multiplicative_expression(&mut self) -> Result<Expression, CadenceError> {
        use crate::parser::ast::ArithmeticOp;

        let mut left = self.parse_postfix_expression()?;

        while matches!(self.current(), Token::Star | Token::Slash | Token::Percent) {
            let op = match self.current() {
                Token::Star => ArithmeticOp::Multiply,
                Token::Slash => ArithmeticOp::Divide,
                Token::Percent => ArithmeticOp::Modulo,
                _ => unreachable!(),
            };
            self.advance();

            let right = self.parse_postfix_expression()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                right: Box::new(right),
                operator: op,
            };
        }

        Ok(left)
    }

    /// Parse postfix operations (method calls and indexing)
    /// Grammar: postfix_expr = primary_expr ('.' identifier '(' args ')') | ('[' expr ']'))*
    /// Desugars method calls to function calls: expr.method(a, b) → method(expr, a, b)
    fn parse_postfix_expression(&mut self) -> Result<Expression, CadenceError> {
        let mut expr = self.parse_primary_expression()?;

        // Handle chained method calls and indexing
        loop {
            if matches!(self.current(), Token::Dot) {
                self.advance(); // consume '.'

                let method_name = match self.current().clone() {
                    Token::Identifier(name) => name,
                    _ => {
                        let span = self.current_span();
                        return Err(CadenceError::new(
                            format!("Expected method name after '.', found {:?}", self.current()),
                            span,
                        ));
                    }
                };
                self.advance();

                // Parse method arguments (must have parentheses)
                self.expect(&Token::LeftParen)?;
                let mut args = vec![expr]; // receiver is first argument

                if !matches!(self.current(), Token::RightParen) {
                    args.push(self.parse_expression()?);
                    while matches!(self.current(), Token::Comma) {
                        self.advance();
                        args.push(self.parse_expression()?);
                    }
                }
                self.expect(&Token::RightParen)?;

                // Desugar to function call: receiver.method(a, b) → method(receiver, a, b)
                expr = Expression::function_call(method_name, args);
            } else if matches!(self.current(), Token::LeftBracket) {
                self.advance(); // consume '['

                let index = self.parse_expression()?;

                self.expect(&Token::RightBracket)?;

                // Create Index expression
                expr = Expression::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse primary expressions (notes, chords, progressions, function calls, patterns)
    fn parse_primary_expression(&mut self) -> Result<Expression, CadenceError> {
        // Handle unary NOT first
        if matches!(self.current(), Token::Not) {
            self.advance();
            let expr = self.parse_primary_expression()?;
            return Ok(Expression::LogicalNot(Box::new(expr)));
        }

        match self.current().clone() {
            Token::Note(note_str) => {
                let note: crate::types::Note = note_str.parse().map_err(|e| {
                    CadenceError::new(
                        format!("Invalid note '{}': {}", note_str, e),
                        self.current_span(),
                    )
                })?;
                self.advance();
                Ok(Expression::Note(note))
            }

            Token::StringLiteral(pattern_str) => {
                // Try to parse as pattern, otherwise treat as string literal
                match crate::types::Pattern::parse(&pattern_str) {
                    Ok(pattern) => {
                        self.advance();
                        Ok(Expression::Pattern(pattern))
                    }
                    Err(_) => {
                        // Not a pattern, so it's a string literal
                        let s = pattern_str.clone();
                        self.advance();
                        Ok(Expression::String(s))
                    }
                }
            }

            Token::LeftBracket => self.parse_bracket_expression(),

            Token::LeftDoubleBracket => self.parse_expr_progression(),

            Token::Number(num) => {
                let name = num.to_string();
                let val = num; // Value is already i32, no deref needed
                self.advance();
                // If followed by LeftParen, it's a function call (e.g., 251(C) for progressions)
                if matches!(self.current(), Token::LeftParen) {
                    self.parse_function_call(name)
                } else {
                    // Keep numbers as numbers - don't auto-convert to notes
                    // Notes should be written explicitly as note names (C, D, E, etc.)
                    Ok(Expression::Number(val))
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

            Token::Boolean(b) => {
                self.advance();
                Ok(Expression::Boolean(b))
            }

            Token::LeftParen => {
                self.advance(); // consume '('
                let expr = self.parse_expression()?;
                self.expect(&Token::RightParen)?;
                Ok(expr)
            }

            token => {
                let span = self.current_span();
                Err(CadenceError::new(
                    format!("Unexpected token in expression: {:?}", token),
                    span,
                ))
            }
        }
    }

    /// Parse bracket expression: [expr, expr, ...]
    /// Returns Expression::Array - evaluator decides if it becomes a Chord or Array
    fn parse_bracket_expression(&mut self) -> Result<Expression, CadenceError> {
        self.expect(&Token::LeftBracket)?;

        let mut elements = Vec::new();

        // Handle empty array
        if matches!(self.current(), Token::RightBracket) {
            self.advance();
            return Ok(Expression::Array(elements));
        }

        // Parse first element (any expression)
        elements.push(self.parse_expression()?);

        // Parse remaining elements
        while matches!(self.current(), Token::Comma) {
            self.advance(); // consume ','
            elements.push(self.parse_expression()?);
        }

        self.expect(&Token::RightBracket)?;
        Ok(Expression::Array(elements))
    }

    /// Parse a progression literal: [[C, E, G], [F, A, C]]
    fn parse_expr_progression(&mut self) -> Result<Expression, CadenceError> {
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
        // Create a Pattern directly from the chords
        Ok(Expression::Pattern(crate::types::Pattern::from_chords(
            chords,
        )))
    }

    /// Parse chord contents (notes only, no brackets)
    fn parse_chord_contents(&mut self) -> Result<crate::types::Chord, CadenceError> {
        let mut notes = Vec::new();

        // Parse first note
        if let Token::Note(note_str) = self.current().clone() {
            let note: crate::types::Note = note_str.parse().map_err(|e| {
                CadenceError::new(
                    format!("Invalid note '{}': {}", note_str, e),
                    self.current_span(),
                )
            })?;
            notes.push(note);
            self.advance();
        } else {
            return Err(CadenceError::new(
                format!("Expected note in chord, found {:?}", self.current()),
                self.current_span(),
            ));
        }

        // Parse remaining notes
        while matches!(self.current(), Token::Comma) {
            self.advance();

            if let Token::Note(note_str) = self.current().clone() {
                let note: crate::types::Note = note_str.parse().map_err(|e| {
                    CadenceError::new(
                        format!("Invalid note '{}': {}", note_str, e),
                        self.current_span(),
                    )
                })?;
                notes.push(note);
                self.advance();
            } else {
                return Err(CadenceError::new(
                    format!("Expected note after comma, found {:?}", self.current()),
                    self.current_span(),
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
    fn parse_function_call(
        &mut self,
        name: String,
    ) -> std::result::Result<Expression, CadenceError> {
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
pub fn parse_statements(input: &str) -> std::result::Result<Program, CadenceError> {
    let mut parser = StatementParser::new(input)?;
    parser.parse_program()
}

/// Convenience function to parse a string into statements with source spans
pub fn parse_spanned_statements(input: &str) -> std::result::Result<SpannedProgram, CadenceError> {
    let mut parser = StatementParser::new(input)?;
    parser.parse_spanned_program()
}

/// Convenience function to parse a string into a single expression
pub fn parse_expression(input: &str) -> std::result::Result<Expression, CadenceError> {
    let mut parser = StatementParser::new(input)?;
    parser.parse_expression()
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
            Statement::Tempo(expr) => match expr {
                Expression::Number(n) => assert_eq!(*n as f32, 120.0),
                _ => panic!("Expected Number expression"),
            },
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

    #[test]
    fn test_parse_use_statement_simple() {
        let program = parse_statements(r#"use "drums.cadence""#).unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Use {
                path,
                imports,
                alias,
            } => {
                assert_eq!(path, "drums.cadence");
                assert!(imports.is_none());
                assert!(alias.is_none());
            }
            _ => panic!("Expected Use statement"),
        }
    }

    #[test]
    fn test_parse_use_statement_with_alias() {
        let program = parse_statements(r#"use "drums.cadence" as d"#).unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Use {
                path,
                imports,
                alias,
            } => {
                assert_eq!(path, "drums.cadence");
                assert!(imports.is_none());
                assert_eq!(alias.as_ref().unwrap(), "d");
            }
            _ => panic!("Expected Use statement"),
        }
    }

    #[test]
    fn test_parse_use_statement_selective() {
        let program = parse_statements(r#"use { kick, snare } from "drums.cadence""#).unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Use {
                path,
                imports,
                alias,
            } => {
                assert_eq!(path, "drums.cadence");
                let items = imports.as_ref().unwrap();
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], "kick");
                assert_eq!(items[1], "snare");
                assert!(alias.is_none());
            }
            _ => panic!("Expected Use statement"),
        }
    }

    #[test]
    fn test_parse_use_statement_selective_with_alias() {
        let program = parse_statements(r#"use { kick } from "drums.cadence" as d"#).unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Use {
                path,
                imports,
                alias,
            } => {
                assert_eq!(path, "drums.cadence");
                let items = imports.as_ref().unwrap();
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], "kick");
                assert_eq!(alias.as_ref().unwrap(), "d");
            }
            _ => panic!("Expected Use statement"),
        }
    }

    #[test]
    fn test_parse_if_with_comparison_equal() {
        let program = parse_statements("if true == true { tempo 120 }").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                assert!(matches!(condition, Expression::Comparison { .. }));
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_none());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_if_with_comparison_not_equal() {
        let program = parse_statements("if true != false { play C }").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::If { condition, .. } => {
                if let Expression::Comparison { operator, .. } = condition {
                    assert!(matches!(operator, ComparisonOp::NotEqual));
                } else {
                    panic!("Expected Comparison expression");
                }
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_else_if() {
        let program = parse_statements("if true { tempo 120 } else if false { tempo 60 }").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::If {
                condition: _,
                then_body,
                else_body,
            } => {
                assert_eq!(then_body.len(), 1);
                // else_body should contain a nested If statement
                let else_stmts = else_body.as_ref().expect("Should have else body");
                assert_eq!(else_stmts.len(), 1);
                assert!(matches!(&else_stmts[0], Statement::If { .. }));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_else_if_chain() {
        let code = r#"
            if true { tempo 120 }
            else if false { tempo 100 }
            else { tempo 60 }
        "#;
        let program = parse_statements(code).unwrap();
        assert_eq!(program.statements.len(), 1);

        // Navigate to first else-if
        match &program.statements[0] {
            Statement::If { else_body, .. } => {
                let else_stmts = else_body.as_ref().expect("Should have first else");
                assert_eq!(else_stmts.len(), 1);

                // That else should be another If with its own else
                match &else_stmts[0] {
                    Statement::If { else_body, .. } => {
                        let final_else = else_body.as_ref().expect("Should have final else");
                        assert_eq!(final_else.len(), 1);
                        assert!(matches!(&final_else[0], Statement::Tempo(_)));
                    }
                    _ => panic!("Expected nested If statement"),
                }
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_comparison_less_than() {
        let program = parse_statements("if 1 < 2 { tempo 120 }").unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::If { condition, .. } => {
                if let Expression::Comparison { operator, .. } = condition {
                    assert!(matches!(operator, ComparisonOp::Less));
                } else {
                    panic!("Expected Comparison expression");
                }
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_comparison_greater_equal() {
        let program = parse_statements("if 10 >= 5 { play C }").unwrap();
        match &program.statements[0] {
            Statement::If { condition, .. } => {
                if let Expression::Comparison { operator, .. } = condition {
                    assert!(matches!(operator, ComparisonOp::GreaterEqual));
                } else {
                    panic!("Expected Comparison expression");
                }
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_logical_and() {
        let program = parse_statements("if true && false { tempo 60 }").unwrap();
        match &program.statements[0] {
            Statement::If { condition, .. } => {
                assert!(matches!(condition, Expression::LogicalAnd { .. }));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_logical_or() {
        let program = parse_statements("if true || false { tempo 60 }").unwrap();
        match &program.statements[0] {
            Statement::If { condition, .. } => {
                assert!(matches!(condition, Expression::LogicalOr { .. }));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_logical_not() {
        let program = parse_statements("if !false { play C }").unwrap();
        match &program.statements[0] {
            Statement::If { condition, .. } => {
                assert!(matches!(condition, Expression::LogicalNot(_)));
            }
            _ => panic!("Expected If statement"),
        }
    }
}

#[cfg(test)]
mod expression_tests {
    use super::*;
    use crate::parser::ast::Expression;

    fn parse(input: &str) -> std::result::Result<Expression, CadenceError> {
        parse_expression(input)
    }

    #[test]
    fn test_parse_single_note() {
        let expr = parse("C").unwrap();
        assert!(matches!(expr, Expression::Note(_)));

        if let Expression::Note(note) = expr {
            assert_eq!(note.pitch_class(), 0); // C
        }
    }

    #[test]
    fn test_parse_chord() {
        // Post-refactor: parser returns Expression::Array, evaluator coerces to Chord
        let expr = parse("[C, E, G]").unwrap();
        assert!(matches!(expr, Expression::Array(_)));

        if let Expression::Array(elements) = expr {
            assert_eq!(elements.len(), 3);
            // Each element should be a Note expression
            assert!(elements.iter().all(|e| matches!(e, Expression::Note(_))));
        }
    }

    #[test]
    fn test_parse_progression() {
        let expr = parse("[[C, E, G], [F, A, C]]").unwrap();
        assert!(matches!(expr, Expression::Pattern(_)));

        if let Expression::Pattern(pattern) = expr {
            let chords = pattern.as_chords().expect("Should be chord-only pattern");
            assert_eq!(chords.len(), 2);

            // Test first chord is C major
            let first_chord = &chords[0];
            assert!(first_chord.contains(&"C".parse().unwrap()));
            assert!(first_chord.contains(&"E".parse().unwrap()));
            assert!(first_chord.contains(&"G".parse().unwrap()));

            // Test second chord is F major
            let second_chord = &chords[1];
            assert!(second_chord.contains(&"F".parse().unwrap()));
            assert!(second_chord.contains(&"A".parse().unwrap()));
            assert!(second_chord.contains(&"C".parse().unwrap()));
        }
    }

    #[test]
    fn test_parse_progression_transpose() {
        let expr = parse("[[C, E, G], [F, A, C]] + 2").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));

        if let Expression::Transpose { target, semitones } = expr {
            assert_eq!(semitones, 2);
            assert!(matches!(*target, Expression::Pattern(_)));
        }
    }

    #[test]
    fn test_parse_empty_progression() {
        let result = parse("[[]]");
        assert!(result.is_err()); // Should fail because empty chord is invalid
    }

    #[test]
    fn test_parse_transpose_positive() {
        let expr = parse("[C, E, G] + 2").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));

        if let Expression::Transpose { target, semitones } = expr {
            assert_eq!(semitones, 2);
            assert!(matches!(*target, Expression::Array(_)));
        }
    }

    #[test]
    fn test_parse_transpose_negative() {
        let expr = parse("[C, E, G] - 5").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));

        if let Expression::Transpose { target, semitones } = expr {
            assert_eq!(semitones, -5);
            assert!(matches!(*target, Expression::Array(_)));
        }
    }

    #[test]
    fn test_parse_set_intersection() {
        let expr = parse("[C, E, G] & [A, C, E]").unwrap();
        assert!(matches!(expr, Expression::Intersection { .. }));

        if let Expression::Intersection { left, right } = expr {
            assert!(matches!(*left, Expression::Array(_)));
            assert!(matches!(*right, Expression::Array(_)));
        }
    }

    #[test]
    fn test_parse_set_union() {
        let expr = parse("[C, E, G] | [F, A, C]").unwrap();
        assert!(matches!(expr, Expression::Union { .. }));
    }

    #[test]
    fn test_parse_set_difference() {
        let expr = parse("[C, E, G] ^ [A, C, E]").unwrap();
        assert!(matches!(expr, Expression::Difference { .. }));
    }

    #[test]
    fn test_parse_function_call() {
        let expr = parse("invert([C, E, G])").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "invert");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Expression::Array(_)));
        }
    }

    #[test]
    fn test_parse_function_call_multiple_args() {
        let expr = parse("test(C, [D, F#, A])").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "test");
            assert_eq!(args.len(), 2);
            assert!(matches!(args[0], Expression::Note(_)));
            assert!(matches!(args[1], Expression::Array(_)));
        }
    }

    #[test]
    fn test_parse_parentheses() {
        let expr = parse("([C, E, G] + 2)").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));
    }

    #[test]
    fn test_operator_precedence() {
        // Set operations should have lower precedence than arithmetic
        let expr = parse("[C, E, G] + 2 & [A, C, E]").unwrap();
        assert!(matches!(expr, Expression::Intersection { .. }));

        if let Expression::Intersection { left, right } = expr {
            // Left side should be a transpose operation
            assert!(matches!(*left, Expression::Transpose { .. }));
            assert!(matches!(*right, Expression::Array(_)));
        }
    }

    #[test]
    fn test_parse_arithmetic_multiply() {
        use crate::parser::ast::ArithmeticOp;
        let expr = parse("3 * 4").unwrap();
        assert!(matches!(expr, Expression::BinaryOp { .. }));

        if let Expression::BinaryOp {
            left,
            right,
            operator,
        } = expr
        {
            assert!(matches!(*left, Expression::Number(3)));
            assert!(matches!(*right, Expression::Number(4)));
            assert_eq!(operator, ArithmeticOp::Multiply);
        }
    }

    #[test]
    fn test_parse_arithmetic_divide() {
        use crate::parser::ast::ArithmeticOp;
        let expr = parse("10 / 2").unwrap();
        assert!(matches!(expr, Expression::BinaryOp { .. }));

        if let Expression::BinaryOp {
            left,
            right,
            operator,
        } = expr
        {
            assert!(matches!(*left, Expression::Number(10)));
            assert!(matches!(*right, Expression::Number(2)));
            assert_eq!(operator, ArithmeticOp::Divide);
        }
    }

    #[test]
    fn test_parse_arithmetic_modulo() {
        use crate::parser::ast::ArithmeticOp;
        let expr = parse("10 % 3").unwrap();
        assert!(matches!(expr, Expression::BinaryOp { .. }));

        if let Expression::BinaryOp {
            left,
            right,
            operator,
        } = expr
        {
            assert!(matches!(*left, Expression::Number(10)));
            assert!(matches!(*right, Expression::Number(3)));
            assert_eq!(operator, ArithmeticOp::Modulo);
        }
    }

    #[test]
    fn test_parse_arithmetic_add() {
        use crate::parser::ast::ArithmeticOp;
        let expr = parse("3 + 4").unwrap();
        assert!(matches!(expr, Expression::BinaryOp { .. }));

        if let Expression::BinaryOp {
            left,
            right,
            operator,
        } = expr
        {
            assert!(matches!(*left, Expression::Number(3)));
            assert!(matches!(*right, Expression::Number(4)));
            assert_eq!(operator, ArithmeticOp::Add);
        }
    }

    #[test]
    fn test_parse_arithmetic_precedence_multiply_before_add() {
        use crate::parser::ast::ArithmeticOp;
        // 2 + 3 * 4 should parse as 2 + (3 * 4), not (2 + 3) * 4
        let expr = parse("2 + 3 * 4").unwrap();
        assert!(matches!(expr, Expression::BinaryOp { .. }));

        if let Expression::BinaryOp {
            left,
            right,
            operator,
        } = expr
        {
            // Outer operation should be Add
            assert_eq!(operator, ArithmeticOp::Add);
            assert!(matches!(*left, Expression::Number(2)));
            // Right side should be 3 * 4
            if let Expression::BinaryOp {
                left: mul_left,
                right: mul_right,
                operator: mul_op,
            } = *right
            {
                assert_eq!(mul_op, ArithmeticOp::Multiply);
                assert!(matches!(*mul_left, Expression::Number(3)));
                assert!(matches!(*mul_right, Expression::Number(4)));
            } else {
                panic!("Expected BinaryOp for 3 * 4");
            }
        }
    }

    #[test]
    fn test_parse_arithmetic_parentheses_override_precedence() {
        use crate::parser::ast::ArithmeticOp;
        // (2 + 3) * 4 should parse parenthesized addition first
        let expr = parse("(2 + 3) * 4").unwrap();
        assert!(matches!(expr, Expression::BinaryOp { .. }));

        if let Expression::BinaryOp {
            left,
            right,
            operator,
        } = expr
        {
            // Outer operation should be Multiply
            assert_eq!(operator, ArithmeticOp::Multiply);
            // Left side should be 2 + 3
            if let Expression::BinaryOp {
                left: add_left,
                right: add_right,
                operator: add_op,
            } = *left
            {
                assert_eq!(add_op, ArithmeticOp::Add);
                assert!(matches!(*add_left, Expression::Number(2)));
                assert!(matches!(*add_right, Expression::Number(3)));
            } else {
                panic!("Expected BinaryOp for 2 + 3");
            }
            assert!(matches!(*right, Expression::Number(4)));
        }
    }

    #[test]
    fn test_parse_complex_arithmetic() {
        // 100 + 10 * 5 - 20 / 2 should work
        let expr = parse("100 + 10 * 5 - 20 / 2").unwrap();
        // This is a complex expression - just verify it parses
        assert!(matches!(expr, Expression::BinaryOp { .. }));
    }

    #[test]
    fn test_parse_array_with_variable() {
        // X is not a valid note name, so lexer treats it as identifier
        // With new Expression::Array, this parses successfully as [Variable, Note, Note]
        let result = parse("[X, E, G]");
        assert!(result.is_ok());
        if let Expression::Array(elements) = result.unwrap() {
            assert_eq!(elements.len(), 3);
            assert!(matches!(&elements[0], Expression::Variable(n) if n == "X"));
            assert!(matches!(&elements[1], Expression::Note(_)));
            assert!(matches!(&elements[2], Expression::Note(_)));
        }
    }

    #[test]
    fn test_parse_error_invalid_note_name() {
        // H is not a valid note (only A-G)
        let result = parse("H"); // H becomes Identifier("H")

        // With our new parser logic, H gets parsed as a variable reference
        // This should succeed at parse time, but would fail at evaluation time
        assert!(result.is_ok());

        if let Ok(Expression::Variable(name)) = result {
            assert_eq!(name, "H");
        } else {
            panic!("Expected H to be parsed as a variable");
        }
    }

    #[test]
    fn test_parse_error_unexpected_token() {
        let result = parse("@"); // @ is truly unexpected
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unexpected token") || err.contains("Unexpected character"));
    }

    #[test]
    fn test_parse_error_missing_bracket() {
        let result = parse("[C, E, G");
        assert!(result.is_err());
    }

    // =========================================================================
    // Method Chaining Tests
    // =========================================================================

    #[test]
    fn test_parse_method_call_simple() {
        // "C E G".fast(2) should desugar to fast("C E G", 2)
        let expr = parse("\"C E G\".fast(2)").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "fast");
            assert_eq!(args.len(), 2);
            // First arg should be the pattern
            assert!(matches!(args[0], Expression::Pattern(_)));
        }
    }

    #[test]
    fn test_parse_method_call_no_args() {
        // "C E G".rev() should desugar to rev("C E G")
        let expr = parse("\"C E G\".rev()").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "rev");
            assert_eq!(args.len(), 1);
        }
    }

    #[test]
    fn test_parse_method_chaining() {
        // "C E G".fast(2).rev() should desugar to rev(fast("C E G", 2))
        let expr = parse("\"C E G\".fast(2).rev()").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "rev");
            assert_eq!(args.len(), 1);
            // The argument should be the result of fast()
            assert!(matches!(args[0], Expression::FunctionCall { .. }));
        }
    }

    #[test]
    fn test_parse_method_on_variable() {
        // x.rev() should desugar to rev(x)
        let expr = parse("x.rev()").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "rev");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Expression::Variable(_)));
        }
    }

    #[test]
    fn test_parse_method_with_multiple_args() {
        // x.every(2, "rev") should desugar to every(x, 2, "rev")
        let expr = parse("x.every(2, \"rev\")").unwrap();
        assert!(matches!(expr, Expression::FunctionCall { .. }));

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "every");
            assert_eq!(args.len(), 3); // receiver + 2 args
        }
    }

    #[test]
    fn test_parse_method_with_transpose() {
        // [C, E, G].invert() + 2 should be (invert([C, E, G])) + 2
        let expr = parse("[C, E, G].invert() + 2").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));

        if let Expression::Transpose { target, semitones } = expr {
            assert_eq!(semitones, 2);
            assert!(matches!(*target, Expression::FunctionCall { .. }));
        }
    }

    #[test]
    fn test_spanned_statement_boundaries() {
        // Test that statement spans cover the entire statement text
        let code = "tempo 120";
        let program = parse_spanned_statements(code).unwrap();
        assert_eq!(program.statements.len(), 1);

        let stmt = &program.statements[0];
        eprintln!("Code: '{}' (len {})", code, code.len());
        eprintln!("Statement span: start={}, end={}", stmt.start, stmt.end);

        // Check every position
        for i in 0..code.len() {
            let found = stmt.contains(i);
            let ch = code.chars().nth(i).unwrap_or('?');
            eprintln!("  pos {}: {} '{}'", i, if found { "✓" } else { "✗" }, ch);
        }

        assert_eq!(stmt.start, 0, "Statement should start at 0");
        // End should include the last character (exclusive end, so >= len)
        assert!(
            stmt.end >= code.len(),
            "Statement end {} should cover entire '{}' (len {})",
            stmt.end,
            code,
            code.len()
        );
    }

    #[test]
    fn test_spanned_statement_with_pattern() {
        // Test pattern statement spans - this is a realistic user case
        let code = r#"let x = "C E G _""#;
        let program = parse_spanned_statements(code).unwrap();
        assert_eq!(program.statements.len(), 1);

        let stmt = &program.statements[0];

        eprintln!("\nPattern statement test:");
        eprintln!("Code: {:?} (len {})", code, code.len());
        eprintln!("Statement span: start={}, end={}", stmt.start, stmt.end);

        // Check every position
        for i in 0..code.len() {
            let found = stmt.contains(i);
            let ch = code.chars().nth(i).unwrap_or('?');
            eprintln!("  pos {:2}: {} '{}'", i, if found { "✓" } else { "✗" }, ch);
        }

        // Position near end should be found
        let near_end = code.len() - 2; // Inside the closing quote
        assert!(
            stmt.contains(near_end),
            "Position {} should be in statement (start={}, end={})",
            near_end,
            stmt.start,
            stmt.end
        );

        // ALL positions should be found
        for i in 0..code.len() {
            assert!(stmt.contains(i), "Position {} should be in statement", i);
        }
    }

    #[test]
    fn test_spanned_multiple_statements() {
        let code = "tempo 120\nplay x loop";
        let program = parse_spanned_statements(code).unwrap();
        assert_eq!(program.statements.len(), 2);

        eprintln!("\nMulti-line test:");
        eprintln!("Code: {:?} (len {})", code, code.len());

        for (i, stmt) in program.statements.iter().enumerate() {
            eprintln!("Statement {}: start={}, end={}", i, stmt.start, stmt.end);
        }

        // The first statement "tempo 120" is 9 chars, ends at position 9
        // Position 8 is '0' - should be in first statement
        let stmt1 = &program.statements[0];
        eprintln!(
            "Position 8 (last char of 'tempo 120'): in stmt0? {}",
            stmt1.contains(8)
        );
        assert!(
            stmt1.contains(8),
            "Last char of first statement should be found"
        );

        // First statement should contain position 5
        assert!(program.statement_at(5).is_some());
        // Second statement should contain position in "loop"
        let loop_pos = code.find("loop").unwrap() + 2;
        assert!(
            program.statement_at(loop_pos).is_some(),
            "Position {} in 'loop' should find statement",
            loop_pos
        );
    }

    #[test]
    fn test_spanned_boundary_strict() {
        // Strict test: the exact edge case user reported
        // When cursor is at the LAST character of a statement (e.g., the `]` in `[C, E, G]`),
        // it should still find that statement

        let code = "let cmaj = [C, E, G]";
        let program = parse_spanned_statements(code).unwrap();
        assert_eq!(program.statements.len(), 1);

        let stmt = &program.statements[0];

        // The closing `]` is at position 19 (0-indexed), code.len() = 20
        let last_char_pos = code.len() - 1; // Position 19, the `]`
        assert_eq!(
            code.chars().nth(last_char_pos),
            Some(']'),
            "Expected ] at last position"
        );

        // This is the critical assertion - last character MUST be in the span
        assert!(
            stmt.contains(last_char_pos),
            "Position {} (the `]`) MUST be in statement span (start={}, end={}). Fix failed!",
            last_char_pos,
            stmt.start,
            stmt.end
        );

        // Also verify span end is >= length (exclusive end)
        assert!(
            stmt.end >= code.len(),
            "Span end {} must be >= code length {} to include last char",
            stmt.end,
            code.len()
        );
    }

    #[test]
    fn test_spanned_multi_line_boundaries() {
        // Test that each statement in a multi-line program covers all its characters
        let code = "let a = [C]\nlet b = [D]\nlet c = [E]";
        let program = parse_spanned_statements(code).unwrap();
        assert_eq!(program.statements.len(), 3);

        // Each statement should cover its last character
        for (i, stmt) in program.statements.iter().enumerate() {
            // Last char before the span ends should be included
            if stmt.end > 0 {
                assert!(
                    stmt.contains(stmt.end - 1),
                    "Statement {} end-1 position {} not covered (start={}, end={})",
                    i,
                    stmt.end - 1,
                    stmt.start,
                    stmt.end
                );
            }
        }

        // Verify no gaps between statements cause lookups to fail
        // Every position in the code should find SOME statement
        for pos in 0..code.len() {
            let ch = code.chars().nth(pos).unwrap();
            // Skip newline positions - those are legitimately between statements
            if ch != '\n' {
                assert!(
                    program.statement_at(pos).is_some(),
                    "Position {} ('{}') must find a statement but returned None",
                    pos,
                    ch
                );
            }
        }
    }
}
