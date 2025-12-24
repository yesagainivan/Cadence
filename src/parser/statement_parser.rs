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
use crate::parser::lexer::{Lexer, Token};
use crate::parser::parser::Parser;
use anyhow::{Result, anyhow};

/// Parses statements and programs (sequences of statements)
pub struct StatementParser {
    tokens: Vec<Token>,
    position: usize,
}

impl StatementParser {
    /// Create a new statement parser from input string
    pub fn new(input: &str) -> Result<Self> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize()?;
        
        Ok(StatementParser {
            tokens,
            position: 0,
        })
    }

    /// Current token
    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::Eof)
    }

    /// Peek at the next token
    fn peek(&self) -> &Token {
        self.tokens.get(self.position + 1).unwrap_or(&Token::Eof)
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
            Err(anyhow!("Expected {:?}, found {:?}", expected, self.current()))
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
        
        Ok(Statement::Play { target, looping, queue, duration })
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
        
        Ok(Statement::If { condition, then_body, else_body })
    }

    /// Parse: return [expression]
    fn parse_return_statement(&mut self) -> Result<Statement> {
        self.expect(&Token::Return)?;
        
        // Check if there's an expression following
        if self.check(&Token::Semicolon) || self.check(&Token::Newline) || 
           self.check(&Token::RightBrace) || self.check(&Token::Eof) {
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

    /// Parse an expression (delegates to the expression parser)
    fn parse_expression(&mut self) -> Result<Expression> {
        // We need to find where the expression ends and pass those tokens
        // For now, we'll re-parse from the remaining input
        // This is a simplified approach - in production we'd share token stream
        
        let remaining_tokens: Vec<Token> = self.tokens[self.position..].iter()
            .take_while(|t| {
                !matches!(t, 
                    Token::Semicolon | Token::Newline | Token::LeftBrace | 
                    Token::RightBrace | Token::Loop | Token::Queue | Token::Eof |
                    // Stop at keywords that start new statements
                    Token::Let | Token::If | Token::Else
                )
            })
            .cloned()
            .collect();
        
        if remaining_tokens.is_empty() {
            return Err(anyhow!("Expected expression"));
        }
        
        // Count how many tokens we consumed
        let consumed = remaining_tokens.len();
        
        // Reconstruct input string from tokens for the expression parser
        // This is hacky but works for now
        let expr_str: String = remaining_tokens.iter()
            .map(|t| format!("{} ", t))
            .collect();
        
        let mut parser = Parser::new(&expr_str)?;
        let expr = parser.parse()?;
        
        // Advance our position
        self.position += consumed;
        
        Ok(expr)
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
