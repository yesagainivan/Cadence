//! REPL (Read-Eval-Print Loop) for the Cadence language

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::playback_engine::PlaybackEngine;
use crate::audio::scheduler::Scheduler;
use crate::commands::{CommandContext, CommandResult, create_registry};
use crate::parser::{Interpreter, InterpreterAction, parse_statements};
use anyhow::Result;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RustylineResult};
use std::collections::HashMap;
use std::sync::Arc;

/// Interactive REPL for the Cadence language
pub struct Repl {
    editor: DefaultEditor,
    audio_handle: Arc<AudioPlayerHandle>,
    scheduler: Arc<Scheduler>,
    // Map of track ID to playback engine
    playback_engines: HashMap<usize, Arc<PlaybackEngine>>,
    /// Interpreter for scripting constructs
    interpreter: Interpreter,
}

impl Repl {
    /// Create a new REPL instance
    pub fn new() -> RustylineResult<Self> {
        let editor = DefaultEditor::new()?;
        let audio_handle =
            Arc::new(AudioPlayerHandle::new().expect("Failed to create audio player"));
        let scheduler = Arc::new(Scheduler::new(90.0)); // Default 90 BPM

        // Initialize with default track 1
        let mut playback_engines = HashMap::new();
        let default_track = 1;
        let engine = Arc::new(PlaybackEngine::new(
            audio_handle.clone(),
            scheduler.clone(),
            default_track,
        ));
        playback_engines.insert(default_track, engine);

        Ok(Repl {
            editor,
            audio_handle,
            scheduler,
            playback_engines,
            interpreter: Interpreter::new(),
        })
    }

    /// Maximum number of tracks allowed
    const MAX_TRACKS: usize = 16;

    /// Get or create a playback engine for a specific track
    fn get_engine(&mut self, track_id: usize) -> Arc<PlaybackEngine> {
        if let Some(engine) = self.playback_engines.get(&track_id) {
            return engine.clone();
        }

        // Check track limit before creating new track
        if self.playback_engines.len() >= Self::MAX_TRACKS {
            println!(
                "{}",
                format!(
                    "⚠️  Maximum {} tracks reached. Cannot create track {}.",
                    Self::MAX_TRACKS,
                    track_id
                )
                .bright_yellow()
            );
            // Return track 1 as fallback
            return self.playback_engines.get(&1).unwrap().clone();
        }

        let engine = Arc::new(PlaybackEngine::new(
            self.audio_handle.clone(),
            self.scheduler.clone(),
            track_id,
        ));
        self.playback_engines.insert(track_id, engine.clone());
        engine
    }

    /// List all active tracks and their status
    pub fn list_tracks(&self) -> String {
        let mut track_ids: Vec<_> = self.playback_engines.keys().collect();
        track_ids.sort();

        let mut output = format!(
            "🎛️  Active Tracks ({}/{}):\n",
            track_ids.len(),
            Self::MAX_TRACKS
        );
        for &id in &track_ids {
            if let Some(engine) = self.playback_engines.get(id) {
                let status = if engine.is_playing() {
                    "▶ playing".bright_green()
                } else {
                    "⏹ stopped".bright_black()
                };
                output.push_str(&format!("  Track {}: {}\n", id, status));
            }
        }
        output
    }

