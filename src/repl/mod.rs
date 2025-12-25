//! REPL (Read-Eval-Print Loop) for the Cadence language

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::clock::MasterClock;
use crate::audio::midi::MidiOutputHandle;
use crate::audio::playback_engine::PlaybackEngine;
use crate::commands::{CommandContext, CommandResult, create_registry};
use crate::parser::{Interpreter, InterpreterAction, parse_statements};
use crate::repl::watcher::FileWatcher;
use anyhow::Result;
use colored::*;
use crossbeam_channel::{Receiver, Sender, unbounded};
use notify::Event;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RustylineResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::thread;

pub mod watcher;

/// Types of events the REPL loop handles
enum ReplEvent {
    Input(Result<String, ReadlineError>),
}

/// Interactive REPL for the Cadence language
pub struct Repl {
    editor: Option<DefaultEditor>,
    audio_handle: Arc<AudioPlayerHandle>,
    midi_handle: Arc<MidiOutputHandle>,
    clock: Arc<MasterClock>,
    /// Shared BPM as atomic for playback engines
    bpm: Arc<AtomicU64>,
    // Map of track ID to playback engine
    playback_engines: HashMap<usize, Arc<PlaybackEngine>>,
    /// Interpreter for scripting constructs
    interpreter: Interpreter,

    // Event channels
    tx_input: Sender<ReplEvent>,
    rx_input: Receiver<ReplEvent>,
    tx_watcher: Sender<notify::Result<Event>>,
    rx_watcher: Receiver<notify::Result<Event>>,

    // File watcher
    watcher: Option<FileWatcher>,
}

