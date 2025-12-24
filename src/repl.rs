//! REPL (Read-Eval-Print Loop) for the Cadence language

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::playback_engine::PlaybackEngine;
use crate::audio::scheduler::Scheduler;
use crate::commands::{CommandContext, CommandResult, create_registry};
use crate::parser::eval;
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
        })
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
                            // Not a command, try evaluating as expression
                            match eval(line) {
                                Ok(value) => {
                                    println!("{}", value);
                                }
                                Err(e) => {
                                    println!(
                                        "{} {}",
                                        "Error:".bright_red().bold(),
                                        e.to_string().red()
                                    );
                                }
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

    #[test]
    fn test_evaluate_expression() {
        // Test basic note evaluation
        let result = eval("C");
        assert!(result.is_ok());

        // Test chord evaluation
        let result = eval("[C, E, G]");
        assert!(result.is_ok());

        // Test arithmetic
        let result = eval("[C, E, G] + 2");
        assert!(result.is_ok());

        // Test function call
        let result = eval("invert([C, E, G])");
        assert!(result.is_ok());

        // Test error handling
        let result = eval("invalid syntax @#$");
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_set_operations() {
        // Test intersection
        let result = eval("[C, E, G] & [A, C, E]");
        assert!(result.is_ok());

        // Test union
        let result = eval("[C, E, G] | [F, A, C]");
        assert!(result.is_ok());

        // Test difference
        let result = eval("[C, E, G] ^ [A, C, E]");
        assert!(result.is_ok());
    }

    #[test]
    fn test_evaluate_complex_expressions() {
        // Test nested operations
        let result = eval("invert([C, E, G] + 2)");
        assert!(result.is_ok());

        // Test function composition
        let result = eval("bass(invert([C, E, G]))");
        assert!(result.is_ok());

        // Test complex set operations
        let result = eval("[C, E, G] + 2 & [A, C, E]");
        assert!(result.is_ok());
    }
}
