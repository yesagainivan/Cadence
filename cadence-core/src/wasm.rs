//! WASM bindings for cadence-core
//!
//! Provides JavaScript-accessible functions for tokenization and parsing.

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

// Why are we not using the evaluator and interpreter?
#[cfg(feature = "wasm")]
use crate::parser::ast::{Expression, Value};
#[cfg(feature = "wasm")]
use crate::parser::evaluator::Evaluator;
#[cfg(feature = "wasm")]
use crate::parser::interpreter::{Interpreter, InterpreterAction};
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
    /// UTF-16 code unit offset from start of source (for JavaScript interop)
    pub utf16_start: usize,
    /// UTF-16 code unit length of token
    pub utf16_len: usize,
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
            utf16_start: token.span.utf16_offset,
            utf16_len: token.span.utf16_len,
        }
    }

    fn classify_token(token: &Token) -> String {
        match token {
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
            | Token::On
            | Token::For
            | Token::In
            | Token::Wait => "keyword".to_string(),

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
            Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent
            | Token::Ampersand
            | Token::Pipe
            | Token::Caret => "operator".to_string(),

            // Comparison
            Token::DoubleEquals
            | Token::NotEquals
            | Token::Less
            | Token::Greater
            | Token::LessEqual
            | Token::GreaterEqual => "operator.comparison".to_string(),

            // Logical operators
            Token::And | Token::Or | Token::Not => "operator.logical".to_string(),

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
            | Token::Dot
            | Token::DotDot => "punctuation".to_string(),

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
            Token::For => "for".to_string(),
            Token::In => "in".to_string(),
            Token::Wait => "wait".to_string(),
            Token::Tempo => "tempo".to_string(),
            Token::Volume => "volume".to_string(),
            Token::Waveform => "waveform".to_string(),
            Token::Queue => "queue".to_string(),
            Token::Plus => "+".to_string(),
            Token::Minus => "-".to_string(),
            Token::Star => "*".to_string(),
            Token::Slash => "/".to_string(),
            Token::Percent => "%".to_string(),
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
            Token::DotDot => "..".to_string(),
            Token::Equals => "=".to_string(),
            Token::DoubleEquals => "==".to_string(),
            Token::NotEquals => "!=".to_string(),
            Token::Less => "<".to_string(),
            Token::Greater => ">".to_string(),
            Token::LessEqual => "<=".to_string(),
            Token::GreaterEqual => ">=".to_string(),
            Token::And => "&&".to_string(),
            Token::Or => "||".to_string(),
            Token::Not => "!".to_string(),
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

/// Documentation item for a built-in function (JS-serializable)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocItemJS {
    pub name: String,
    pub category: String,
    pub description: String,
    pub signature: String,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn get_documentation() -> JsValue {
    let registry = crate::parser::builtins::get_registry();
    let docs = registry.get_documentation();

    let js_docs: Vec<DocItemJS> = docs
        .into_iter()
        .map(|d| DocItemJS {
            name: d.name,
            category: d.category,
            description: d.description,
            signature: d.signature,
        })
        .collect();

    serde_wasm_bindgen::to_value(&js_docs).unwrap_or(JsValue::NULL)
}

// ============================================================================
// Symbol Table WASM API (for Language Service features)
// ============================================================================

/// A symbol for JavaScript consumption
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind"))]
pub enum SymbolJS {
    Function {
        name: String,
        params: Vec<String>,
        signature: String,
        /// UTF-16 span start
        start: usize,
        /// UTF-16 span end
        end: usize,
    },
    Variable {
        name: String,
        value_type: Option<String>,
        /// UTF-16 span start
        start: usize,
        /// UTF-16 span end
        end: usize,
    },
}

/// Result of get_symbols call
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SymbolsResult {
    pub success: bool,
    pub symbols: Vec<SymbolJS>,
    pub error: Option<String>,
}

/// Get all symbols from a Cadence source file
/// Parses the code and binds symbols, returning them all
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn get_symbols(code: &str) -> JsValue {
    use crate::parser::binder::Binder;
    use crate::parser::statement_parser::parse_spanned_statements;

    // Parse the code
    let program = match parse_spanned_statements(code) {
        Ok(p) => p,
        Err(e) => {
            return serde_wasm_bindgen::to_value(&SymbolsResult {
                success: false,
                symbols: vec![],
                error: Some(e.to_string()),
            })
            .unwrap_or(JsValue::NULL);
        }
    };

    // Bind symbols
    let table = Binder::bind(&program);

    // Convert to JS-friendly format
    let mut symbols = Vec::new();

    for func in table.all_functions() {
        symbols.push(SymbolJS::Function {
            name: func.name.clone(),
            params: func.params.clone(),
            signature: func.signature(),
            start: func.span.utf16_start,
            end: func.span.utf16_end,
        });
    }

    for var in table.all_variables() {
        symbols.push(SymbolJS::Variable {
            name: var.name.clone(),
            value_type: var.value_type.clone(),
            start: var.span.utf16_start,
            end: var.span.utf16_end,
        });
    }

    serde_wasm_bindgen::to_value(&SymbolsResult {
        success: true,
        symbols,
        error: None,
    })
    .unwrap_or(JsValue::NULL)
}

/// Get the symbol at a specific cursor position (for hover)
/// Returns the symbol if found, or null
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn get_symbol_at_position(code: &str, position: usize) -> JsValue {
    use crate::parser::binder::Binder;
    use crate::parser::statement_parser::parse_spanned_statements;
    use crate::parser::symbols::Symbol;

    // Parse the code
    let program = match parse_spanned_statements(code) {
        Ok(p) => p,
        Err(_) => return JsValue::NULL,
    };

    // Bind symbols
    let table = Binder::bind(&program);

    // Find symbol at position
    match table.get_at_position(position) {
        Some(Symbol::Function(func)) => serde_wasm_bindgen::to_value(&SymbolJS::Function {
            name: func.name.clone(),
            params: func.params.clone(),
            signature: func.signature(),
            start: func.span.utf16_start,
            end: func.span.utf16_end,
        })
        .unwrap_or(JsValue::NULL),
        Some(Symbol::Variable(var)) => serde_wasm_bindgen::to_value(&SymbolJS::Variable {
            name: var.name.clone(),
            value_type: var.value_type.clone(),
            start: var.span.utf16_start,
            end: var.span.utf16_end,
        })
        .unwrap_or(JsValue::NULL),
        None => JsValue::NULL,
    }
}

#[cfg(feature = "wasm")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ParseResult {
    success: bool,
    error: Option<String>,
}

// ============================================================================
// Rational Time Serialization
// ============================================================================

use crate::types::{beats, Time};

/// Rational number for JS serialization (preserves exactness)
/// Serializes as { "n": numerator, "d": denominator }
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RationalJS {
    /// Numerator
    pub n: i64,
    /// Denominator
    pub d: i64,
}

impl From<Time> for RationalJS {
    fn from(t: Time) -> Self {
        RationalJS {
            n: *t.numer(),
            d: *t.denom(),
        }
    }
}

impl From<&Time> for RationalJS {
    fn from(t: &Time) -> Self {
        RationalJS {
            n: *t.numer(),
            d: *t.denom(),
        }
    }
}

impl RationalJS {
    /// Convert to f32 for internal calculations
    pub fn to_f32(self) -> f32 {
        self.n as f32 / self.d as f32
    }
}

// ============================================================================
// Script Execution Types (for JS interop)
// ============================================================================

/// Note information for a single note (JS-serializable version)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoteInfoJS {
    /// MIDI note number (0-127)
    pub midi: u8,
    /// Frequency in Hz
    pub frequency: f32,
    /// Display name with octave (e.g., "C#4", "Bb3")
    pub name: String,
    /// Pitch class (0-11): C=0, C#=1, D=2, etc.
    pub pitch_class: u8,
    /// Octave in scientific pitch notation
    pub octave: i8,
}

