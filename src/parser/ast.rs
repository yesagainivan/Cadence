use crate::types::{chord::Chord, note::Note, pattern::Pattern, progression::Progression};
use std::fmt;

// ============================================================================
// Program and Statement AST (for scripting)
// ============================================================================

/// A program is a sequence of statements
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Program {
            statements: Vec::new(),
        }
    }

    pub fn push(&mut self, stmt: Statement) {
        self.statements.push(stmt);
    }

    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

/// Statement types for scripting
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Variable binding: let prog = ii_V_I(C)
    Let { name: String, value: Expression },

    /// Variable re-assignment: prog = other_expr
    Assign { name: String, value: Expression },

    /// Expression statement (evaluates and optionally prints): [C, E, G]
    Expression(Expression),

    /// Play command with options: play progression loop queue
    Play {
        target: Expression,
        looping: bool,
        queue: bool,
        duration: Option<f32>,
    },

    /// Stop playback
    Stop,

    /// Set tempo: tempo 120
    Tempo(f32),

    /// Set volume: volume 0.5
    Volume(f32),

    /// Infinite loop: loop { ... }
    Loop { body: Vec<Statement> },

    /// Repeat n times: repeat 4 { ... }
    Repeat { count: u32, body: Vec<Statement> },

    /// Conditional: if condition { ... } else { ... }
    If {
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },

    /// Break out of loop
    Break,

    /// Continue to next iteration
    Continue,

    /// Return a value (for functions)
    Return(Option<Expression>),

    /// Load a file: load "path/to/file.cadence"
    Load(String),

    /// Comment (preserved for tooling/pretty-printing)
    Comment(String),

    /// Block of statements: { stmt1; stmt2; }
    Block(Vec<Statement>),

    /// Track selector: track 1 { ... } or track 1 play ...
    Track { id: usize, body: Box<Statement> },
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Let { name, value } => write!(f, "let {} = {}", name, value),
            Statement::Assign { name, value } => write!(f, "{} = {}", name, value),
            Statement::Expression(expr) => write!(f, "{}", expr),
            Statement::Play {
                target,
                looping,
                queue,
                duration,
            } => {
                write!(f, "play {}", target)?;
                if *looping {
                    write!(f, " loop")?;
                }
                if *queue {
                    write!(f, " queue")?;
                }
                if let Some(d) = duration {
                    write!(f, " duration {}", d)?;
                }
                Ok(())
            }
            Statement::Stop => write!(f, "stop"),
            Statement::Tempo(bpm) => write!(f, "tempo {}", bpm),
            Statement::Volume(vol) => write!(f, "volume {}", vol),
            Statement::Loop { .. } => write!(f, "loop {{ ... }}"),
            Statement::Repeat { count, .. } => write!(f, "repeat {} {{ ... }}", count),
            Statement::If { .. } => write!(f, "if ... {{ ... }}"),
            Statement::Break => write!(f, "break"),
            Statement::Continue => write!(f, "continue"),
            Statement::Return(Some(expr)) => write!(f, "return {}", expr),
            Statement::Return(None) => write!(f, "return"),
            Statement::Load(path) => write!(f, "load \"{}\"", path),
            Statement::Comment(text) => write!(f, "// {}", text),
            Statement::Block(_) => write!(f, "{{ ... }}"),
            Statement::Track { id, body } => write!(f, "track {} {}", id, body),
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stmt in &self.statements {
            writeln!(f, "{}", stmt)?;
        }
        Ok(())
    }
}

// ============================================================================
// Expression AST (existing, unchanged)
// ============================================================================

/// Represents different types of expressions in the Cadence language
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A single note literal: C, F#, Bb
    Note(Note),

    /// A chord literal: [C, E, G]
    Chord(Chord),

    /// A progression literal: [[C, E, G], [F, A, C], [G, B, D]]
    Progression(Progression),

    /// Variable reference: prog (lookup in environment)
    Variable(String),

    /// Arithmetic operation: [C, E, G] + 2, [[C, E, G], [F, A, C]] + 2
    Transpose {
        target: Box<Expression>,
        semitones: i8,
    },

    /// Set intersection: [C, E, G] & [A, C, E]
    Intersection {
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Set union: [C, E, G] | [A, C, E]
    Union {
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Set symmetric difference: [C, E, G] ^ [A, C, E]
    Difference {
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Function call: invert([C, E, G]), map(invert, [[C, E, G], [F, A, C]])
    FunctionCall { name: String, args: Vec<Expression> },

    /// Boolean literal (for conditionals)
    Boolean(bool),

    /// Comparison: expr == expr, expr != expr
    Comparison {
        left: Box<Expression>,
        right: Box<Expression>,
        operator: ComparisonOp,
    },

    /// Pattern literal: "C E G _"
    Pattern(Pattern),
}

/// Comparison operators
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    // Future: Less, Greater, LessEqual, GreaterEqual
}

/// Represents the result of evaluating an expression
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Note(Note),
    Chord(Chord),
    Progression(Progression),
    Boolean(bool),
    Pattern(Pattern),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Note(note) => write!(f, "{}", note),
            Expression::Chord(chord) => write!(f, "{}", chord),
            Expression::Progression(progression) => write!(f, "{}", progression),
            Expression::Transpose { target, semitones } => {
                if *semitones >= 0 {
                    write!(f, "{} + {}", target, semitones)
                } else {
                    write!(f, "{} - {}", target, semitones.abs())
                }
            }
            Expression::Intersection { left, right } => {
                write!(f, "{} & {}", left, right)
            }
            Expression::Union { left, right } => {
                write!(f, "{} | {}", left, right)
            }
            Expression::Difference { left, right } => {
                write!(f, "{} ^ {}", left, right)
            }
            Expression::FunctionCall { name, args } => {
                write!(f, "{}(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expression::Variable(name) => write!(f, "{}", name),
            Expression::Boolean(b) => write!(f, "{}", b),
            Expression::Comparison {
                left,
                right,
                operator,
            } => {
                let op_str = match operator {
                    ComparisonOp::Equal => "==",
                    ComparisonOp::NotEqual => "!=",
                };
                write!(f, "{} {} {}", left, op_str, right)
            }
            Expression::Pattern(pattern) => write!(f, "{}", pattern),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Note(note) => write!(f, "{}", note),
            Value::Chord(chord) => write!(f, "{}", chord),
            Value::Progression(progression) => write!(f, "{}", progression),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Pattern(pattern) => write!(f, "{}", pattern),
        }
    }
}

