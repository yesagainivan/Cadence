use crate::types::{chord::Chord, note::Note};
use crate::{
    parser::{
        ast::Expression,
        lexer::{Lexer, Token},
    },
    types::Progression,
};
use anyhow::{Result, anyhow};

/// Recursive descent parser for the Cadence language
pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    current_token: Token,
}

impl Parser {
    /// Create a new parser from input string
    pub fn new(input: &str) -> Result<Self> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize()?;

        if tokens.is_empty() {
            return Err(anyhow!("No tokens to parse"));
        }

        let current_token = tokens[0].clone();

        Ok(Parser {
            tokens,
            position: 0,
            current_token,
        })
    }

    /// Advance to the next token
    fn advance(&mut self) {
        if self.position < self.tokens.len() - 1 {
            self.position += 1;
            self.current_token = self.tokens[self.position].clone();
        }
    }

    /// Peek at the next token without advancing
    // currently unused
    fn _peek(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1)
    }

    /// Check if current token matches expected type
    fn expect(&mut self, expected: Token) -> Result<()> {
        if std::mem::discriminant(&self.current_token) == std::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(anyhow!(
                "Expected {:?}, found {:?}",
                expected,
                self.current_token
            ))
        }
    }

    /// Parse the input into an expression
    pub fn parse(&mut self) -> Result<Expression> {
        self.parse_expression()
    }

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
            self.current_token,
            Token::Ampersand | Token::Pipe | Token::Caret
        ) {
            let op = self.current_token.clone();
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

        if matches!(self.current_token, Token::Plus | Token::Minus) {
            let is_plus = matches!(self.current_token, Token::Plus);
            self.advance();

            if let Token::Number(semitones) = self.current_token {
                self.advance();
                let semitones = if is_plus { semitones } else { -semitones };
                expr = Expression::transpose(expr, semitones);
            } else {
                return Err(anyhow!("Expected number after +/- operator"));
            }
        }

        Ok(expr)
    }

    /// Parse primary expressions (notes, chords, progressions, function calls, parentheses)
    /// Grammar: primary_expr = note | chord | progression | function_call | identifier | '(' expression ')'
    fn parse_primary_expression(&mut self) -> Result<Expression> {
        match &self.current_token {
            Token::Note(note_str) => {
                let note: Note = note_str
                    .parse()
                    .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
                self.advance();
                Ok(Expression::Note(note))
            }

            Token::LeftBracket => self.parse_chord(),

            Token::LeftDoubleBracket => self.parse_progression(),

            Token::Identifier(name) => {
                let name = name.clone();
                self.advance();

                // Check if this is a function call (has parentheses) or just an identifier
                if matches!(self.current_token, Token::LeftParen) {
                    // It's a function call - back up and parse it properly
                    self.position -= 1;
                    self.current_token = Token::Identifier(name.clone());
                    self.parse_function_call(name)
                } else {
                    // It's just a bare identifier - treat it as a function call with no args
                    Ok(Expression::function_call(name, vec![]))
                }
            }

            Token::LeftParen => {
                self.advance(); // consume '('
                let expr = self.parse_expression()?;
                self.expect(Token::RightParen)?;
                Ok(expr)
            }

            _ => Err(anyhow!("Unexpected token: {:?}", self.current_token)),
        }
    }

    /// Parse a chord literal: [C, E, G]
    /// Grammar: chord = '[' note (',' note)* ']'
    fn parse_chord(&mut self) -> Result<Expression> {
        let chord = self.parse_chord_inner()?;
        Ok(Expression::Chord(chord))
    }

    /// Parse a progression literal: [[C, E, G], [F, A, C]]
    /// Grammar: progression = '[[' chord (',' chord)* ']]'
    fn parse_progression(&mut self) -> Result<Expression> {
        self.expect(Token::LeftDoubleBracket)?;

        let mut chords = Vec::new();

        // Parse first chord - we need to parse the chord contents directly
        // since LeftDoubleBracket already consumed the first [
        let first_chord = self.parse_chord_contents()?;
        chords.push(first_chord);

        // Parse remaining chords
        while matches!(self.current_token, Token::Comma) {
            self.advance(); // consume ','

            // Expect [ for the next chord
            self.expect(Token::LeftBracket)?;
            let chord = self.parse_chord_contents()?; // Parse contents without expecting [
            chords.push(chord);
        }

        self.expect(Token::RightDoubleBracket)?;

        let progression = Progression::from_chords(chords);
        Ok(Expression::Progression(progression))
    }

    /// Parse just the chord contents (notes and commas) without brackets
    fn parse_chord_contents(&mut self) -> Result<Chord> {
        let mut notes = Vec::new();

        // Parse first note
        if let Token::Note(note_str) = &self.current_token {
            let note: Note = note_str
                .parse()
                .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
            notes.push(note);
            self.advance();
        } else {
            return Err(anyhow!(
                "Expected note in chord, found {:?}",
                self.current_token
            ));
        }

        // Parse remaining notes
        while matches!(self.current_token, Token::Comma) {
            self.advance(); // consume ','

            if let Token::Note(note_str) = &self.current_token {
                let note: Note = note_str
                    .parse()
                    .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
                notes.push(note);
                self.advance();
            } else {
                return Err(anyhow!(
                    "Expected note after comma, found {:?}",
                    self.current_token
                ));
            }
        }

        // The last chord in a progression will end with RightDoubleBracket, not RightBracket
        if matches!(self.current_token, Token::RightBracket) {
            self.advance(); // consume ]
        }
        // If it's RightDoubleBracket, don't consume it - let parse_progression handle it
        else if !matches!(self.current_token, Token::RightDoubleBracket) {
            return Err(anyhow!(
                "Expected RightBracket or RightDoubleBracket, found {:?}",
                self.current_token
            ));
        }

        Ok(Chord::from_notes(notes))
    }

    /// Parse the inner part of a chord (notes without brackets)
    /// This is extracted so it can be reused by progression parsing
    fn parse_chord_inner(&mut self) -> Result<Chord> {
        self.expect(Token::LeftBracket)?;

        let mut notes = Vec::new();

        // Parse first note
        if let Token::Note(note_str) = &self.current_token {
            let note: Note = note_str
                .parse()
                .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
            notes.push(note);
            self.advance();
        } else {
            return Err(anyhow!(
                "Expected note in chord, found {:?}",
                self.current_token
            ));
        }

        // Parse remaining notes
        while matches!(self.current_token, Token::Comma) {
            self.advance(); // consume ','

            if let Token::Note(note_str) = &self.current_token {
                let note: Note = note_str
                    .parse()
                    .map_err(|e| anyhow!("Invalid note '{}': {}", note_str, e))?;
                notes.push(note);
                self.advance();
            } else {
                return Err(anyhow!(
                    "Expected note after comma, found {:?}",
                    self.current_token
                ));
            }
        }

        self.expect(Token::RightBracket)?;
        Ok(Chord::from_notes(notes))
    }

    /// Parse a function call: invert([C, E, G])
    /// Grammar: function_call = identifier '(' expression (',' expression)* ')'
    fn parse_function_call(&mut self, name: String) -> Result<Expression> {
        self.advance(); // consume function name
        self.expect(Token::LeftParen)?;

        let mut args = Vec::new();

        // Handle empty argument list
        if matches!(self.current_token, Token::RightParen) {
            self.advance();
            return Ok(Expression::function_call(name, args));
        }

        // Parse first argument
        args.push(self.parse_expression()?);

        // Parse remaining arguments
        while matches!(self.current_token, Token::Comma) {
            self.advance(); // consume ','
            args.push(self.parse_expression()?);
        }

        self.expect(Token::RightParen)?;

        Ok(Expression::function_call(name, args))
    }
}

