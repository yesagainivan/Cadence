//! REPL (Read-Eval-Print Loop) for the Cadence language

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::clock::MasterClock;
use crate::audio::event_dispatcher::{DispatcherHandle, EventDispatcher, PatternId};
use crate::audio::midi::MidiOutputHandle;
use crate::commands::{create_registry, CommandContext, CommandResult};
use crate::parser::{parse_statements, Interpreter, InterpreterAction, Value};
use crate::repl::watcher::FileWatcher;
use anyhow::Result;
use colored::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use notify::Event;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RustylineResult};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
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
    /// Unified event dispatcher (handles both one-shot and looping playback)
    dispatcher_handle: DispatcherHandle,
    /// Track which pattern IDs are active per track (for stopping)
    active_patterns: HashMap<usize, PatternId>,
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

        let (tx_input, rx_input) = unbounded();
        let (tx_watcher, rx_watcher) = unbounded();

        // Spawn the unified event dispatcher (replaces Scheduler + PlaybackEngines)
        let dispatcher_tick_rx = clock.subscribe();
        let dispatcher_handle = EventDispatcher::spawn(audio_handle.clone(), dispatcher_tick_rx);

        Ok(Repl {
            editor: Some(editor),
            audio_handle,
            midi_handle,
            clock,
            bpm,
            dispatcher_handle,
            active_patterns: HashMap::new(),
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

    /// List all active tracks and their status
    pub fn list_tracks(&self) -> String {
        if self.active_patterns.is_empty() {
            return "No active tracks".to_string();
        }
        let mut track_ids: Vec<_> = self.active_patterns.keys().cloned().collect();
        track_ids.sort();

        let mut output = format!(
            "🎛️  Active Tracks ({}/{}):\n",
            track_ids.len(),
            Self::MAX_TRACKS
        );
        for id in track_ids {
            output.push_str(&format!("  Track {}: ▶ playing\n", id));
        }
        output
    }

    /// Convert a Value to frequencies for one-shot playback
    fn value_to_frequencies(value: &Value) -> Option<(Vec<f32>, Vec<crate::types::DrumSound>)> {
        match value {
            Value::Note(note) => Some((vec![note.frequency()], vec![])),
            Value::Chord(chord) => {
                let freqs: Vec<f32> = chord.notes_vec().iter().map(|n| n.frequency()).collect();
                Some((freqs, vec![]))
            }
            Value::Pattern(pattern) => {
                // For immediate play, get the first event
                let events = pattern.to_rich_events();
                if let Some(first) = events.first() {
                    let freqs: Vec<f32> = first.notes.iter().map(|n| n.frequency).collect();
                    Some((freqs, first.drums.clone()))
                } else {
                    Some((vec![], vec![]))
                }
            }
            Value::String(s) => {
                if let Ok(pattern) = crate::types::Pattern::parse(s) {
                    let events = pattern.to_rich_events();
                    if let Some(first) = events.first() {
                        let freqs: Vec<f32> = first.notes.iter().map(|n| n.frequency).collect();
                        Some((freqs, first.drums.clone()))
                    } else {
                        Some((vec![], vec![]))
                    }
                } else {
                    None
                }
            }
            Value::EveryPattern(every) => {
                // For immediate play, use base pattern's first event
                let events = every.base.to_rich_events();
                if let Some(first) = events.first() {
                    let freqs: Vec<f32> = first.notes.iter().map(|n| n.frequency).collect();
                    Some((freqs, first.drums.clone()))
                } else {
                    Some((vec![], vec![]))
                }
            }
            _ => None,
        }
    }

    /// Execute an interpreter action (triggers actual audio/state changes)
    fn execute_action(&mut self, action: InterpreterAction, _ctx: &mut CommandContext) {
        match action {
            InterpreterAction::PlayExpression {
                expression,
                looping,
                queue_mode: _,
                track_id,
                display_value,
                scheduled_beat: _,
            } => {
                // Ensure the clock is running before starting playback
                self.clock.start();

                // Extract envelope and waveform from the pattern if present
                let pattern_props: Option<(
                    Option<(f32, f32, f32, f32)>,
                    Option<crate::types::Waveform>,
                )> = match &display_value {
                    Value::Pattern(pattern) => Some((pattern.envelope, pattern.waveform)),
                    Value::EveryPattern(every) => Some((every.base.envelope, every.base.waveform)),
                    _ => None,
                };

                if let Some((envelope, waveform)) = pattern_props {
                    if let Some(env) = envelope {
                        self.dispatcher_handle
                            .set_track_envelope(track_id, Some(env));
                    }
                    if let Some(wf) = waveform {
                        self.dispatcher_handle.set_track_waveform(track_id, wf);
                    }
                }

                if looping {
                    // For looping plays, start a loop in the dispatcher
                    let shared_env = self.interpreter.shared_environment();
                    let pattern_id = self
                        .dispatcher_handle
                        .start_loop(expression, shared_env, track_id);
                    self.active_patterns.insert(track_id, pattern_id);
                    println!(
                        "🔊 Playing {} (Track {}) - live reactive!",
                        display_value, track_id
                    );
                } else {
                    // For one-shot plays, trigger immediately
                    if let Some((freqs, drums)) = Self::value_to_frequencies(&display_value) {
                        self.dispatcher_handle
                            .trigger_immediate(track_id, freqs, drums);
                    } else {
                        println!("{} Cannot play this value", "Playback error:".red());
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
                self.dispatcher_handle.set_track_volume(track_id, volume);
            }
            InterpreterAction::SetWaveform { waveform, track_id } => {
                // Parse waveform name and set it on the audio handle
                use crate::types::Waveform;
                if let Some(wf) = Waveform::from_str(&waveform) {
                    self.dispatcher_handle.set_track_waveform(track_id, wf);
                } else {
                    println!(
                        "{} Unknown waveform: {} (Track {})",
                        "Waveform error:".red(),
                        waveform,
                        track_id
                    );
                }
            }
            InterpreterAction::Stop { track_id } => {
                match track_id {
                    Some(id) => {
                        self.dispatcher_handle.stop_track(id);
                        self.active_patterns.remove(&id);
                    }
                    None => {
                        // Stop all playback
                        self.dispatcher_handle.stop_all();
                        self.active_patterns.clear();
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
        match action {
            InterpreterAction::PlayExpression {
                expression,
                looping: true, // Only handle looped expressions specially
                queue_mode: _,
                track_id,
                display_value,
                scheduled_beat,
            } => {
                // KEY FIX: If this track is already playing, SKIP the play command!
                // The reactive expression will automatically pick up variable changes
                // on the next beat. This is what makes hot-reload feel like the REPL.
                if self.active_patterns.contains_key(&track_id) {
                    // Use the pre-evaluated display_value from when the action was created
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
                        queue_mode: None, // Immediate play since track isn't running
                        track_id,
                        display_value,
                        scheduled_beat,
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

        thread::spawn(move || loop {
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
        });

        // Create command registry and context
        let registry = create_registry();
        let mut ctx = CommandContext::new_with_midi(
            self.audio_handle.clone(),
            self.clock.clone(),
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

                                                // Execute collected actions (immediate plays)
                                                for action in self.interpreter.take_actions() {
                                                    self.execute_action(action, &mut ctx);
                                                }

                                                // Send scheduled events to the dispatcher
                                                let scheduled_events = self.interpreter.take_scheduled_events();
                                                if !scheduled_events.is_empty() {
                                                    // Get current beat for scheduling relative to now
                                                    let base_beat = self.clock.current_beat();
                                                    self.dispatcher_handle.schedule(scheduled_events, base_beat);
                                                    // Start the clock if not already running
                                                    self.clock.start();
                                                }

                                                // Reset virtual time for next interaction
                                                self.interpreter.reset_virtual_time();
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
