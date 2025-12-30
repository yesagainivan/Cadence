use crate::types::{
    chord::Chord,
    note::Note,
    pattern::{EveryPattern, Pattern},
};
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

// ============================================================================
// Spanned versions for source tracking (editor integration)
// ============================================================================

/// A statement with source location tracking
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedStatement {
    pub statement: Statement,
    /// Byte offset of statement start in source
    pub start: usize,
    /// Byte offset of statement end in source
    pub end: usize,
    /// UTF-16 code unit offset of statement start (for JavaScript interop)
    pub utf16_start: usize,
    /// UTF-16 code unit offset of statement end
    pub utf16_end: usize,
}

impl SpannedStatement {
    pub fn new(statement: Statement, start: usize, end: usize) -> Self {
        SpannedStatement {
            statement,
            start,
            end,
            utf16_start: 0,
            utf16_end: 0,
        }
    }

    /// Create with UTF-16 offsets
    pub fn with_utf16(
        statement: Statement,
        start: usize,
        end: usize,
        utf16_start: usize,
        utf16_end: usize,
    ) -> Self {
        SpannedStatement {
            statement,
            start,
            end,
            utf16_start,
            utf16_end,
        }
    }

    /// Check if a given position (byte offset) is within this statement
    /// Uses inclusive end bound so cursor at last character still matches
    pub fn contains(&self, position: usize) -> bool {
        position >= self.start && position <= self.end
    }

    /// Check if a given UTF-16 position is within this statement
    /// Use this when position comes from JavaScript/CodeMirror
    /// Uses inclusive end bound so cursor at last character still matches
    pub fn contains_utf16(&self, position: usize) -> bool {
        position >= self.utf16_start && position <= self.utf16_end
    }
}

/// A program with source location tracking for each statement
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedProgram {
    pub statements: Vec<SpannedStatement>,
}

impl SpannedProgram {
    pub fn new() -> Self {
        SpannedProgram {
            statements: Vec::new(),
        }
    }

    pub fn push(&mut self, stmt: SpannedStatement) {
        self.statements.push(stmt);
    }

    /// Find the statement containing the given position (byte offset).
    pub fn statement_at(&self, position: usize) -> Option<&SpannedStatement> {
        self.statements.iter().find(|s| s.contains(position))
    }

    /// Find the statement containing the given UTF-16 position.
    /// Use this when position comes from JavaScript/CodeMirror.
    pub fn statement_at_utf16(&self, position: usize) -> Option<&SpannedStatement> {
        self.statements.iter().find(|s| s.contains_utf16(position))
    }

    /// Convert to regular Program (strips span info)
    pub fn to_program(&self) -> Program {
        Program {
            statements: self
                .statements
                .iter()
                .map(|s| s.statement.clone())
                .collect(),
        }
    }
}

impl Default for SpannedProgram {
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

    /// Play command with options: play progression loop queue [beat|bar|cycle]
    Play {
        target: Expression,
        looping: bool,
        /// None = immediate, Some("beat"|"bar"|"cycle") = queued with sync mode
        queue_mode: Option<String>,
        duration: Option<f32>,
    },

    /// Stop playback
    Stop,

    /// Set tempo: tempo 120 or tempo 100 + x
    Tempo(Expression),

    /// Set volume: volume 0.5 or volume x
    Volume(Expression),

    /// Set waveform: waveform "sine"
    Waveform(String),

    /// Infinite loop: loop { ... }
    Loop { body: Vec<Statement> },

    /// Repeat n times: repeat 4 { ... }
    Repeat { count: u32, body: Vec<Statement> },

    /// For loop: for i in 0..10 { ... }
    For {
        var: String,
        start: i32,
        end: i32,
        body: Vec<Statement>,
    },

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

