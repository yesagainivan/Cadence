//! Interpreter for executing Cadence programs
//!
//! Executes statements with side effects (audio, variable binding, control flow).

use crate::parser::ast::{Expression, Program, Statement, Value};
use crate::parser::environment::{Environment, SharedEnvironment};
use crate::parser::evaluator::Evaluator;
use crate::types::QueueMode;
use anyhow::{Result, anyhow};
use std::sync::{Arc, RwLock};

/// Control flow signals for break/continue/return
#[derive(Debug)]
pub enum ControlFlow {
    Normal,
    Break,
    Continue,
    Return(Option<Value>),
}

/// Actions to be executed by the host (REPL)
/// The Interpreter collects these; the host decides how to execute them
#[derive(Debug, Clone)]
pub enum InterpreterAction {
    /// Play an expression reactively (re-evaluated each beat for live updates)
    PlayExpression {
        expression: Expression,
        looping: bool,
        /// None = immediate play, Some(mode) = queue with specified sync mode
        queue_mode: Option<QueueMode>,
        track_id: usize,
    },
    /// Set the tempo (global)
    SetTempo(f32),
    /// Set the volume for a specific track (0.0-1.0)
    SetVolume { volume: f32, track_id: usize },
    /// Set the waveform for a specific track
    SetWaveform { waveform: String, track_id: usize },
    /// Stop playback (specific track or all)
    Stop { track_id: Option<usize> },
}

