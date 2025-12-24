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
use std::sync::Arc;

/// Interactive REPL for the Cadence language
pub struct Repl {
    editor: DefaultEditor,
    audio_handle: Arc<AudioPlayerHandle>,
    scheduler: Arc<Scheduler>,
    playback_engine: Arc<PlaybackEngine>,
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
        let playback_engine =
            Arc::new(PlaybackEngine::new(audio_handle.clone(), scheduler.clone()));

        Ok(Repl {
            editor,
            audio_handle,
            scheduler,
            playback_engine,
            interpreter: Interpreter::new(),
        })
    }

    /// Execute an interpreter action (triggers actual audio/state changes)
    fn execute_action(&self, action: InterpreterAction, _ctx: &mut CommandContext) {
        use crate::audio::playback_engine::ProgressionConfig;

        match action {
            InterpreterAction::PlayValue {
                value,
                looping,
                queue,
            } => {
                // Convert value to frequencies and create config
                let frequencies: Vec<Vec<f32>> = match value {
                    crate::parser::Value::Chord(chord) => {
                        vec![chord.notes().map(|n| n.frequency()).collect()]
                    }
                    crate::parser::Value::Progression(progression) => progression
                        .chords()
                        .map(|c| c.notes().map(|n| n.frequency()).collect())
                        .collect(),
                    crate::parser::Value::Note(note) => {
                        vec![vec![note.frequency()]]
                    }
                    _ => {
                        println!("{}", "Cannot play this value type".red());
                        return;
                    }
                };

                let mut config = ProgressionConfig::new(frequencies);
                if looping {
                    config = config.with_looping();
                }

                if queue {
                    if let Err(e) = self.playback_engine.queue_progression(config) {
                        println!("{} {}", "Playback error:".red(), e);
                    } else {
                        println!("🔁 Queued for next beat...");
                    }
                } else {
                    if let Err(e) = self.playback_engine.play_progression(config) {
                        println!("{} {}", "Playback error:".red(), e);
                    } else {
                        println!("🔊 Audio playback started.");
                    }
                }
            }
            InterpreterAction::SetTempo(bpm) => {
                self.scheduler.set_bpm(bpm);
                // Already printed by interpreter
            }
            InterpreterAction::SetVolume(_vol) => {
                // Volume control would go here
            }
            InterpreterAction::Stop => {
                if let Err(e) = self.playback_engine.stop() {
                    println!("{} {}", "Stop error:".red(), e);
                }
                // Already printed by interpreter
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
        let registry = create_registry();
        let mut ctx = CommandContext::new(
            self.audio_handle.clone(),
            self.scheduler.clone(),
            self.playback_engine.clone(),
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