impl Repl {
    /// Create a new REPL instance
    pub fn new() -> RustylineResult<Self> {
        let editor = DefaultEditor::new()?;
        let audio_handle =
            Arc::new(AudioPlayerHandle::new().expect("Failed to create audio player"));

        // Initialize MIDI output (non-fatal if it fails)
        let midi_handle = Arc::new(MidiOutputHandle::new().expect("Failed to create MIDI output"));

        let clock = Arc::new(MasterClock::new(90.0)); // Default 90 BPM
        let bpm = Arc::new(AtomicU64::new(90.0_f32.to_bits() as u64));

        // Initialize with default track 1 (with MIDI support)
        let mut playback_engines = HashMap::new();
        let default_track = 1;
        let tick_rx = clock.subscribe();
        let engine = Arc::new(PlaybackEngine::new_with_midi(
            audio_handle.clone(),
            tick_rx,
            bpm.clone(),
            default_track,
            Some(midi_handle.clone()),
        ));
        playback_engines.insert(default_track, engine);

        let (tx_input, rx_input) = unbounded();
        let (tx_watcher, rx_watcher) = unbounded();

        Ok(Repl {
            editor: Some(editor),
            audio_handle,
            midi_handle,
            clock,
            bpm,
            playback_engines,
            interpreter: Interpreter::new(),
            tx_input,
            rx_input,
            tx_watcher,
            rx_watcher,
            watcher: None,
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

        let tick_rx = self.clock.subscribe();
        let engine = Arc::new(PlaybackEngine::new_with_midi(
            self.audio_handle.clone(),
            tick_rx,
            self.bpm.clone(),
            track_id,
            Some(self.midi_handle.clone()),
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

                // For patterns, set the note duration based on the pattern's step timing
                // Each step gets an equal share of the cycle (default 4 beats)
                if let crate::parser::Value::Pattern(ref pattern) = display_value {
                    let step_beats = pattern.step_beats();
                    if step_beats > 0.0 {
                        config =
                            config.with_duration(crate::audio::clock::Duration::Beats(step_beats));
                    }
                }

                if looping {
                    config = config.with_looping();
                }

                // Ensure the clock is running before starting playback
                self.clock.start();

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
                self.clock.set_bpm(bpm);
                self.bpm
                    .store(bpm.to_bits() as u64, std::sync::atomic::Ordering::Relaxed);
                // Also start the clock if not already running
                self.clock.start();
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

    /// Execute an action but skip looped play expressions if track is already playing.
    /// This is used during file hot-reload for smoother transitions.
    ///
    /// The key insight: reactive expressions are re-evaluated on EVERY beat,
    /// so if you change `let bass = "C2 G1"` to `let bass = "C2 _ C2 G1"`,
    /// the track playing `bass` will automatically pick up the new value
    /// WITHOUT needing to restart the progression!
    fn execute_action_queued(&mut self, action: InterpreterAction, ctx: &mut CommandContext) {
        use crate::parser::Evaluator;

        match action {
            InterpreterAction::PlayExpression {
                expression,
                looping: true, // Only handle looped expressions specially
                queue: _,
                track_id,
            } => {
                let engine = self.get_engine(track_id);

                // KEY FIX: If this track is already playing, SKIP the play command!
                // The reactive expression will automatically pick up variable changes
                // on the next beat. This is what makes hot-reload feel like the REPL.
                if engine.is_playing() {
                    // Just validate that the expression is still valid
                    let shared_env = self.interpreter.shared_environment();
                    let evaluator = Evaluator::new();
                    let display_value = {
                        let env_guard = shared_env.read().unwrap();
                        match evaluator.eval_with_env(expression.clone(), Some(&env_guard)) {
                            Ok(v) => v,
                            Err(e) => {
                                println!(
                                    "{} Expression error on Track {}: {}",
                                    "Error:".red(),
                                    track_id,
                                    e
                                );
                                return;
                            }
                        }
                    };
                    println!(
                        "🔄 Track {} updated: {} (reactive, no restart needed)",
                        track_id, display_value
                    );
                    return;
                }

                // Track is not playing - start it normally
                self.execute_action(
                    InterpreterAction::PlayExpression {
                        expression,
                        looping: true,
                        queue: false, // Immediate play since track isn't running
                        track_id,
                    },
                    ctx,
                );
            }
            // For all other actions, use normal execution
            other => self.execute_action(other, ctx),
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

        // Move editor to thread
        let mut editor = self.editor.take().expect("Repl editor missing");
        let tx_input = self.tx_input.clone();

        thread::spawn(move || {
            loop {
                let prompt = format!("{} ", "cadence>".bright_magenta().bold());
                let readline = editor.readline(&prompt);

                match readline {
                    Ok(line) => {
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            let _ = editor.add_history_entry(&line);
                        }
                        if tx_input.send(ReplEvent::Input(Ok(line))).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = tx_input.send(ReplEvent::Input(Err(err)));
                        break;
                    }
                }
            }
        });

        // Create command registry and context
        // Use track 1 engine for global context for now
        let default_engine = self.get_engine(1);
        let registry = create_registry();
        let mut ctx = CommandContext::new_with_midi(
            self.audio_handle.clone(),
            self.clock.clone(),
            default_engine,
            self.midi_handle.clone(),
        );

        loop {
            crossbeam_channel::select! {
                recv(self.rx_input) -> msg => match msg {
                    Ok(ReplEvent::Input(res)) => {
                        match res {
                            Ok(line) => {
                                if line.is_empty() {
                                    continue;
                                }

                                // Handle REPL-specific commands (needs access to playback_engines)
                                if line == "tracks" {
                                    println!("{}", self.list_tracks());
                                    continue;
                                }

                                // Try to execute as a command
                                match registry.execute(&line, &mut ctx) {
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
                                    CommandResult::Watch(path) => {
                                         // Initialize watcher if needed
                                         if self.watcher.is_none() {
                                            match FileWatcher::new(self.tx_watcher.clone()) {
                                                Ok(w) => self.watcher = Some(w),
                                                Err(e) => println!("{} Failed to create watcher: {}", "Error:".red(), e),
                                            }
                                         }

                                         if let Some(w) = &mut self.watcher {
                                             if let Err(e) = w.watch(&path) {
                                                  println!("{} Failed to watch {}: {}", "Error:".red(), path, e);
                                             } else {
                                                  println!("{} Watching {} for changes...", "eyes".bright_cyan(), path.bright_green());
                                             }
                                         }
                                    }
                                    CommandResult::NotACommand => {
                                        // Parse and execute as statement(s)
                                        match parse_statements(&line) {
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
                    },
                    Err(_) => break, // Channel closed
                },

                recv(self.rx_watcher) -> msg => match msg {
                    Ok(Ok(event)) => {
                        // Only care about modifications or creations
                        // notify 5.0+ events are granular
                        // We generally reload on any write-close or modify
                        use notify::EventKind;
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                for path in event.paths {
                                    println!("{} File changed: {}", "⚡".bright_yellow(), path.display());

                                    // Reload the file content
                                    match std::fs::read_to_string(&path) {
                                        Ok(contents) => {
                                            println!("Reloading...");
                                            match parse_statements(&contents) {
                                                Ok(program) => {
                                                    match self.interpreter.run_program(&program) {
                                                        Ok(_) => println!("{} Reloaded successfully", "✓".bright_green()),
                                                        Err(e) => println!("{} Runtime error: {}", "Error:".red(), e),
                                                    }

                                                    // Execute actions using queued execution for smoother hot-reload
                                                    // Looped patterns will queue instead of immediate restart
                                                    for action in self.interpreter.take_actions() {
                                                        self.execute_action_queued(action, &mut ctx);
                                                    }
                                                },
                                                Err(e) => println!("{} Parse error: {}", "Error:".red(), e),
                                            }
                                        },
                                        Err(e) => println!("{} Failed to read file: {}", "Error:".red(), e),
                                    }
                                }
                            },
                            _ => {}
                        }
                    },
                    Ok(Err(e)) => println!("{} Watch error: {}", "Error:".red(), e),
                    Err(_) => break, // Channel closed
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
