//! WASM bindings for cadence-core
//!
//! Provides JavaScript-accessible functions for tokenization and parsing.

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::parser::ast::{Expression, Value};
use crate::parser::evaluator::Evaluator;
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

// ============================================================================
// Script Execution Types (for JS interop)
// ============================================================================

/// A single playback event with frequencies, duration, and rest flag
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayEventJS {
    /// Frequencies to play (empty for rest)
    pub frequencies: Vec<f32>,
    /// Duration in beats
    pub duration: f32,
    /// Whether this is a rest (silence)
    pub is_rest: bool,
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

            // Extract events, envelope, and waveform based on value type
            let (events, envelope, waveform) = match value {
                Value::Pattern(ref pattern) => {
                    let events = pattern
                        .to_events()
                        .into_iter()
                        .map(|(freqs, duration, is_rest)| PlayEventJS {
                            frequencies: freqs,
                            duration,
                            is_rest,
                        })
                        .collect();
                    let envelope = pattern.envelope;
                    let waveform = pattern.waveform.as_ref().map(|w| w.name().to_string());
                    (events, envelope, waveform)
                }
                Value::Chord(chord) => {
                    let freqs: Vec<f32> = chord.notes_vec().iter().map(|n| n.frequency()).collect();
                    let events = vec![PlayEventJS {
                        frequencies: freqs,
                        duration: 1.0, // Default 1 beat for single chord
                        is_rest: false,
                    }];
                    (events, None, None)
                }
                Value::Note(note) => {
                    let events = vec![PlayEventJS {
                        frequencies: vec![note.frequency()],
                        duration: 1.0,
                        is_rest: false,
                    }];
                    (events, None, None)
                }
                _ => return None,
            };

            Some(ActionJS::Play {
                events,
                looping: *looping,
                track_id: *track_id,
                envelope,
                waveform,
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

        // Reset environment cycle to 0
        if let Ok(mut env) = self.interpreter.environment.write() {
            let _ = env.define("_cycle".to_string(), Value::Number(0));
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

        // Update _cycle in environment
        if let Ok(mut env) = self.interpreter.environment.write() {
            let _ = env.define("_cycle".to_string(), Value::Number(self.cycle));
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
        let env = self.interpreter.environment.read().unwrap();
        let evaluator = Evaluator::new();
        let mut js_actions = Vec::new();
        let mut to_remove = Vec::new();

        for (i, (expr, looping, track_id, start_beat)) in self.active_tracks.iter().enumerate() {
            // Evaluate expression
            let value = match evaluator.eval_with_env(expr.clone(), Some(&env)) {
                Ok(v) => v,
                Err(_) => continue, // Skip error
            };

            // Convert to events (flat list with durations)
            let (events, envelope, waveform) = match value {
                Value::Pattern(ref pattern) => {
                    let evs = pattern
                        .to_events()
                        .into_iter()
                        .map(|(freqs, duration, is_rest)| PlayEventJS {
                            frequencies: freqs,
                            duration,
                            is_rest,
                        })
                        .collect();
                    let env = pattern.envelope;
                    let wav = pattern.waveform.as_ref().map(|w| w.name().to_string());
                    (evs, env, wav)
                }
                Value::Chord(chord) => {
                    let freqs: Vec<f32> = chord.notes_vec().iter().map(|n| n.frequency()).collect();
                    (
                        vec![PlayEventJS {
                            frequencies: freqs,
                            duration: 1.0,
                            is_rest: false,
                        }],
                        None,
                        None,
                    )
                }
                Value::Note(note) => (
                    vec![PlayEventJS {
                        frequencies: vec![note.frequency()],
                        duration: 1.0,
                        is_rest: false,
                    }],
                    None,
                    None,
                ),
                _ => continue,
            };

            // Calculate total duration
            let total_duration: f32 = events.iter().map(|e| e.duration).sum();
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
                (local_beat as f32 % total_duration)
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
                current_time += event.duration;
            }

            if !beat_events.is_empty() {
                js_actions.push(ActionJS::Play {
                    events: beat_events,
                    looping: *looping,
                    track_id: *track_id,
                    envelope,
                    waveform,
                });
            }
        }

        // Remove expired tracks
        for i in to_remove.into_iter().rev() {
            self.active_tracks.remove(i);
        }

        js_actions
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
