use crate::audio::audio::AudioPlayerHandle;
use crate::audio::playback_engine::{PlaybackEngine, ProgressionConfig};
use crate::audio::scheduler::{Duration, Scheduler};
use crate::parser::{Value, eval};
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
        let scheduler = Arc::new(Scheduler::new(90.0)); // Default 120 BPM
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

        loop {
            let prompt = format!("{} ", "cadence>".bright_magenta().bold());
            match self.editor.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    self.editor.add_history_entry(line.to_owned())?;

                    // Check for commands
                    // Check for progression playback FIRST (more specific)
                    if line.starts_with("audio play progression") {
                        let mut rest = line.trim_start_matches("audio play progression").trim();

                        // Parse for 'loop', 'queue', or 'duration <n>' modifiers from the end
                        // These can appear in any order, so we loop until no more are found
                        let mut loop_enabled = false;
                        let mut queue_enabled = false;
                        let mut duration_beats = 1.0;

                        // Parse modifiers in any order
                        loop {
                            if rest.ends_with(" loop") {
                                loop_enabled = true;
                                rest = rest.trim_end_matches(" loop").trim();
                            } else if rest.ends_with(" queue") {
                                queue_enabled = true;
                                rest = rest.trim_end_matches(" queue").trim();
                            } else {
                                break;
                            }
                        }

                        // Check for 'duration <n>' at the end
                        if let Some(duration_pos) = rest.rfind(" duration ") {
                            let duration_str = &rest[duration_pos + 10..]; // Skip " duration "
                            if let Ok(dur) = duration_str.trim().parse::<f32>() {
                                duration_beats = dur;
                                rest = &rest[..duration_pos];
                            }
                        }

                        let expr_str = rest;

                        if expr_str.is_empty() {
                            println!(
                                "{}",
                                "Error: No expression provided after 'audio play progression'"
                                    .bright_red()
                            );
                            continue;
                        }

                        match self.evaluate_expression(&expr_str) {
                            Ok(value) => {
                                if let Value::Progression(prog) = value {
                                    // Convert progression to frequencies
                                    let mut frequencies_vec = Vec::new();
                                    for chord in prog.chords() {
                                        let freqs: Vec<f32> =
                                            chord.notes().map(|n| n.frequency()).collect();
                                        frequencies_vec.push(freqs);
                                    }

                                    // Create progression config
                                    let mut config = ProgressionConfig::new(frequencies_vec)
                                        .with_duration(Duration::Beats(duration_beats));

                                    if loop_enabled {
                                        config = config.with_looping();
                                    }

                                    let chord_count = prog.chords().count();

                                    // Start playback (queue for beat-synced, immediate otherwise)
                                    let result = if queue_enabled {
                                        self.playback_engine.queue_progression(config)
                                    } else {
                                        self.playback_engine.play_progression(config)
                                    };

                                    match result {
                                        Ok(_) => {
                                            if loop_enabled && queue_enabled {
                                                println!("{}", "🔁 Queued looping progression for next beat... (use 'audio stop' to stop)".bright_green());
                                            } else if loop_enabled {
                                                println!("{}", "🔁 Looping progression... (use 'audio stop' to stop)".bright_green());
                                            } else if queue_enabled {
                                                println!("{}", format!("🎵 Queued progression ({} chords) for next beat...", chord_count).bright_green());
                                            } else {
                                                println!("{}", format!("🎵 Playing progression ({} chords, {:.1} BPM)...", chord_count, self.scheduler.get_bpm()).bright_green());
                                            }
                                        }
                                        Err(e) => {
                                            println!("{}", format!("Error: Failed to start progression playback: {}", e).bright_red());
                                        }
                                    }
                                } else {
                                    println!(
                                        "{}",
                                        "Error: Expression is not a progression".bright_red()
                                    );
                                }
                            }
                            Err(e) => {
                                println!("{}", format!("Error: {}", e).bright_red());
                            }
                        }
                    // Now check for generic audio play (less specific)
                    } else if line.starts_with("audio play") {
                        let expr_str = line.trim_start_matches("audio play").trim();
                        if expr_str.is_empty() {
                            println!(
                                "{}",
                                "Error: No expression provided after 'audio play'".bright_red()
                            );
                            continue;
                        }

                        match self.evaluate_expression(expr_str) {
                            Ok(value) => {
                                // Get frequencies from the value
                                match get_frequencies_from_value(&value) {
                                    Ok(frequencies) => {
                                        // Set notes BEFORE starting playback
                                        if let Err(e) = self.audio_handle.set_notes(frequencies) {
                                            println!(
                                                "{}",
                                                format!("Error: Failed to set notes: {}", e)
                                                    .bright_red()
                                            );
                                            continue;
                                        }

                                        // Now start playback
                                        if let Err(e) = self.audio_handle.play() {
                                            println!(
                                                "{}",
                                                format!("Error: Failed to start playback: {}", e)
                                                    .bright_red()
                                            );
                                        } else {
                                            println!(
                                                "{}",
                                                "🔊 Audio playback started.".bright_green()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        println!("{}", format!("Error: {}", e).bright_red());
                                    }
                                }
                            }
                            Err(e) => {
                                println!("{}", format!("Error: {}", e).bright_red());
                            }
                        }
                    } else if line == "audio stop" {
                        // Stop both progression playback and direct audio
                        let _ = self.playback_engine.stop();
                        if let Err(e) = self.audio_handle.pause() {
                            println!(
                                "{}",
                                format!("Error: Failed to stop audio playback: {}", e).bright_red()
                            );
                        } else {
                            println!("{}", "🔇 Audio playback stopped.".bright_green());
                        }
                    } else if line.starts_with("audio volume") {
                        let volume_str = line.trim_start_matches("audio volume").trim();
                        if volume_str.is_empty() {
                            // Show current volume - not yet supported by AudioPlayerHandle
                            println!("{}", "Volume control: use 'audio volume <0-100>'");
                        } else {
                            // Set volume
                            match volume_str.parse::<f32>() {
                                Ok(vol) => {
                                    let normalized_vol = if vol > 1.0 { vol / 100.0 } else { vol };
                                    if let Err(e) = self.audio_handle.set_volume(normalized_vol) {
                                        println!("{}", format!("Error: {}", e).bright_red());
                                    } else {
                                        println!(
                                            "{}",
                                            format!(
                                                "🔊 Volume set to {:.0}%",
                                                normalized_vol * 100.0
                                            )
                                            .bright_green()
                                        );
                                    }
                                }
                                Err(_) => {
                                    println!("{}", "Error: Invalid volume value. Use a number between 0-100 or 0.0-1.0".bright_red());
                                }
                            }
                        }
                    } else if line.starts_with("tempo") {
                        let tempo_str = line.trim_start_matches("tempo").trim();
                        if tempo_str.is_empty() {
                            // Show current tempo
                            println!("Current tempo: {:.1} BPM", self.scheduler.get_bpm());
                        } else {
                            // Set tempo
                            match tempo_str.parse::<f32>() {
                                Ok(bpm) if bpm > 0.0 && bpm <= 400.0 => {
                                    self.scheduler.set_bpm(bpm);
                                    println!(
                                        "{}",
                                        format!("🎵 Tempo set to {:.1} BPM", bpm).bright_green()
                                    );
                                }
                                _ => {
                                    println!(
                                        "{}",
                                        "Error: Invalid tempo. Use a value between 1-400 BPM"
                                            .bright_red()
                                    );
                                }
                            }
                        }
                    } else if line == "help" {
                        self.print_help();
                        continue;
                    } else if line == "quit" || line == "exit" {
                        break;
                    } else {
                        // Evaluate the expression
                        match self.evaluate_expression(line) {
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

    /// Evaluate a single expression
    fn evaluate_expression(&self, input: &str) -> Result<Value> {
        eval(input)
    }

    /// Print help information
    fn print_help(&self) {
        println!("{}", "🎵 Cadence Language Help".bold());
        println!("{}", "=======================".bold());
        println!();
        println!("{}", "Basic Usage:".green());
        println!("  {}              - Single note", "C".cyan());
        println!("  {}      - Chord literal", "[C, E, G]".cyan());
        println!(
            "  {} + 2  - Transpose chord up 2 semitones",
            "[C, E, G]".cyan()
        );
        println!(
            "  {} - 5  - Transpose chord down 5 semitones",
            "[C, E, G]".cyan()
        );
        println!();
        println!("{}", "Progressions:".green());
        println!(
            "  {}      - Progression literal",
            "[[C, E, G], [F, A, C]]".cyan()
        );
        println!(
            "  {} + 2  - Transpose entire progression",
            "[[C, E, G], [F, A, C]]".cyan()
        );
        println!();
        println!("{}", "Set Operations:".green());
        println!(
            "  {} & {}  - Intersection (common tones)",
            "[C, E, G]".cyan(),
            "[A, C, E]".cyan()
        );
        println!(
            "  {} | {}  - Union (all notes)",
            "[C, E, G]".cyan(),
            "[A, C, E]".cyan()
        );
        println!(
            "  {} ^ {}  - Difference (non-common tones)",
            "[C, E, G]".cyan(),
            "[A, C, E]".cyan()
        );
        println!();
        println!("{}", "Functions:".green());
        println!("  {}         - First inversion", "invert([C, E, G])".cyan());
        println!(
            "  {}    - Nth inversion (D=2, so 2nd inversion)",
            "invert_n([C, E, G], D)".cyan()
        );
        println!("  {}           - Get root note", "root([C, E, G])".cyan());
        println!("  {}           - Get bass note", "bass([C, E, G])".cyan());
        println!();
        println!("{}", "Progression Functions:".green());
        println!(
            "  {}  - Reverse chord order",
            "retrograde([[C, E, G], [F, A, C]])".cyan()
        );
        println!(
            "  {} - Apply function to all chords",
            "map(invert, [[C, E, G], [F, A, C]])".cyan()
        );
        println!();
        println!("{}", "Examples:".green());
        println!("  cadence> {}", "[C, E, G]".cyan());
        println!("  C Major: [C, E, G]");
        println!();
        println!("  cadence> {}", "[C, E, G] + 7".cyan());
        println!("  G Major: [G, B, D]");
        println!();
        println!("  cadence> {}", "[[C, E, G], [F, A, C]]".cyan());
        println!("  [C Major: [C, E, G], F Major: [F, A, C]]");
        println!();
        println!("  cadence> {}", "retrograde([[C, E, G], [F, A, C]])".cyan());
        println!("  [F Major: [F, A, C], C Major: [C, E, G]]");
        println!();
        println!("  cadence> {}", "invert([C, E, G])".cyan());
        println!("  C Major/E (1st inv): [C, E, G]");
        println!();
        println!("  cadence> {}", "[C, E, G] & [A, C, E]".cyan());
        println!("  [C, E]");
        println!();
        println!("{}", "Voice Leading Analysis:".green());
        println!(
            "  {}  - Analyze movement between two chords",
            "voice_leading([C, E, G], [F, A, C])".cyan()
        );
        println!(
            "  {}         - Find common tones",
            "common_tones([C, E, G], [F, A, C])".cyan()
        );
        println!(
            "  {}     - Optimize all voice leading",
            "smooth_voice_leading([[C, E, G], [F, A, C]])".cyan()
        );
        println!(
            "  {}  - Detailed analysis of progression",
            "analyze_voice_leading([[C, E, G], [F, A, C]])".cyan()
        );
        println!(
            "  {}  - Get quality score",
            "voice_leading_quality([[C, E, G], [F, A, C]])".cyan()
        );
        //
        println!();
        println!("{}", "Roman Numeral Analysis:".green());
        println!(
            "  {}           - Analyze chord in key",
            "roman_numeral([C, E, G], C)".cyan()
        );
        println!(
            "  {}                     - Short form",
            "rn([F, A, C], C)".cyan()
        );
        println!(
            "  {}  - Analyze progression",
            "analyze_progression([[C, E, G], [F, A, C]], C)".cyan()
        );
        println!();

        println!("{}", "Progressions:".green());
        println!(
            "  {}                 - List all available",
            "list_progressions()".cyan()
        );
        println!(
            "  {}            - Direct progression call",
            "I_V_vi_IV(C)".cyan()
        );
        println!(
            "  {}       - Function call (flexible)",
            "progression(I-V-vi-IV, C)".cyan()
        );
        println!("  {}              - Jazz turnaround", "ii_V_I(C)".cyan());
        println!("  {}           - Canon progression", "Pachelbel(D)".cyan());
        println!("  {}         - 12-bar blues", "12_bar_blues(E)".cyan());
        println!();

        println!("{}", "Advanced Examples:".green());
        println!("  cadence> {}", "I_V_vi_IV(C)".cyan());
        println!("  Generated I-V-vi-IV progression in C");
        println!(
            "  [C Major: [C, E, G], G Major: [G, B, D], A minor: [A, C, E], F Major: [F, A, C]]"
        );
        println!();
        println!("  cadence> {}", "rn([F#, A, C], C)".cyan());
        println!("  #iv° in C major (Tritone substitution - creates strong pull to V)");
        println!("  F# diminished: [F#, A, C]");
        println!();
        println!("  cadence> {}", "analyze_progression(ii_V_I(C), C)".cyan());
        println!("  Roman Numeral Analysis in C major:");
        println!("    1: ii (Supertonic - predominant function, leads to V)");
        println!("    2: V (Dominant - tension, leads strongly to I)");
        println!("    3: I (Tonic - home, stability, resolution)");

        println!();
        println!("{}", "Audio Commands:".green());
        println!("  {}  - Play a note or chord", "audio play <expr>".cyan());
        println!("  {}      - Stop current playback", "audio stop".cyan());
        println!(
            "  {} - Set volume (0-100 or 0.0-1.0)",
            "audio volume <level>".cyan()
        );
        println!("  {}     - Show current volume", "audio volume".cyan());
        println!();
        println!("{}", "Other Commands:".green());
        println!("  {}         - Show this help", "help".bright_green());
        println!("  {}         - Exit the REPL", "quit".bright_red());
    }
}

fn get_frequencies_from_value(value: &Value) -> Result<Vec<f32>> {
    let mut frequencies = Vec::new();
    match value {
        Value::Note(note) => {
            frequencies.push(note.frequency());
        }
        Value::Chord(chord) => {
            for note in chord.notes() {
                frequencies.push(note.frequency());
            }
        }
        Value::Progression(_) => {
            return Err(anyhow::anyhow!(
                "Progressions are not yet supported for audio playback. Try playing individual chords instead."
            ));
        }
    }
    Ok(frequencies)
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
        let repl = Repl::new().unwrap();

        // Test basic note evaluation
        let result = repl.evaluate_expression("C");
        assert!(result.is_ok());

        // Test chord evaluation
        let result = repl.evaluate_expression("[C, E, G]");
        assert!(result.is_ok());

        // Test arithmetic
        let result = repl.evaluate_expression("[C, E, G] + 2");
        assert!(result.is_ok());

        // Test function call
        let result = repl.evaluate_expression("invert([C, E, G])");
        assert!(result.is_ok());

        // Test error handling
        let result = repl.evaluate_expression("invalid syntax @#$");
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_set_operations() {
        let repl = Repl::new().unwrap();

        // Test intersection
        let result = repl.evaluate_expression("[C, E, G] & [A, C, E]");
        assert!(result.is_ok());

        // Test union
        let result = repl.evaluate_expression("[C, E, G] | [F, A, C]");
        assert!(result.is_ok());

        // Test difference
        let result = repl.evaluate_expression("[C, E, G] ^ [A, C, E]");
        assert!(result.is_ok());
    }

    #[test]
    fn test_evaluate_complex_expressions() {
        let repl = Repl::new().unwrap();

        // Test nested operations
        let result = repl.evaluate_expression("invert([C, E, G] + 2)");
        assert!(result.is_ok());

        // Test function composition
        let result = repl.evaluate_expression("bass(invert([C, E, G]))");
        assert!(result.is_ok());

        // Test complex set operations
        let result = repl.evaluate_expression("[C, E, G] + 2 & [A, C, E]");
        assert!(result.is_ok());
    }
}