/// Interpreter for executing Cadence statements
pub struct Interpreter {
    /// Expression evaluator
    evaluator: Evaluator,
    /// Variable environment (thread-safe for reactive playback)
    pub environment: SharedEnvironment,
    /// Current tempo (BPM)
    pub tempo: f32,
    /// Current volume (0.0-1.0)
    pub volume: f32,
    /// Current track ID (default 1)
    pub current_track: usize,
    /// Whether we're inside a track N { } block
    in_track_block: bool,
    /// Last evaluated expression result
    last_eval_result: Option<Value>,
    /// Actions collected during execution (for host to execute)
    actions: Vec<InterpreterAction>,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Interpreter {
            evaluator: Evaluator::new(),
            environment: Arc::new(RwLock::new(Environment::new())),
            tempo: 120.0,
            volume: 0.5,
            current_track: 1,
            in_track_block: false,
            last_eval_result: None,
            actions: Vec::new(),
        }
    }

    /// Get a clone of the shared environment for passing to playback threads
    pub fn shared_environment(&self) -> SharedEnvironment {
        self.environment.clone()
    }

    /// Take collected actions (clears internal list)
    pub fn take_actions(&mut self) -> Vec<InterpreterAction> {
        std::mem::take(&mut self.actions)
    }

    /// Clear collected actions without returning them
    pub fn clear_actions(&mut self) {
        self.actions.clear();
    }

    /// Run a complete program
    pub fn run_program(&mut self, program: &Program) -> Result<Option<Value>> {
        let mut last_value = None;

        for stmt in &program.statements {
            match self.run_statement(stmt)? {
                ControlFlow::Normal => {}
                ControlFlow::Return(val) => return Ok(val),
                ControlFlow::Break => return Err(anyhow!("Break outside of loop")),
                ControlFlow::Continue => return Err(anyhow!("Continue outside of loop")),
            }

            // Capture last expression result
            if let Statement::Expression(_) = stmt {
                last_value = self.last_eval_result.take();
            }
        }

        Ok(last_value)
    }

    /// Run a single statement
    pub fn run_statement(&mut self, stmt: &Statement) -> Result<ControlFlow> {
        match stmt {
            Statement::Let { name, value } => {
                let val = self.eval_expression(value)?;
                self.environment.write().unwrap().define(name.clone(), val);
                Ok(ControlFlow::Normal)
            }

            Statement::Assign { name, value } => {
                let val = self.eval_expression(value)?;
                if self.environment.read().unwrap().is_defined(name) {
                    self.environment
                        .write()
                        .unwrap()
                        .set(name, val)
                        .map_err(|e| anyhow!("{}", e))?;
                } else {
                    return Err(anyhow!("Cannot assign to undefined variable '{}'", name));
                }
                Ok(ControlFlow::Normal)
            }

            Statement::Expression(expr) => {
                let val = self.eval_expression(expr)?;
                self.last_eval_result = Some(val);
                Ok(ControlFlow::Normal)
            }

            Statement::Tempo(bpm) => {
                self.tempo = *bpm;
                self.actions.push(InterpreterAction::SetTempo(*bpm));
                println!("Tempo set to {} BPM", bpm);
                Ok(ControlFlow::Normal)
            }

            Statement::Volume(vol) => {
                self.volume = *vol;
                self.actions.push(InterpreterAction::SetVolume {
                    volume: *vol,
                    track_id: self.current_track,
                });
                println!(
                    "Volume set to {:.0}% (Track {})",
                    vol * 100.0,
                    self.current_track
                );
                Ok(ControlFlow::Normal)
            }

            Statement::Waveform(name) => {
                self.actions.push(InterpreterAction::SetWaveform {
                    waveform: name.clone(),
                    track_id: self.current_track,
                });
                println!("Waveform set to {} (Track {})", name, self.current_track);
                Ok(ControlFlow::Normal)
            }

            Statement::Stop => {
                // At top-level, stop ALL tracks.
                // Inside a `track N { stop }` block, stop only that track.
                let stop_target = if self.in_track_block {
                    Some(self.current_track) // Stop specific track
                } else {
                    None // Stop all tracks
                };

                self.actions.push(InterpreterAction::Stop {
                    track_id: stop_target,
                });

                match stop_target {
                    None => println!("Stopping all playback"),
                    Some(id) => println!("Stopping playback (Track {})", id),
                }
                Ok(ControlFlow::Normal)
            }

            Statement::Play {
                target,
                looping,
                queue_mode: ast_queue_mode,
                duration: _,
            } => {
                // Validate expression can be evaluated (catch errors early)
                let val = self.eval_expression(target)?;
                // Convert string queue mode to QueueMode enum
                let queue_mode = ast_queue_mode.as_ref().map(|mode| match mode.as_str() {
                    "bar" => QueueMode::Bar,
                    "cycle" => QueueMode::Cycle,
                    _ => QueueMode::Beat, // default
                });
                self.actions.push(InterpreterAction::PlayExpression {
                    expression: target.clone(),
                    looping: *looping,
                    queue_mode,
                    track_id: self.current_track,
                });
                if *looping {
                    println!("Playing {} (looping, Track {})", val, self.current_track);
                } else {
                    println!("Playing {} (Track {})", val, self.current_track);
                }
                Ok(ControlFlow::Normal)
            }

            Statement::Track { id, body } => {
                let old_track = self.current_track;
                let old_in_block = self.in_track_block;
                self.current_track = *id;
                self.in_track_block = true;

                // Execute body
                let result = self.run_statement(body);

                // Restore track context
                self.current_track = old_track;
                self.in_track_block = old_in_block;

                result
            }

            Statement::Load(path) => {
                let contents = std::fs::read_to_string(path)
                    .map_err(|e| anyhow!("Failed to load '{}': {}", path, e))?;
                let program = crate::parser::parse_statements(&contents)
                    .map_err(|e| anyhow!("Parse error in '{}': {}", path, e))?;

                println!("Loaded: {}", path);
                self.run_program(&program)?;
                Ok(ControlFlow::Normal)
            }

            Statement::Loop { body } => loop {
                for stmt in body {
                    match self.run_statement(stmt)? {
                        ControlFlow::Normal => {}
                        ControlFlow::Break => return Ok(ControlFlow::Normal),
                        ControlFlow::Continue => break,
                        ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
                    }
                }
            },

            Statement::Repeat { count, body } => {
                for _ in 0..*count {
                    self.environment.write().unwrap().push_scope();
                    for stmt in body {
                        match self.run_statement(stmt)? {
                            ControlFlow::Normal => {}
                            ControlFlow::Break => {
                                self.environment.write().unwrap().pop_scope();
                                return Ok(ControlFlow::Normal);
                            }
                            ControlFlow::Continue => break,
                            ControlFlow::Return(val) => {
                                self.environment.write().unwrap().pop_scope();
                                return Ok(ControlFlow::Return(val));
                            }
                        }
                    }
                    self.environment.write().unwrap().pop_scope();
                }
                Ok(ControlFlow::Normal)
            }

            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond_val = self.eval_expression(condition)?;
                let is_true = match cond_val {
                    Value::Boolean(b) => b,
                    _ => return Err(anyhow!("Condition must be a boolean")),
                };

                let body = if is_true {
                    then_body
                } else {
                    match else_body {
                        Some(b) => b,
                        None => return Ok(ControlFlow::Normal),
                    }
                };

                self.environment.write().unwrap().push_scope();
                for stmt in body {
                    match self.run_statement(stmt)? {
                        ControlFlow::Normal => {}
                        cf => {
                            self.environment.write().unwrap().pop_scope();
                            return Ok(cf);
                        }
                    }
                }
                self.environment.write().unwrap().pop_scope();
                Ok(ControlFlow::Normal)
            }

            Statement::Break => Ok(ControlFlow::Break),
            Statement::Continue => Ok(ControlFlow::Continue),
            Statement::Return(expr) => {
                let val = match expr {
                    Some(e) => Some(self.eval_expression(e)?),
                    None => None,
                };
                Ok(ControlFlow::Return(val))
            }

            Statement::Comment(_) => Ok(ControlFlow::Normal),

            Statement::Block(stmts) => {
                self.environment.write().unwrap().push_scope();
                for stmt in stmts {
                    match self.run_statement(stmt)? {
                        ControlFlow::Normal => {}
                        cf => {
                            self.environment.write().unwrap().pop_scope();
                            return Ok(cf);
                        }
                    }
                }
                self.environment.write().unwrap().pop_scope();
                Ok(ControlFlow::Normal)
            }

            Statement::FunctionDef { name, params, body } => {
                // Store the function as a Value::Function in the environment
                let func_value = Value::Function {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                };
                self.environment
                    .write()
                    .unwrap()
                    .define(name.clone(), func_value);
                println!("Defined function: {}({})", name, params.join(", "));
                Ok(ControlFlow::Normal)
            }
        }
    }

    /// Evaluate an expression using the environment
    fn eval_expression(&self, expr: &crate::parser::ast::Expression) -> Result<Value> {
        // Use eval_with_env to enable variable resolution
        let env_guard = self.environment.read().unwrap();
        self.evaluator.eval_with_env(expr.clone(), Some(&env_guard))
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::statement_parser::parse_statements;

    #[test]
    fn test_let_and_tempo() {
        let mut interpreter = Interpreter::new();
        // Note: Using 120 because lexer uses i8 for numbers (max 127)
        let program = parse_statements("tempo 120").unwrap();
        interpreter.run_program(&program).unwrap();

        assert_eq!(interpreter.tempo, 120.0);
    }

    #[test]
    fn test_volume() {
        let mut interpreter = Interpreter::new();
        let program = parse_statements("volume 75").unwrap();
        interpreter.run_program(&program).unwrap();

        assert!((interpreter.volume - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_variable_resolution() {
        let mut interpreter = Interpreter::new();

        // Define a variable with let
        let program = parse_statements("let x = [C, E, G]").unwrap();
        interpreter.run_program(&program).unwrap();

        // Reference the variable - it should resolve from environment
        let program2 = parse_statements("x").unwrap();
        let result = interpreter.run_program(&program2).unwrap();

        // Should return the chord value
        assert!(result.is_some());
        match result.unwrap() {
            crate::parser::ast::Value::Chord(chord) => {
                assert_eq!(chord.len(), 3);
            }
            _ => panic!("Expected Chord value"),
        }
    }

    #[test]
    fn test_undefined_variable_error() {
        let mut interpreter = Interpreter::new();
        let program = parse_statements("undefined_var").unwrap();
        let result = interpreter.run_program(&program);

        // Should error - variable not defined
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not defined"));
    }

    #[test]
    fn test_variable_in_expression() {
        let mut interpreter = Interpreter::new();

        // Define and use variable in an expression
        let program = parse_statements("let chord = [C, E, G]").unwrap();
        interpreter.run_program(&program).unwrap();

        // Use variable in a transpose operation
        let program2 = parse_statements("chord + 2").unwrap();
        let result = interpreter.run_program(&program2).unwrap();

        // Should return transposed chord (D, F#, A)
        assert!(result.is_some());
        match result.unwrap() {
            crate::parser::ast::Value::Chord(chord) => {
                assert_eq!(chord.len(), 3);
                // D is pitch class 2
                let notes: Vec<_> = chord.notes().collect();
                assert!(notes.iter().any(|n| n.pitch_class() == 2));
            }
            _ => panic!("Expected Chord value"),
        }
    }

    #[test]
    fn test_load_file() {
        use std::io::Write;

        // Create a temp file with multi-line cadence code
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_cadence.cadence");
        {
            let mut file = std::fs::File::create(&temp_file).unwrap();
            // Use newlines to separate statements (multi-line file!)
            writeln!(file, "let x = [C, E, G]").unwrap();
            writeln!(file, "tempo 100").unwrap();
        }

        let mut interpreter = Interpreter::new();
        let load_path = temp_file.to_str().unwrap().to_string();
        let program = parse_statements(&format!("load \"{}\"", load_path)).unwrap();
        interpreter.run_program(&program).unwrap();

        // Verify the file was loaded: tempo should be 100
        assert_eq!(interpreter.tempo, 100.0);

        // Verify variable x was defined
        assert!(interpreter.environment.read().unwrap().is_defined("x"));

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_load_nonexistent_file() {
        let mut interpreter = Interpreter::new();
        let program = parse_statements("load \"nonexistent_file.cadence\"").unwrap();
        let result = interpreter.run_program(&program);

        // Should error - file doesn't exist
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to load"));
    }

    #[test]
    fn test_variable_reassignment() {
        let mut interpreter = Interpreter::new();

        // Define and then reassign
        let program = parse_statements("let x = [C, E, G]\nx = [D, F, A]").unwrap();
        let result = interpreter.run_program(&program);
        assert!(result.is_ok());

        // Verify x is now D minor
        let val = interpreter.environment.read().unwrap().get("x").cloned();
        assert!(val.is_some());
    }

    #[test]
    fn test_assign_to_undefined_variable() {
        let mut interpreter = Interpreter::new();

        // Assign without let should fail
        let program = parse_statements("x = [C, E, G]").unwrap();
        let result = interpreter.run_program(&program);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("undefined variable")
        );
    }

    #[test]
    fn test_action_flow_play() {
        let mut interpreter = Interpreter::new();

        // Play statement should generate a PlayExpression action
        let program = parse_statements("play [C, E, G]").unwrap();
        let _result = interpreter.run_program(&program);

        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            crate::parser::interpreter::InterpreterAction::PlayExpression {
                looping,
                queue_mode,
                ..
            } => {
                assert!(!looping);
                assert!(queue_mode.is_none());
            }
            _ => panic!("Expected PlayExpression action"),
        }
    }

    #[test]
    fn test_action_flow_tempo() {
        let mut interpreter = Interpreter::new();

        // Tempo statement should generate a SetTempo action
        let program = parse_statements("tempo 90").unwrap();
        let _result = interpreter.run_program(&program);

        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            crate::parser::interpreter::InterpreterAction::SetTempo(bpm) => {
                assert_eq!(*bpm, 90.0);
            }
            _ => panic!("Expected SetTempo action"),
        }
    }

    #[test]
    fn test_action_flow_multiple() {
        let mut interpreter = Interpreter::new();

        // Multiple statements should generate multiple actions
        let program = parse_statements("tempo 100\nplay [C, E, G]\nstop").unwrap();
        let _result = interpreter.run_program(&program);

        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 3);

        // Should be: SetTempo, PlayExpression, Stop
        assert!(matches!(
            actions[0],
            crate::parser::interpreter::InterpreterAction::SetTempo(_)
        ));
        assert!(matches!(
            actions[1],
            crate::parser::interpreter::InterpreterAction::PlayExpression { .. }
        ));
        assert!(matches!(
            actions[2],
            crate::parser::interpreter::InterpreterAction::Stop { .. }
        ));
    }

    // =========================================================================
    // Control Flow Tests
    // =========================================================================

    #[test]
    fn test_repeat_execution() {
        let mut interpreter = Interpreter::new();

        // repeat 3 times should execute body 3 times
        let program = parse_statements("repeat 3 { tempo 100 }").unwrap();
        interpreter.run_program(&program).unwrap();

        // Should have 3 SetTempo actions
        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 3);
        assert!(
            actions
                .iter()
                .all(|a| matches!(a, InterpreterAction::SetTempo(100.0)))
        );
    }

    #[test]
    fn test_repeat_with_break() {
        let mut interpreter = Interpreter::new();

        // Break should exit early
        let program = parse_statements("repeat 10 { tempo 100; break }").unwrap();
        interpreter.run_program(&program).unwrap();

        // Should only have 1 action (break exits after first iteration)
        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn test_loop_with_break() {
        let mut interpreter = Interpreter::new();

        // Infinite loop should exit with break
        let program = parse_statements("loop { tempo 120; break }").unwrap();
        interpreter.run_program(&program).unwrap();

        // Should have exactly 1 SetTempo action
        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], InterpreterAction::SetTempo(120.0)));
    }

    #[test]
    fn test_continue_in_repeat() {
        let mut interpreter = Interpreter::new();

        // Continue should skip to next iteration
        // Only the first tempo in each iteration should execute
        let program = parse_statements("repeat 3 { tempo 100; continue; tempo 200 }").unwrap();
        interpreter.run_program(&program).unwrap();

        let actions = interpreter.take_actions();
        // Should have 3 actions (one per iteration), all tempo 100
        assert_eq!(actions.len(), 3);
        assert!(
            actions
                .iter()
                .all(|a| matches!(a, InterpreterAction::SetTempo(100.0)))
        );
    }

    #[test]
    fn test_nested_loops() {
        let mut interpreter = Interpreter::new();

        // Nested repeat loops: outer 2x, inner 3x = 6 total
        let program = parse_statements("repeat 2 { repeat 3 { tempo 90 } }").unwrap();
        interpreter.run_program(&program).unwrap();

        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 6);
    }

    #[test]
    fn test_break_outside_loop_error() {
        let mut interpreter = Interpreter::new();

        // Break outside of loop should error
        let program = parse_statements("break").unwrap();
        let result = interpreter.run_program(&program);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Break outside of loop")
        );
    }

    #[test]
    fn test_stop_all_at_toplevel() {
        let mut interpreter = Interpreter::new();

        // At top-level (track 1), stop should target all tracks (None)
        let program = parse_statements("stop").unwrap();
        interpreter.run_program(&program).unwrap();

        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            InterpreterAction::Stop { track_id } => {
                assert_eq!(*track_id, None); // Stop ALL tracks
            }
            _ => panic!("Expected Stop action"),
        }
    }

    #[test]
    fn test_stop_specific_track() {
        let mut interpreter = Interpreter::new();

        // Inside track 2 block, stop should target only track 2
        let program = parse_statements("track 2 { stop }").unwrap();
        interpreter.run_program(&program).unwrap();

        let actions = interpreter.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            InterpreterAction::Stop { track_id } => {
                assert_eq!(*track_id, Some(2)); // Stop only track 2
            }
            _ => panic!("Expected Stop action"),
        }
    }
}