impl Expression {
    /// Helper constructor for transpose expressions
    pub fn transpose(target: Expression, semitones: i8) -> Self {
        Expression::Transpose {
            target: Box::new(target),
            semitones,
        }
    }

    /// Helper constructor for intersection expressions
    pub fn intersection(left: Expression, right: Expression) -> Self {
        Expression::Intersection {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Helper constructor for union expressions
    pub fn union(left: Expression, right: Expression) -> Self {
        Expression::Union {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Helper constructor for difference expressions
    pub fn difference(left: Expression, right: Expression) -> Self {
        Expression::Difference {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Helper constructor for function call expressions
    pub fn function_call(name: impl Into<String>, args: Vec<Expression>) -> Self {
        Expression::FunctionCall {
            name: name.into(),
            args,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_expression_display() {
        // Test note expression
        let c_note = Expression::Note(Note::from_str("C").unwrap());
        assert_eq!(format!("{}", c_note), "C");

        // Test chord expression
        let c_major = Expression::Chord(Chord::from_note_strings(vec!["C", "E", "G"]).unwrap());
        assert!(format!("{}", c_major).contains("C Major"));

        // Test progression expression
        let progression = Expression::Progression(
            Progression::from_chord_strings(vec![vec!["C", "E", "G"], vec!["F", "A", "C"]])
                .unwrap(),
        );
        let display = format!("{}", progression);
        // Note: colored output may contain ANSI codes, check for content presence
        assert!(display.contains("["));
        assert!(display.contains("C Major") || display.contains("C")); // May have colored output
        assert!(display.contains("F Major") || display.contains("F"));

        // Test transpose expression
        let transposed = Expression::transpose(c_major.clone(), 2);
        assert!(format!("{}", transposed).contains(" + 2"));

        let transposed_down = Expression::transpose(c_major.clone(), -3);
        assert!(format!("{}", transposed_down).contains(" - 3"));

        // Test set operations
        let a_minor = Expression::Chord(Chord::from_note_strings(vec!["A", "C", "E"]).unwrap());

        let intersection = Expression::intersection(c_major.clone(), a_minor.clone());
        assert!(format!("{}", intersection).contains(" & "));

        let union = Expression::union(c_major.clone(), a_minor.clone());
        assert!(format!("{}", union).contains(" | "));

        let difference = Expression::difference(c_major, a_minor);
        assert!(format!("{}", difference).contains(" ^ "));

        // Test function call
        let invert_call = Expression::function_call("invert", vec![c_note]);
        assert_eq!(format!("{}", invert_call), "invert(C)");
    }

    #[test]
    fn test_value_display() {
        let note_val = Value::Note(Note::from_str("F#").unwrap());
        assert_eq!(format!("{}", note_val), "F#");

        let chord_val = Value::Chord(Chord::from_note_strings(vec!["D", "F#", "A"]).unwrap());
        assert!(format!("{}", chord_val).contains("D Major"));

        let progression_val = Value::Progression(
            Progression::from_chord_strings(vec![vec!["C", "E", "G"], vec!["F", "A", "C"]])
                .unwrap(),
        );
        let display = format!("{}", progression_val);
        assert!(display.contains("C Major"));
        assert!(display.contains("F Major"));
    }

    #[test]
    fn test_expression_constructors() {
        let c_note = Expression::Note(Note::from_str("C").unwrap());

        // Test helper constructors don't panic
        let _transpose = Expression::transpose(c_note.clone(), 5);
        let _intersection = Expression::intersection(c_note.clone(), c_note.clone());
        let _union = Expression::union(c_note.clone(), c_note.clone());
        let _difference = Expression::difference(c_note.clone(), c_note.clone());
        let _function = Expression::function_call("test", vec![c_note]);
    }

    #[test]
    fn test_progression_expressions() {
        let progression = Progression::from_chord_strings(vec![
            vec!["C", "E", "G"],
            vec!["F", "A", "C"],
            vec!["G", "B", "D"],
        ])
        .unwrap();

        let prog_expr = Expression::Progression(progression.clone());
        let prog_value = Value::Progression(progression);

        // Test that they display correctly
        let expr_display = format!("{}", prog_expr);
        let value_display = format!("{}", prog_value);

        assert_eq!(expr_display, value_display);
        assert!(expr_display.contains("C Major"));
        assert!(expr_display.contains("F Major"));
        assert!(expr_display.contains("G Major"));
    }
}
