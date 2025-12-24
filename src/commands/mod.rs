//! Command registry for REPL commands
//!
//! Provides a clean, extensible pattern for handling REPL commands.

pub mod audio;
pub mod general;

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::clock::MasterClock;
use crate::audio::playback_engine::PlaybackEngine;
use crate::parser::{Value, eval};
use std::sync::Arc;

/// Result of executing a command
#[derive(Debug)]
pub enum CommandResult {
    /// Command executed successfully, continue REPL
    Success,
    /// Command executed, show this message
    Message(String),
    /// Exit the REPL
    Exit,
    /// Not a command, try evaluating as expression
    NotACommand,
    /// Error occurred
    Error(String),
}

/// Context passed to command handlers
pub struct CommandContext {
    pub audio_handle: Arc<AudioPlayerHandle>,
    pub clock: Arc<MasterClock>,
    pub playback_engine: Arc<PlaybackEngine>,
}

impl CommandContext {
    pub fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        clock: Arc<MasterClock>,
        playback_engine: Arc<PlaybackEngine>,
    ) -> Self {
        Self {
            audio_handle,
            clock,
            playback_engine,
        }
    }

    /// Evaluate an expression string
    pub fn eval(&self, input: &str) -> anyhow::Result<Value> {
        eval(input)
    }
}

/// A command handler function
pub type CommandHandler = fn(&str, &mut CommandContext) -> CommandResult;

/// Registry of available commands
pub struct CommandRegistry {
    /// Commands indexed by their prefix (e.g., "audio play progression")
    /// Sorted by prefix length descending for longest-match-first lookup
    commands: Vec<(String, CommandHandler)>,
}

impl CommandRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Register a command with its prefix
    pub fn register(&mut self, prefix: &str, handler: CommandHandler) {
        self.commands.push((prefix.to_string(), handler));
        // Sort by prefix length descending for longest-match-first
        self.commands.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    }

    /// Execute a command, returning NotACommand if no match found
    pub fn execute(&self, input: &str, ctx: &mut CommandContext) -> CommandResult {
        for (prefix, handler) in &self.commands {
            if input == prefix || input.starts_with(&format!("{} ", prefix)) {
                let args = if input.len() > prefix.len() {
                    input[prefix.len()..].trim()
                } else {
                    ""
                };
                return handler(args, ctx);
            }
        }
        CommandResult::NotACommand
    }

    /// Get all registered command prefixes
    pub fn list_commands(&self) -> Vec<&str> {
        self.commands.iter().map(|(p, _)| p.as_str()).collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a fully populated command registry with all built-in commands
pub fn create_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Register commands (order matters for prefix matching - register specific first)
    registry.register("audio play progression", audio::cmd_audio_play_progression);
    registry.register("audio play", audio::cmd_audio_play);
    registry.register("audio stop", audio::cmd_audio_stop);
    registry.register("audio volume", audio::cmd_audio_volume);
    registry.register("tempo", general::cmd_tempo);
    registry.register("help", general::cmd_help);
    registry.register("quit", general::cmd_quit);
    registry.register("exit", general::cmd_quit);

    registry
}