    /// Function definition: fn name(param1, param2) { body }
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },

    /// Wait statement: wait <beats> (advances virtual time)
    Wait { beats: Expression },
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
                queue_mode,
                duration,
            } => {
                write!(f, "play {}", target)?;
                if *looping {
                    write!(f, " loop")?;
                }
                if let Some(mode) = queue_mode {
                    write!(f, " queue {}", mode)?;
                }
                if let Some(d) = duration {
                    write!(f, " duration {}", d)?;
                }
                Ok(())
            }
            Statement::Stop => write!(f, "stop"),
            Statement::Tempo(bpm) => write!(f, "tempo {}", bpm),
            Statement::Volume(vol) => write!(f, "volume {}", vol),
            Statement::Waveform(name) => write!(f, "waveform \"{}\"", name),
            Statement::Loop { .. } => write!(f, "loop {{ ... }}"),
            Statement::Repeat { count, .. } => write!(f, "repeat {} {{ ... }}", count),
            Statement::For {
                var, start, end, ..
            } => write!(f, "for {} in {}..{} {{ ... }}", var, start, end),
            Statement::If { .. } => write!(f, "if ... {{ ... }}"),
            Statement::Break => write!(f, "break"),
            Statement::Continue => write!(f, "continue"),
            Statement::Return(Some(expr)) => write!(f, "return {}", expr),
            Statement::Return(None) => write!(f, "return"),
            Statement::Load(path) => write!(f, "load \"{}\"", path),
            Statement::Comment(text) => write!(f, "// {}", text),
            Statement::Block(_) => write!(f, "{{ ... }}"),
            Statement::Track { id, body } => write!(f, "track {} {}", id, body),
            Statement::FunctionDef { name, params, .. } => {
                write!(f, "fn {}({}) {{ ... }}", name, params.join(", "))
            }
            Statement::Wait { beats } => write!(f, "wait {}", beats),
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

    // Note: Progressions are now represented as Pattern with chord steps
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

    /// String literal (that failed to parse as pattern): "rev"
    String(String),

    /// Numeric literal: 20, 100, etc.
    Number(i32),

    /// Pre-evaluated value (for dynamic function dispatch)
    /// Used when we need to pass an already-evaluated Value back through as an Expression
    Value(Box<Value>),

    /// Array of expressions: [root, third, fifth] or [C4, E4, G4]
    /// Resolved to Chord at evaluation time if all elements are notes
    Array(Vec<Expression>),

    /// Logical AND: expr && expr
    LogicalAnd {
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Logical OR: expr || expr
    LogicalOr {
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Logical NOT: !expr
    LogicalNot(Box<Expression>),

    /// Index operation: pattern[0], chord[1], array[-1]
    Index {
        target: Box<Expression>,
        index: Box<Expression>,
    },

    /// Binary arithmetic operation: expr + expr, expr * expr, etc.
    BinaryOp {
        left: Box<Expression>,
        right: Box<Expression>,
        operator: ArithmeticOp,
    },
}

/// Comparison operators
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
}

/// Arithmetic operators
#[derive(Debug, Clone, PartialEq)]
pub enum ArithmeticOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

/// Represents the result of evaluating an expression
#[derive(Debug, Clone)]
pub enum Value {
    Note(Note),
    Chord(Chord),
    Boolean(bool),
    Pattern(Pattern),
    Number(i32),
    String(String),
    /// User-defined function
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    /// Unit value (void) - for functions that don't return anything
    Unit,
    /// Array of values (when elements are not all notes)
    Array(Vec<Value>),
    /// Pattern combinator that applies a transformation every N cycles
    /// Used for TidalCycles-style `every(2, rev, pattern)` alternation
    EveryPattern(Box<EveryPattern>),
    /// Lazy/thunked expression - evaluated on each access
    /// Used for TidalCycles-style reactive variables
    Thunk {
        expression: Box<Expression>,
        /// Environment captured at definition time (for closures)
        env: crate::parser::environment::SharedEnvironment,
    },
}

// Manual PartialEq impl because Thunk contains SharedEnvironment which doesn't impl PartialEq
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Note(a), Value::Note(b)) => a == b,
            (Value::Chord(a), Value::Chord(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Pattern(a), Value::Pattern(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (
                Value::Function {
                    name: n1,
                    params: p1,
                    body: b1,
                },
                Value::Function {
                    name: n2,
                    params: p2,
                    body: b2,
                },
            ) => n1 == n2 && p1 == p2 && b1 == b2,
            (Value::Unit, Value::Unit) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::EveryPattern(a), Value::EveryPattern(b)) => a == b,
            // For thunks, compare only the expression (env identity doesn't matter for equality)
            (Value::Thunk { expression: e1, .. }, Value::Thunk { expression: e2, .. }) => e1 == e2,
            _ => false,
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Note(note) => write!(f, "{}", note),
            Expression::Chord(chord) => write!(f, "{}", chord),
            // Progressions now use Pattern representation
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
                    ComparisonOp::Less => "<",
                    ComparisonOp::Greater => ">",
                    ComparisonOp::LessEqual => "<=",
                    ComparisonOp::GreaterEqual => ">=",
                };
                write!(f, "{} {} {}", left, op_str, right)
            }
            Expression::Pattern(pattern) => write!(f, "{}", pattern),
            Expression::String(s) => write!(f, "\"{}\"", s),
            Expression::Number(n) => write!(f, "{}", n),
            Expression::Value(v) => write!(f, "{}", v),
            Expression::Array(elements) => {
                write!(f, "[")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }
            Expression::LogicalAnd { left, right } => {
                write!(f, "{} && {}", left, right)
            }
            Expression::LogicalOr { left, right } => {
                write!(f, "{} || {}", left, right)
            }
            Expression::LogicalNot(expr) => {
                write!(f, "!{}", expr)
            }
            Expression::Index { target, index } => {
                write!(f, "{}[{}]", target, index)
            }
            Expression::BinaryOp {
                left,
                right,
                operator,
            } => {
                let op_str = match operator {
                    ArithmeticOp::Add => "+",
                    ArithmeticOp::Subtract => "-",
                    ArithmeticOp::Multiply => "*",
                    ArithmeticOp::Divide => "/",
                    ArithmeticOp::Modulo => "%",
                };
                write!(f, "{} {} {}", left, op_str, right)
            }
        }
    }
}