/// Convenience function to parse a string into an expression
pub fn parse(input: &str) -> Result<Expression> {
    let mut parser = Parser::new(input)?;
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::types::note::Note;

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
        let expr = parse("[C, E, G]").unwrap();
        assert!(matches!(expr, Expression::Chord(_)));

        if let Expression::Chord(chord) = expr {
            assert_eq!(chord.len(), 3);
            assert!(chord.contains(&"C".parse().unwrap()));
            assert!(chord.contains(&"E".parse().unwrap()));
            assert!(chord.contains(&"G".parse().unwrap()));
        }
    }

    #[test]
    fn test_parse_progression() {
        let expr = parse("[[C, E, G], [F, A, C]]").unwrap();
        assert!(matches!(expr, Expression::Progression(_)));

        if let Expression::Progression(progression) = expr {
            assert_eq!(progression.len(), 2);

            // Test first chord is C major
            let first_chord = &progression[0];
            assert!(first_chord.contains(&"C".parse().unwrap()));
            assert!(first_chord.contains(&"E".parse().unwrap()));
            assert!(first_chord.contains(&"G".parse().unwrap()));

            // Test second chord is F major
            let second_chord = &progression[1];
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
            assert!(matches!(*target, Expression::Progression(_)));
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
            assert!(matches!(*target, Expression::Chord(_)));
        }
    }

    #[test]
    fn test_parse_transpose_negative() {
        let expr = parse("[C, E, G] - 5").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));

        if let Expression::Transpose { target, semitones } = expr {
            assert_eq!(semitones, -5);
            assert!(matches!(*target, Expression::Chord(_)));
        }
    }

    #[test]
    fn test_parse_set_intersection() {
        let expr = parse("[C, E, G] & [A, C, E]").unwrap();
        assert!(matches!(expr, Expression::Intersection { .. }));

        if let Expression::Intersection { left, right } = expr {
            assert!(matches!(*left, Expression::Chord(_)));
            assert!(matches!(*right, Expression::Chord(_)));
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
            assert!(matches!(args[0], Expression::Chord(_)));
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
            assert!(matches!(args[1], Expression::Chord(_)));
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
            assert!(matches!(*right, Expression::Chord(_)));
        }
    }

    #[test]
    fn test_whitespace_handling() {
        let expr = parse("  [ C , E , G ]  + 2  ").unwrap();
        assert!(matches!(expr, Expression::Transpose { .. }));
    }

    #[test]
    fn test_parse_error_invalid_note() {
        // X is not a valid note name, so lexer treats it as identifier
        // Parser expects a note in chord, gets identifier → specific error message
        let result = parse("[X, E, G]");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Expected note in chord")
        );
    }

    #[test]
    fn test_parse_error_invalid_note_name() {
        // H is not a valid note (only A-G)
        let result = parse("H"); // H becomes Identifier("H")

        // With our new parser logic, H gets parsed as a function call H() with no arguments
        // This should succeed at parse time, but would fail at evaluation time
        assert!(result.is_ok());

        if let Ok(Expression::FunctionCall { name, args }) = result {
            assert_eq!(name, "H");
            assert!(args.is_empty());
        } else {
            panic!("Expected H to be parsed as a function call");
        }
    }

    #[test]
    fn test_parse_error_unexpected_token() {
        let result = parse("@"); // @ is truly unexpected
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unexpected character")
        ); // This comes from lexer
    }

    #[test]
    fn test_parse_error_missing_bracket() {
        let result = parse("[C, E, G");
        assert!(result.is_err());
    }

    // #[test]
    // fn test_parse_error_unexpected_token() {
    //     let result = parse("@");
    //     assert!(result.is_err());
    // }
}