impl From<&crate::types::NoteInfo> for NoteInfoJS {
    fn from(info: &crate::types::NoteInfo) -> Self {
        NoteInfoJS {
            midi: info.midi,
            frequency: info.frequency,
            name: info.name.clone(),
            pitch_class: info.pitch_class,
            octave: info.octave,
        }
    }
}

/// A single playback event with rich note data for visualization and playback
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayEventJS {
    /// Rich note information (MIDI, frequency, name, etc.)
    pub notes: Vec<NoteInfoJS>,
    /// Frequencies to play (for backward compatibility with audio engine)
    pub frequencies: Vec<f32>,
    /// Drum sounds to play (for percussion)
    pub drums: Vec<String>,
    /// Start time in beats relative to pattern start (exact rational)
    pub start_beat: RationalJS,
    /// Duration in beats (exact rational)
    pub duration: RationalJS,
    /// Whether this is a rest (silence)
    pub is_rest: bool,
}

/// Pattern events with cycle timing information for visualization
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PatternEventsJS {
    /// Individual playback events
    pub events: Vec<PlayEventJS>,
    /// Total beats in one pattern cycle (exact rational, affected by fast/slow)
    pub beats_per_cycle: RationalJS,
}

/// Serializable action for JavaScript consumption
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum ActionJS {
    /// Play a pattern/chord with events
    Play {
        events: Vec<PlayEventJS>,
        looping: bool,
        track_id: usize,
        /// Custom ADSR envelope: (attack, decay, sustain, release) in seconds/level
        envelope: Option<(f32, f32, f32, f32)>,
        /// Custom waveform name
        waveform: Option<String>,
        /// Stereo pan position (0.0 = left, 0.5 = center, 1.0 = right)
        pan: Option<f32>,
    },
    /// Set the global tempo
    SetTempo { bpm: f32 },
    /// Set volume for a track
    SetVolume { volume: f32, track_id: usize },
    /// Set waveform for a track
    SetWaveform { waveform: String, track_id: usize },
    /// Stop playback
    Stop { track_id: Option<usize> },
}

/// Result of running a script
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScriptResult {
    pub success: bool,
    pub actions: Vec<ActionJS>,
    pub error: Option<String>,
    /// Console output from the interpreter (e.g., "Tempo set to 120 BPM")
    pub output: Vec<String>,
}