use crate::types::DrumSound;

/// Playback info extracted from a Value - frequencies, duration, and optional drums
#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    /// Frequencies to play (Hz) - empty for rests
    pub frequencies: Vec<f32>,
    /// Duration of this event in beats
    pub duration_beats: f32,
    /// Drum sounds to trigger
    pub drums: Vec<DrumSound>,
}

impl Value {
    /// Convert this value to playback information for scheduling
    ///
    /// Returns a list of (frequencies, duration, drums) tuples for each step/event.
    /// For a single note/chord, returns one entry.
    /// For a pattern, returns one entry per step.
    pub fn to_playback_info(&self) -> Result<Vec<PlaybackInfo>, String> {
        match self {
            Value::Note(note) => Ok(vec![PlaybackInfo {
                frequencies: vec![note.frequency()],
                duration_beats: 1.0,
                drums: vec![],
            }]),
            Value::Chord(chord) => Ok(vec![PlaybackInfo {
                frequencies: chord.notes_vec().iter().map(|n| n.frequency()).collect(),
                duration_beats: 1.0,
                drums: vec![],
            }]),
            Value::Pattern(pattern) => {
                // Convert pattern to playback info with per-event durations
                Ok(pattern
                    .to_rich_events()
                    .into_iter()
                    .map(|event| PlaybackInfo {
                        frequencies: if event.is_rest {
                            vec![]
                        } else {
                            event.notes.iter().map(|n| n.frequency).collect()
                        },
                        duration_beats: event.duration_f32(),
                        drums: event.drums,
                    })
                    .collect())
            }
            Value::String(s) => {
                // Try to parse string as a pattern
                if let Ok(pattern) = Pattern::parse(s) {
                    Value::Pattern(pattern).to_playback_info()
                } else {
                    Err(format!("Cannot play a string \"{}\"", s))
                }
            }
            Value::Boolean(_) => Err("Cannot play a boolean value".to_string()),
            Value::Number(_) => Err("Cannot play a raw number".to_string()),
            Value::Function { name, .. } => {
                Err(format!("Cannot play a function '{}' - call it first", name))
            }
            Value::Unit => Err("Cannot play unit (void)".to_string()),
            Value::Array(_) => Err("Cannot play an array directly".to_string()),
            Value::EveryPattern(every) => {
                // For static evaluation (non-looping), use the base pattern
                // The real cycle selection happens in the playback engine
                Value::Pattern(every.base.clone()).to_playback_info()
            }
            Value::Thunk { .. } => {
                Err("Cannot play a thunk directly - it should have been evaluated".to_string())
            }
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Note(note) => write!(f, "{}", note),
            Value::Chord(chord) => write!(f, "{}", chord),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Pattern(pattern) => write!(f, "{}", pattern),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Function { name, params, .. } => {
                write!(f, "<fn {}({})>", name, params.join(", "))
            }
            Value::Unit => write!(f, "()"),
            Value::Array(values) => {
                write!(f, "[")?;
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            Value::EveryPattern(every) => write!(f, "{}", every),
            Value::Thunk { expression, .. } => write!(f, "<thunk: {}>", expression),
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

        // Test pattern expression (progressions are now patterns)
        let c_chord = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let f_chord = Chord::from_note_strings(vec!["F", "A", "C"]).unwrap();
        let pattern = Expression::Pattern(Pattern::from_chords(vec![c_chord, f_chord]));
        let display = format!("{}", pattern);
        assert!(display.contains("C Major") || display.contains("C"));
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

        // Test pattern value (replaces progression)
        let c_chord = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let f_chord = Chord::from_note_strings(vec!["F", "A", "C"]).unwrap();
        let pattern_val = Value::Pattern(Pattern::from_chords(vec![c_chord, f_chord]));
        let display = format!("{}", pattern_val);
        assert!(display.contains("C Major") || display.contains("C"));
        assert!(display.contains("F Major") || display.contains("F"));
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
    fn test_pattern_as_progression() {
        // Pattern is now the unified type for chord sequences
        let c_chord = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let f_chord = Chord::from_note_strings(vec!["F", "A", "C"]).unwrap();
        let g_chord = Chord::from_note_strings(vec!["G", "B", "D"]).unwrap();

        let pattern = Pattern::from_chords(vec![c_chord, f_chord, g_chord]);

        let prog_expr = Expression::Pattern(pattern.clone());
        let prog_value = Value::Pattern(pattern);

        // Test that they display correctly
        let expr_display = format!("{}", prog_expr);
        let value_display = format!("{}", prog_value);

        assert_eq!(expr_display, value_display);
        assert!(expr_display.contains("C Major") || expr_display.contains("C"));
        assert!(expr_display.contains("F Major") || expr_display.contains("F"));
        assert!(expr_display.contains("G Major") || expr_display.contains("G"));
    }
}