    /// Execute an interpreter action (triggers actual audio/state changes)
    fn execute_action(&mut self, action: InterpreterAction, _ctx: &mut CommandContext) {
        use crate::audio::playback_engine::ProgressionConfig;
        use crate::parser::Evaluator;

        match action {
            InterpreterAction::PlayExpression {
                expression,
                looping,
                queue,
                track_id,
            } => {
                let engine = self.get_engine(track_id);

                // Get the shared environment for reactive evaluation
                let shared_env = self.interpreter.shared_environment();

                // Validate expression can be evaluated (catch errors early)
                let evaluator = Evaluator::new();
                let display_value = {
                    let env_guard = shared_env.read().unwrap();
                    match evaluator.eval_with_env(expression.clone(), Some(&env_guard)) {
                        Ok(v) => v,
                        Err(e) => {
                            println!("{} Failed to evaluate expression: {}", "Error:".red(), e);
                            return;
                        }
                    }
                };

                // Use reactive playback - the expression will be re-evaluated on each beat
                // This enables live variable updates: `play a loop` then `a = E` will
                // change the sound on the next beat!
                let mut config = ProgressionConfig::new_reactive(expression, shared_env);
                if looping {
                    config = config.with_looping();
                }

                if queue {
                    if let Err(e) = engine.queue_progression(config) {
                        println!("{} {}", "Playback error:".red(), e);
                    } else {
                        println!(
                            "🔁 Queued {} for next beat... (Track {})",
                            display_value, track_id
                        );
                    }
                } else {
                    if let Err(e) = engine.play_progression(config) {
                        println!("{} {}", "Playback error:".red(), e);
                    } else {
                        println!(
                            "🔊 Playing {} (Track {}) - live reactive!",
                            display_value, track_id
                        );
                    }
                }
            }
            InterpreterAction::SetTempo(bpm) => {
                self.scheduler.set_bpm(bpm);
                // Already printed by interpreter
            }
            InterpreterAction::SetVolume { volume, track_id } => {
                let engine = self.get_engine(track_id);
                if let Err(e) = engine.set_volume(volume) {
                    println!("{} {} (Track {})", "Volume error:".red(), e, track_id);
                }
            }
            InterpreterAction::Stop { track_id } => {
                match track_id {
                    Some(id) => {
                        let engine = self.get_engine(id);
                        if let Err(e) = engine.stop() {
                            println!("{} {} (Track {})", "Stop error:".red(), e, id);
                        }
                    }
                    None => {
                        // Stop all tracks
                        for (id, engine) in &self.playback_engines {
                            if let Err(e) = engine.stop() {
                                println!("{} {} (Track {})", "Stop error:".red(), e, id);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Start the REPL loop
    pub fn run(&mut self) -> Result<()> {
        println!(
            "{} {}",
            "🎵".bright_yellow(),
            "Cadence Music Programming Language".bright_cyan().bold()
        );
        println!(
            "Type expressions like: {}, {}, {}",
            "[C, E, G]".cyan(),
            "[C, E, G] + 2".cyan(),
            "invert([C, E, G])".cyan()
        );
        println!(
            "Type '{}' for more information, '{}' or {} to exit.\n",
            "help".bright_green(),
            "quit".bright_red(),
            "Ctrl+C".bright_red()
        );

        // Create command registry and context
        // Use track 1 engine for global context for now
        let default_engine = self.get_engine(1);
        let registry = create_registry();
        let mut ctx = CommandContext::new(
            self.audio_handle.clone(),
            self.scheduler.clone(),
            default_engine,
        );

        loop {
            let prompt = format!("{} ", "cadence>".bright_magenta().bold());
            match self.editor.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    self.editor.add_history_entry(line.to_owned())?;

                    // Handle REPL-specific commands (needs access to playback_engines)
                    if line == "tracks" {
                        println!("{}", self.list_tracks());
                        continue;
                    }

                    // Try to execute as a command
                    match registry.execute(line, &mut ctx) {
                        CommandResult::Success => {
                            // Command executed, no output needed
                        }
                        CommandResult::Message(msg) => {
                            println!("{}", msg);
                        }
                        CommandResult::Exit => {
                            println!("{} 🎵", "Goodbye!".bright_cyan());
                            break;
                        }
                        CommandResult::Error(e) => {
                            println!("{} {}", "Error:".bright_red().bold(), e.red());
                        }
                        CommandResult::NotACommand => {
                            // Parse and execute as statement(s)
                            match parse_statements(line) {
                                Ok(program) => {
                                    match self.interpreter.run_program(&program) {
                                        Ok(Some(value)) => println!("{}", value),
                                        Ok(None) => {} // Statement with no value
                                        Err(e) => println!(
                                            "{} {}",
                                            "Error:".bright_red().bold(),
                                            e.to_string().red()
                                        ),
                                    }

                                    // Execute collected actions
                                    for action in self.interpreter.take_actions() {
                                        self.execute_action(action, &mut ctx);
                                    }
                                }
                                Err(e) => println!(
                                    "{} {}",
                                    "Parse error:".bright_red().bold(),
                                    e.to_string().red()
                                ),
                            }
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("{} 🎵", "Goodbye!".bright_cyan());
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("{} 🎵", "Goodbye!".bright_cyan());
                    break;
                }
                Err(err) => {
                    println!(
                        "{} {}",
                        "Error reading input:".bright_red().bold(),
                        err.to_string().red()
                    );
                }
            }
        }

        Ok(())
    }
}

impl Default for Repl {
    fn default() -> Self {
        Self::new().expect("Failed to create REPL")
    }
}

/// Convenience function to start the REPL
pub fn start() -> Result<()> {
    let mut repl = Repl::new().map_err(|e| anyhow::anyhow!("Failed to initialize REPL: {}", e))?;
    repl.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_creation() {
        // Test that we can create a REPL instance
        let result = Repl::new();
        assert!(result.is_ok());
    }

    fn run_statement(input: &str) -> bool {
        let program = parse_statements(input);
        if program.is_err() {
            return false;
        }
        let mut interpreter = Interpreter::new();
        interpreter.run_program(&program.unwrap()).is_ok()
    }

    #[test]
    fn test_evaluate_expression() {
        // Test basic note evaluation
        assert!(run_statement("C"));

        // Test chord evaluation
        assert!(run_statement("[C, E, G]"));

        // Test arithmetic
        assert!(run_statement("[C, E, G] + 2"));

        // Test function call
        assert!(run_statement("invert([C, E, G])"));

        // Test error handling
        assert!(!run_statement("invalid syntax @#$"));
    }

    #[test]
    fn test_evaluate_set_operations() {
        // Test intersection
        assert!(run_statement("[C, E, G] & [A, C, E]"));

        // Test union
        assert!(run_statement("[C, E, G] | [F, A, C]"));

        // Test difference
        assert!(run_statement("[C, E, G] ^ [A, C, E]"));
    }

    #[test]
    fn test_evaluate_complex_expressions() {
        // Test nested operations
        assert!(run_statement("invert([C, E, G] + 2)"));

        // Test function composition
        assert!(run_statement("bass(invert([C, E, G]))"));

        // Test complex set operations
        assert!(run_statement("[C, E, G] + 2 & [A, C, E]"));
    }

    #[test]
    fn test_scripting_statements() {
        // Test tempo
        assert!(run_statement("tempo 120"));

        // Test stop
        assert!(run_statement("stop"));

        // Test let (basic parsing)
        assert!(run_statement("let x = [C, E, G]"));
    }
}