/// Convert interpreter actions to JS-serializable actions
#[cfg(feature = "wasm")]
fn convert_action(
    action: &crate::parser::interpreter::InterpreterAction,
    env: &crate::parser::environment::Environment,
    evaluator: &crate::parser::evaluator::Evaluator,
) -> Option<ActionJS> {
    use crate::parser::ast::Value;
    use crate::parser::interpreter::InterpreterAction;
    use crate::types::NoteInfo;

    match action {
        InterpreterAction::PlayExpression {
            expression,
            looping,
            track_id,
            ..
        } => {
            // Evaluate the expression to get a Value
            let value = evaluator
                .eval_with_env(expression.clone(), Some(env))
                .ok()?;

            // Extract events, envelope, waveform, and pan based on value type
            let (events, envelope, waveform, pan) = match value {
                Value::Pattern(ref pattern) => {
                    // Use to_rich_events() for full note identity
                    let events = pattern
                        .to_rich_events()
                        .into_iter()
                        .map(|event| PlayEventJS {
                            notes: event.notes.iter().map(NoteInfoJS::from).collect(),
                            frequencies: event.notes.iter().map(|n| n.frequency).collect(),
                            drums: event
                                .drums
                                .iter()
                                .map(|d| d.short_name().to_string())
                                .collect(),
                            start_beat: event.start_beat.into(),
                            duration: event.duration.into(),
                            is_rest: event.is_rest,
                        })
                        .collect();
                    let envelope = pattern.envelope;
                    let waveform = pattern.waveform.as_ref().map(|w| w.name().to_string());
                    let pan = pattern.pan;
                    (events, envelope, waveform, pan)
                }
                Value::Chord(chord) => {
                    // Create a rich event for a single chord
                    let note_infos: Vec<NoteInfo> =
                        chord.notes_vec().iter().map(NoteInfo::from_note).collect();
                    let events = vec![PlayEventJS {
                        notes: note_infos.iter().map(NoteInfoJS::from).collect(),
                        frequencies: note_infos.iter().map(|n| n.frequency).collect(),
                        drums: vec![],
                        start_beat: beats(0).into(),
                        duration: beats(1).into(), // Default 1 beat for single chord
                        is_rest: false,
                    }];
                    (events, None, None, None)
                }
                Value::Note(note) => {
                    // Create a rich event for a single note
                    let note_info = NoteInfo::from_note(&note);
                    let events = vec![PlayEventJS {
                        notes: vec![NoteInfoJS::from(&note_info)],
                        frequencies: vec![note_info.frequency],
                        drums: vec![],
                        start_beat: beats(0).into(),
                        duration: beats(1).into(),
                        is_rest: false,
                    }];
                    (events, None, None, None)
                }
                _ => return None,
            };

            Some(ActionJS::Play {
                events,
                looping: *looping,
                track_id: *track_id,
                envelope,
                waveform,
                pan,
            })
        }
        InterpreterAction::SetTempo(bpm) => Some(ActionJS::SetTempo { bpm: *bpm }),
        InterpreterAction::SetVolume { volume, track_id } => Some(ActionJS::SetVolume {
            volume: *volume,
            track_id: *track_id,
        }),
        InterpreterAction::SetWaveform { waveform, track_id } => Some(ActionJS::SetWaveform {
            waveform: waveform.clone(),
            track_id: *track_id,
        }),
        InterpreterAction::Stop { track_id } => Some(ActionJS::Stop {
            track_id: *track_id,
        }),
    }
}

/// Run a Cadence script and return actions for playback
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn run_script(input: &str) -> JsValue {
    use crate::parser::evaluator::Evaluator;
    use crate::parser::interpreter::Interpreter;
    use crate::parser::parse_statements;

    // Parse the input
    let program = match parse_statements(input) {
        Ok(p) => p,
        Err(e) => {
            return serde_wasm_bindgen::to_value(&ScriptResult {
                success: false,
                actions: vec![],
                error: Some(e.to_string()),
                output: vec![],
            })
            .unwrap_or(JsValue::NULL);
        }
    };

    // Run the interpreter
    let mut interpreter = Interpreter::new();
    if let Err(e) = interpreter.run_program(&program) {
        return serde_wasm_bindgen::to_value(&ScriptResult {
            success: false,
            actions: vec![],
            error: Some(e.to_string()),
            output: vec![],
        })
        .unwrap_or(JsValue::NULL);
    }

    // Get actions and convert to JS format
    let raw_actions = interpreter.take_actions();
    let env = interpreter.environment.read().unwrap();
    let evaluator = Evaluator::new();

    let actions: Vec<ActionJS> = raw_actions
        .iter()
        .filter_map(|a| convert_action(a, &env, &evaluator))
        .collect();

    serde_wasm_bindgen::to_value(&ScriptResult {
        success: true,
        actions,
        error: None,
        output: vec![], // TODO: capture stdout
    })
    .unwrap_or(JsValue::NULL)
}

/// Get play events for the statement at the given cursor position
/// This is used by the piano roll to visualize the pattern at the cursor
/// Returns PatternEventsJS with events and cycle timing info
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn get_events_at_position(code: &str, position: usize) -> JsValue {
    use crate::parser::ast::{Statement, Value};
    use crate::parser::evaluator::Evaluator;
    use crate::parser::interpreter::Interpreter;
    use crate::parser::statement_parser::parse_spanned_statements;

    // Parse with span tracking
    let spanned_program = match parse_spanned_statements(code) {
        Ok(p) => p,
        Err(_) => return JsValue::NULL,
    };

    // Find statement containing cursor position (using UTF-16 since position comes from JS)
    let spanned_stmt = match spanned_program.statement_at_utf16(position) {
        Some(s) => s,
        None => return JsValue::NULL,
    };

    // Run the full program first to populate environment
    let mut interpreter = Interpreter::new();
    let program = spanned_program.to_program();
    let _ = interpreter.run_program(&program);

    let env = interpreter.environment.read().unwrap();
    let evaluator = Evaluator::new();

    // Extract the expression to visualize
    let expr = match &spanned_stmt.statement {
        Statement::Play { target, .. } => target.clone(),
        Statement::Expression(e) => e.clone(),
        Statement::Let { value, .. } => value.clone(),
        Statement::Assign { value, .. } => value.clone(),
        Statement::Track { body, .. } => {
            // Unwrap the inner statement (typically a Play statement)
            match body.as_ref() {
                Statement::Play { target, .. } => target.clone(),
                _ => return JsValue::NULL,
            }
        }
        _ => return JsValue::NULL,
    };

    // Evaluate the expression
    let value = match evaluator.eval_with_env(expr, Some(&env)) {
        Ok(v) => v,
        Err(_) => return JsValue::NULL,
    };

    // Convert to events with cycle timing
    let result: PatternEventsJS = match value {
        Value::Pattern(ref p) => {
            let events = p
                .to_rich_events()
                .iter()
                .map(|e| PlayEventJS {
                    notes: e
                        .notes
                        .iter()
                        .map(|n| NoteInfoJS {
                            midi: n.midi,
                            frequency: n.frequency,
                            name: n.name.clone(),
                            pitch_class: n.pitch_class,
                            octave: n.octave,
                        })
                        .collect(),
                    frequencies: e.notes.iter().map(|n| n.frequency).collect(),
                    drums: e.drums.iter().map(|d| d.short_name().to_string()).collect(),
                    start_beat: e.start_beat.into(),
                    duration: e.duration.into(),
                    is_rest: e.is_rest,
                })
                .collect();
            PatternEventsJS {
                events,
                beats_per_cycle: p.beats_per_cycle.into(),
            }
        }
        Value::Chord(c) => {
            let notes: Vec<NoteInfoJS> = c
                .notes_vec()
                .iter()
                .map(|n| NoteInfoJS {
                    midi: n.midi_note(),
                    frequency: n.frequency(),
                    name: n.full_name(),
                    pitch_class: n.pitch_class(),
                    octave: n.octave(),
                })
                .collect();
            let frequencies: Vec<f32> = c.notes_vec().iter().map(|n| n.frequency()).collect();
            PatternEventsJS {
                events: vec![PlayEventJS {
                    notes,
                    frequencies,
                    drums: vec![],
                    start_beat: beats(0).into(),
                    duration: beats(1).into(),
                    is_rest: false,
                }],
                beats_per_cycle: beats(1).into(), // Single chord = 1 beat cycle
            }
        }
        Value::Note(n) => PatternEventsJS {
            events: vec![PlayEventJS {
                notes: vec![NoteInfoJS {
                    midi: n.midi_note(),
                    frequency: n.frequency(),
                    name: n.full_name(),
                    pitch_class: n.pitch_class(),
                    octave: n.octave(),
                }],
                frequencies: vec![n.frequency()],
                drums: vec![],
                start_beat: beats(0).into(),
                duration: beats(1).into(),
                is_rest: false,
            }],
            beats_per_cycle: beats(1).into(), // Single note = 1 beat cycle
        },
        Value::EveryPattern(ref every) => {
            // For piano roll visualization, show the base pattern
            // (during playback, the correct pattern is selected based on cycle)
            let p = &every.base;
            let events = p
                .to_rich_events()
                .iter()
                .map(|e| PlayEventJS {
                    notes: e
                        .notes
                        .iter()
                        .map(|n| NoteInfoJS {
                            midi: n.midi,
                            frequency: n.frequency,
                            name: n.name.clone(),
                            pitch_class: n.pitch_class,
                            octave: n.octave,
                        })
                        .collect(),
                    frequencies: e.notes.iter().map(|n| n.frequency).collect(),
                    drums: e.drums.iter().map(|d| d.short_name().to_string()).collect(),
                    start_beat: e.start_beat.into(),
                    duration: e.duration.into(),
                    is_rest: e.is_rest,
                })
                .collect();
            PatternEventsJS {
                events,
                beats_per_cycle: p.beats_per_cycle.into(),
            }
        }
        _ => return JsValue::NULL,
    };

    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

// ============================================================================
// Cursor Context API (for Properties Panel)
// ============================================================================

/// Editable properties for a cursor context
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditablePropertiesJS {
    /// Current waveform (if pattern/play)
    pub waveform: Option<String>,
    /// Current ADSR envelope: [attack, decay, sustain, release]
    pub envelope: Option<[f32; 4]>,
    /// Current tempo (if tempo statement)
    pub tempo: Option<f32>,
    /// Current volume (if volume statement)
    pub volume: Option<f32>,
    /// Beats per cycle (if pattern)
    pub beats_per_cycle: Option<f32>,
}

/// Source span information
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpanInfoJS {
    pub start: usize,
    pub end: usize,
    /// UTF-16 code unit offset for JavaScript string operations
    pub utf16_start: usize,
    /// UTF-16 code unit end position
    pub utf16_end: usize,
}

/// Cursor context for the Properties Panel
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CursorContextJS {
    /// Type of statement at cursor
    pub statement_type: String,
    /// The evaluated value type (if applicable)
    pub value_type: Option<String>,
    /// Editable properties for this context
    pub properties: Option<EditablePropertiesJS>,
    /// Source span for replacement
    pub span: SpanInfoJS,
    /// Variable name if this is a let/assign statement
    pub variable_name: Option<String>,
}

/// Get cursor context for the statement at the given position
/// Returns CursorContextJS with statement metadata and editable properties
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn get_context_at_cursor(code: &str, position: usize) -> JsValue {
    use crate::parser::ast::{Statement, Value};
    use crate::parser::evaluator::Evaluator;
    use crate::parser::interpreter::Interpreter;
    use crate::parser::statement_parser::parse_spanned_statements;

    // Parse with span tracking
    let spanned_program = match parse_spanned_statements(code) {
        Ok(p) => p,
        Err(_) => return JsValue::NULL,
    };

    // Find statement containing cursor position (using UTF-16 since position comes from JS)
    let spanned_stmt = match spanned_program.statement_at_utf16(position) {
        Some(s) => s,
        None => return JsValue::NULL,
    };

    // Run the full program to populate environment
    let mut interpreter = Interpreter::new();
    let program = spanned_program.to_program();
    let _ = interpreter.run_program(&program);

    let env = interpreter.environment.read().unwrap();
    let evaluator = Evaluator::new();

    // Determine statement type and extract expression if applicable
    let (statement_type, expr_opt, variable_name) = match &spanned_stmt.statement {
        Statement::Let { name, value } => {
            ("let".to_string(), Some(value.clone()), Some(name.clone()))
        }
        Statement::Assign { name, value } => (
            "assign".to_string(),
            Some(value.clone()),
            Some(name.clone()),
        ),
        Statement::Play { target, .. } => ("play".to_string(), Some(target.clone()), None),
        Statement::Expression(e) => ("expression".to_string(), Some(e.clone()), None),
        Statement::Tempo(expr) => {
            // Extract tempo value from expression if it's a simple number
            let tempo_val = match expr {
                Expression::Number(n) => Some(*n as f32),
                _ => None,
            };
            // Direct tempo statement
            let context = CursorContextJS {
                statement_type: "tempo".to_string(),
                value_type: Some("number".to_string()),
                properties: Some(EditablePropertiesJS {
                    waveform: None,
                    envelope: None,
                    tempo: tempo_val,
                    volume: None,
                    beats_per_cycle: None,
                }),
                span: SpanInfoJS {
                    start: spanned_stmt.start,
                    end: spanned_stmt.end,
                    utf16_start: spanned_stmt.utf16_start,
                    utf16_end: spanned_stmt.utf16_end,
                },
                variable_name: None,
            };
            return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
        }
        Statement::Volume(expr) => {
            // Extract volume value from expression if it's a simple number
            let vol_val = match expr {
                Expression::Number(n) => Some(*n as f32 / 100.0),
                _ => None,
            };
            // Direct volume statement
            let context = CursorContextJS {
                statement_type: "volume".to_string(),
                value_type: Some("number".to_string()),
                properties: Some(EditablePropertiesJS {
                    waveform: None,
                    envelope: None,
                    tempo: None,
                    volume: vol_val,
                    beats_per_cycle: None,
                }),
                span: SpanInfoJS {
                    start: spanned_stmt.start,
                    end: spanned_stmt.end,
                    utf16_start: spanned_stmt.utf16_start,
                    utf16_end: spanned_stmt.utf16_end,
                },
                variable_name: None,
            };
            return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
        }
        Statement::Waveform(name) => {
            // Direct waveform statement
            let context = CursorContextJS {
                statement_type: "waveform".to_string(),
                value_type: Some("string".to_string()),
                properties: Some(EditablePropertiesJS {
                    waveform: Some(name.clone()),
                    envelope: None,
                    tempo: None,
                    volume: None,
                    beats_per_cycle: None,
                }),
                span: SpanInfoJS {
                    start: spanned_stmt.start,
                    end: spanned_stmt.end,
                    utf16_start: spanned_stmt.utf16_start,
                    utf16_end: spanned_stmt.utf16_end,
                },
                variable_name: None,
            };
            return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
        }
        Statement::Stop => {
            let context = CursorContextJS {
                statement_type: "stop".to_string(),
                value_type: None,
                properties: None,
                span: SpanInfoJS {
                    start: spanned_stmt.start,
                    end: spanned_stmt.end,
                    utf16_start: spanned_stmt.utf16_start,
                    utf16_end: spanned_stmt.utf16_end,
                },
                variable_name: None,
            };
            return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
        }
        Statement::Comment(text) => {
            let context = CursorContextJS {
                statement_type: "comment".to_string(),
                value_type: None,
                properties: None,
                span: SpanInfoJS {
                    start: spanned_stmt.start,
                    end: spanned_stmt.end,
                    utf16_start: spanned_stmt.utf16_start,
                    utf16_end: spanned_stmt.utf16_end,
                },
                variable_name: Some(text.clone()),
            };
            return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
        }
        Statement::Track { id, body } => {
            // Unwrap the inner statement (typically a Play statement)
            let body_desc = format!("{}", body);
            match body.as_ref() {
                Statement::Play { target, .. } => {
                    // Delegate to the pattern evaluation logic below
                    (
                        format!("Track {{ id: {}, body: {} }}", id, body_desc),
                        Some(target.clone()),
                        None,
                    )
                }
                _ => {
                    // Non-play body
                    let context = CursorContextJS {
                        statement_type: format!("track {}", id),
                        value_type: None,
                        properties: None,
                        span: SpanInfoJS {
                            start: spanned_stmt.start,
                            end: spanned_stmt.end,
                            utf16_start: spanned_stmt.utf16_start,
                            utf16_end: spanned_stmt.utf16_end,
                        },
                        variable_name: None,
                    };
                    return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
                }
            }
        }
        _ => {
            // Other statement types (loop, repeat, if, etc.)
            let context = CursorContextJS {
                statement_type: format!("{:?}", spanned_stmt.statement)
                    .split('(')
                    .next()
                    .unwrap_or("unknown")
                    .to_lowercase(),
                value_type: None,
                properties: None,
                span: SpanInfoJS {
                    start: spanned_stmt.start,
                    end: spanned_stmt.end,
                    utf16_start: spanned_stmt.utf16_start,
                    utf16_end: spanned_stmt.utf16_end,
                },
                variable_name: None,
            };
            return serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL);
        }
    };

    // If we have an expression, evaluate it to get properties
    let (value_type, properties) = if let Some(expr) = expr_opt {
        match evaluator.eval_with_env(expr, Some(&env)) {
            Ok(value) => {
                let (vt, props) = match value {
                    Value::Pattern(ref p) => {
                        let props = EditablePropertiesJS {
                            waveform: p.waveform.as_ref().map(|w| w.name().to_string()),
                            envelope: p.envelope.map(|(a, d, s, r)| [a, d, s, r]),
                            tempo: None,
                            volume: None,
                            beats_per_cycle: Some(p.beats_per_cycle_f32()),
                        };
                        ("pattern".to_string(), Some(props))
                    }
                    Value::Chord(_) => ("chord".to_string(), None),
                    Value::Note(_) => ("note".to_string(), None),
                    Value::Number(_) => ("number".to_string(), None),
                    Value::String(_) => ("string".to_string(), None),
                    Value::Boolean(_) => ("boolean".to_string(), None),
                    Value::Function { .. } => ("function".to_string(), None),
                    Value::Unit => ("unit".to_string(), None),
                    Value::Array(_) => ("array".to_string(), None),
                    Value::EveryPattern(ref every) => {
                        // For EveryPattern, expose properties from the base pattern
                        let props = EditablePropertiesJS {
                            waveform: every.base.waveform.as_ref().map(|w| w.name().to_string()),
                            envelope: every.base.envelope.map(|(a, d, s, r)| [a, d, s, r]),
                            tempo: None,
                            volume: None,
                            beats_per_cycle: Some(every.base.beats_per_cycle_f32()),
                        };
                        ("every_pattern".to_string(), Some(props))
                    }
                };
                (Some(vt), props)
            }
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    let context = CursorContextJS {
        statement_type,
        value_type,
        properties,
        span: SpanInfoJS {
            start: spanned_stmt.start,
            end: spanned_stmt.end,
            utf16_start: spanned_stmt.utf16_start,
            utf16_end: spanned_stmt.utf16_end,
        },

        variable_name,
    };

    serde_wasm_bindgen::to_value(&context).unwrap_or(JsValue::NULL)
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct WasmInterpreter {
    interpreter: Interpreter, // Interpreter holds the environment
    cycle: i32,
    // Store expressions to re-evaluate: (expression, looping, track_id, start_beat)
    active_tracks: Vec<(Expression, bool, usize, i32)>,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl WasmInterpreter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        WasmInterpreter {
            interpreter: Interpreter::new(),
            cycle: 0,
            active_tracks: Vec::new(),
        }
    }

    /// Load and execute a script, setting up the environment and active tracks
    pub fn load(&mut self, code: &str) -> JsValue {
        use crate::parser::parse_statements;

        // Reset state
        self.active_tracks.clear();
        self.cycle = 0;
        self.interpreter.clear_actions(); // Clear previous actions

        // Reset environment cycle and beat to 0
        if let Ok(mut env) = self.interpreter.environment.write() {
            let _ = env.define("_cycle".to_string(), Value::Number(0));
            let _ = env.define("_beat".to_string(), Value::Number(0));
        }

        // Parse
        let program = match parse_statements(code) {
            Ok(p) => p,
            Err(e) => {
                return serde_wasm_bindgen::to_value(&ScriptResult {
                    success: false,
                    actions: vec![],
                    error: Some(e.to_string()),
                    output: vec![],
                })
                .unwrap_or(JsValue::NULL)
            }
        };

        // Run
        if let Err(e) = self.interpreter.run_program(&program) {
            return serde_wasm_bindgen::to_value(&ScriptResult {
                success: false,
                actions: vec![],
                error: Some(e.to_string()),
                output: vec![],
            })
            .unwrap_or(JsValue::NULL);
        }

        // Process actions to find Play commands and store them
        let actions = self.interpreter.take_actions();
        let mut js_actions = Vec::new();

        {
            let env = self.interpreter.environment.read().unwrap();
            let evaluator = Evaluator::new();

            for action in actions {
                match action {
                    InterpreterAction::PlayExpression {
                        expression,
                        looping,
                        track_id,
                        ..
                    } => {
                        // Store for reactive playback, starting at cycle 0
                        self.active_tracks
                            .push((expression.clone(), looping, track_id, 0));

                        // Note: We DO NOT immediately convert/play here.
                        // The first call to tick() (triggered by audio-engine) will handle Beat 0.
                    }
                    other => {
                        // Pass through other actions (tempo, volume, etc.)
                        if let Some(js_action) = convert_action(&other, &env, &evaluator) {
                            js_actions.push(js_action);
                        }
                    }
                }
            }
        }

        // Generate Beat 0 events immediately to avoid delay
        if !self.active_tracks.is_empty() {
            let beat_actions = self.generate_beat_events(0);
            js_actions.extend(beat_actions);
        }

        serde_wasm_bindgen::to_value(&ScriptResult {
            success: true,
            actions: js_actions,
            error: None,
            output: vec![],
        })
        .unwrap_or(JsValue::NULL)
    }

    /// Update the script without resetting the cycle counter (for live coding)
    pub fn update(&mut self, code: &str) -> JsValue {
        use crate::parser::parse_statements;

        let current_cycle = self.cycle;

        // Clear active tracks
        self.active_tracks.clear();
        self.interpreter.clear_actions();

        // Parse
        let program = match parse_statements(code) {
            Ok(p) => p,
            Err(e) => {
                return serde_wasm_bindgen::to_value(&ScriptResult {
                    success: false,
                    actions: vec![],
                    error: Some(e.to_string()),
                    output: vec![],
                })
                .unwrap_or(JsValue::NULL)
            }
        };

        // Run (will update variables in existing environment)
        if let Err(e) = self.interpreter.run_program(&program) {
            return serde_wasm_bindgen::to_value(&ScriptResult {
                success: false,
                actions: vec![],
                error: Some(e.to_string()),
                output: vec![],
            })
            .unwrap_or(JsValue::NULL);
        }

        // Process actions to find Play commands and store them
        let actions = self.interpreter.take_actions();
        let mut js_actions = Vec::new();
        let env = self.interpreter.environment.read().unwrap();
        let evaluator = Evaluator::new();

        for action in actions {
            match action {
                InterpreterAction::PlayExpression {
                    expression,
                    looping,
                    track_id,
                    ..
                } => {
                    // Fix for phase reset on live update:
                    // - Looping tracks should align with global grid (start_beat = 0)
                    //   so they continue smoothly even if code changes.
                    // - One-shot tracks should start fresh at the next beat (start_beat = current_cycle + 1).
                    let start_beat = if looping { 0 } else { current_cycle + 1 };

                    self.active_tracks
                        .push((expression.clone(), looping, track_id, start_beat));
                }
                other => {
                    if let Some(js_action) = convert_action(&other, &env, &evaluator) {
                        js_actions.push(js_action);
                    }
                }
            }
        }

        serde_wasm_bindgen::to_value(&ScriptResult {
            success: true,
            actions: js_actions,
            error: None,
            output: vec![],
        })
        .unwrap_or(JsValue::NULL)
    }

    /// Advance time by one beat and re-evaluate active tracks
    pub fn tick(&mut self) -> JsValue {
        use crate::parser::ast::Value;

        // Increment cycle
        self.cycle += 1;

        // Update _cycle and _beat in environment
        if let Ok(mut env) = self.interpreter.environment.write() {
            let _ = env.define("_cycle".to_string(), Value::Number(self.cycle));
            let _ = env.define("_beat".to_string(), Value::Number(self.cycle));
        }

        let js_actions = self.generate_beat_events(self.cycle);

        serde_wasm_bindgen::to_value(&ScriptResult {
            success: true,
            actions: js_actions,
            error: None,
            output: vec![],
        })
        .unwrap_or(JsValue::NULL)
    }

    // Helper to generate events for a specific beat index
    fn generate_beat_events(&mut self, beat: i32) -> Vec<ActionJS> {
        use crate::types::NoteInfo;

        let evaluator = Evaluator::new();
        let mut js_actions = Vec::new();
        let mut to_remove = Vec::new();

        for (i, (expr, looping, track_id, start_beat)) in self.active_tracks.iter().enumerate() {
            // Calculate local beat for this track
            let local_beat = beat - start_beat;

            // We need to first evaluate to get pattern duration, then calculate cycle
            // For now, do an initial evaluation to get the pattern duration
            let env_read = self.interpreter.environment.read().unwrap();
            let initial_value = match evaluator.eval_with_env(expr.clone(), Some(&env_read)) {
                Ok(v) => v,
                Err(_) => continue,
            };
            drop(env_read);

            // Get pattern duration from initial evaluation
            let total_duration: f32 = match &initial_value {
                Value::Pattern(ref pattern) => pattern
                    .to_rich_events()
                    .iter()
                    .map(|e| e.duration_f32())
                    .sum(),
                Value::EveryPattern(ref every) => {
                    // Use base pattern duration (base and transformed should have same duration)
                    every
                        .base
                        .to_rich_events()
                        .iter()
                        .map(|e| e.duration_f32())
                        .sum()
                }
                _ => 1.0,
            };

            // Calculate the pattern cycle number for this track
            let pattern_cycle = if total_duration > 0.0 {
                (local_beat as f32 / total_duration).floor() as i32
            } else {
                0
            };

            // Set _cycle to the pattern cycle for this track before re-evaluation
            {
                let mut env_write = self.interpreter.environment.write().unwrap();
                let _ = env_write.define("_cycle".to_string(), Value::Number(pattern_cycle));
            }

            // Re-evaluate with the correct _cycle set
            let env_read = self.interpreter.environment.read().unwrap();
            let value = match evaluator.eval_with_env(expr.clone(), Some(&env_read)) {
                Ok(v) => v,
                Err(_) => continue, // Skip error
            };
            drop(env_read);

            // Convert to rich events (with full note identity)
            let (events, envelope, waveform, pan) = match value {
                Value::Pattern(ref pattern) => {
                    let evs = pattern
                        .to_rich_events()
                        .into_iter()
                        .map(|event| PlayEventJS {
                            notes: event.notes.iter().map(NoteInfoJS::from).collect(),
                            frequencies: event.notes.iter().map(|n| n.frequency).collect(),
                            drums: event
                                .drums
                                .iter()
                                .map(|d| d.short_name().to_string())
                                .collect(),
                            start_beat: event.start_beat.into(),
                            duration: event.duration.into(),
                            is_rest: event.is_rest,
                        })
                        .collect();
                    let env = pattern.envelope;
                    let wav = pattern.waveform.as_ref().map(|w| w.name().to_string());
                    let pan = pattern.pan;
                    (evs, env, wav, pan)
                }
                Value::Chord(chord) => {
                    let note_infos: Vec<NoteInfo> =
                        chord.notes_vec().iter().map(NoteInfo::from_note).collect();
                    (
                        vec![PlayEventJS {
                            notes: note_infos.iter().map(NoteInfoJS::from).collect(),
                            frequencies: note_infos.iter().map(|n| n.frequency).collect(),
                            drums: vec![],
                            start_beat: beats(0).into(),
                            duration: beats(1).into(),
                            is_rest: false,
                        }],
                        None,
                        None,
                        None,
                    )
                }
                Value::Note(note) => {
                    let note_info = NoteInfo::from_note(&note);
                    (
                        vec![PlayEventJS {
                            notes: vec![NoteInfoJS::from(&note_info)],
                            frequencies: vec![note_info.frequency],
                            drums: vec![],
                            start_beat: beats(0).into(),
                            duration: beats(1).into(),
                            is_rest: false,
                        }],
                        None,
                        None,
                        None,
                    )
                }
                Value::EveryPattern(ref every) => {
                    // Select the appropriate pattern based on pattern_cycle
                    let pattern = every.get_pattern_for_cycle(pattern_cycle as usize);
                    let evs = pattern
                        .to_rich_events()
                        .into_iter()
                        .map(|event| PlayEventJS {
                            notes: event.notes.iter().map(NoteInfoJS::from).collect(),
                            frequencies: event.notes.iter().map(|n| n.frequency).collect(),
                            drums: event
                                .drums
                                .iter()
                                .map(|d| d.short_name().to_string())
                                .collect(),
                            start_beat: event.start_beat.into(),
                            duration: event.duration.into(),
                            is_rest: event.is_rest,
                        })
                        .collect();
                    let env = pattern.envelope;
                    let wav = pattern.waveform.as_ref().map(|w| w.name().to_string());
                    let pan = pattern.pan;
                    (evs, env, wav, pan)
                }
                _ => continue,
            };

            // Calculate total duration
            let total_duration: f32 = events.iter().map(|e| e.duration.to_f32()).sum();
            let total_duration_int = total_duration.ceil() as i32;

            // Calculate local beat index
            let local_beat = beat - start_beat;

            // Handle non-looping expiration
            if !looping && local_beat >= total_duration_int {
                to_remove.push(i);
                continue;
            }

            // Calculate wrap for looping
            let effective_beat = if total_duration > 0.0 {
                local_beat as f32 % total_duration
            } else {
                0.0
            };

            // FILTER: Return events starting in [effective_beat, effective_beat + 1.0)
            let mut current_time = 0.0;
            let mut beat_events = Vec::new();

            for event in events {
                // Check if event falls within this beat window
                // Simple logic for integer beats:
                // If effective_beat is integer (e.g. 0.0), we want events starting >= 0.0 and < 1.0.
                // This effectively "consumes" 1 beat of the pattern.

                // Note: this assumes `tick` handles exactly 1 beat.
                if current_time >= effective_beat && current_time < (effective_beat + 0.99) {
                    beat_events.push(event.clone());
                }
                current_time += event.duration.to_f32();
            }

            if !beat_events.is_empty() {
                js_actions.push(ActionJS::Play {
                    events: beat_events,
                    looping: *looping,
                    track_id: *track_id,
                    envelope,
                    waveform,
                    pan,
                });
            }
        }

        // Remove expired tracks
        for i in to_remove.into_iter().rev() {
            self.active_tracks.remove(i);
        }

        js_actions
    }

    /// Get user-defined functions from the environment as DocItems (for hover)
    pub fn get_user_functions(&self) -> JsValue {
        let env = self.interpreter.environment.read().unwrap();
        let mut docs: Vec<DocItemJS> = Vec::new();

        for (name, value) in env.all_bindings() {
            // Skip internal variables (starting with _)
            if name.starts_with('_') {
                continue;
            }
            if let Value::Function { params, .. } = value {
                docs.push(DocItemJS {
                    name: name.clone(),
                    category: "User".to_string(),
                    description: "User-defined function".to_string(),
                    signature: format!("fn {}({})", name, params.join(", ")),
                });
            }
        }

        serde_wasm_bindgen::to_value(&docs).unwrap_or(JsValue::NULL)
    }

    /// Define/Updates a global variable
    pub fn set_variable(&mut self, _name: &str, _json_value: JsValue) {
        // This would require parsing JSON to Value, simpler to just re-eval small snippets for now
        // or just expose a method to run a small statement.
        // For now, MVP: only load() and tick() are enough for reactive cycles.
    }
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
