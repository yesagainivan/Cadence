//! Audio-related commands

use crate::commands::{CommandContext, CommandResult};
use crate::parser::Value;
use colored::*;

/// Handle `audio play progression <expr>` - simplified for new dispatcher architecture
/// Now plays each chord immediately using trigger_note
pub fn cmd_audio_play_progression(args: &str, ctx: &mut CommandContext) -> CommandResult {
    let expr_str = args.trim();

    if expr_str.is_empty() {
        return CommandResult::Error(
            "No expression provided after 'audio play progression'".to_string(),
        );
    }

    match ctx.eval(expr_str) {
        Ok(Value::Pattern(prog)) => {
            // Convert pattern to frequencies via as_chords()
            let chords = match prog.as_chords() {
                Some(c) => c,
                None => {
                    return CommandResult::Error(
                        "Pattern contains rests or groups - cannot convert to progression"
                            .to_string(),
                    );
                }
            };

            let chord_count = chords.len();

            // For immediate feedback, play the first chord
            if let Some(first_chord) = chords.first() {
                let frequencies: Vec<f32> = first_chord.notes().map(|n| n.frequency()).collect();
                if !frequencies.is_empty() {
                    if let Err(e) = ctx.audio_handle.trigger_note(1, frequencies) {
                        return CommandResult::Error(format!("Failed to play: {}", e));
                    }
                }
            }

            CommandResult::Message(
                format!(
                    "🎵 Triggered first chord of progression ({} chords). Use 'play X loop' for continuous playback.",
                    chord_count
                )
                .bright_green()
                .to_string(),
            )
        }
        Ok(_) => CommandResult::Error("Expression is not a pattern/progression".to_string()),
        Err(e) => CommandResult::Error(e.to_string()),
    }
}

/// Handle `audio play <expr>` - play a note or chord
pub fn cmd_audio_play(args: &str, ctx: &mut CommandContext) -> CommandResult {
    if args.is_empty() {
        return CommandResult::Error("No expression provided after 'audio play'".to_string());
    }

    match ctx.eval(args) {
        Ok(value) => match get_frequencies_from_value(&value) {
            Ok(frequencies) => {
                // Use trigger_note for proper envelope attack
                if let Err(e) = ctx.audio_handle.trigger_note(1, frequencies) {
                    return CommandResult::Error(format!("Failed to play: {}", e));
                }

                CommandResult::Message("🔊 Audio playback started.".bright_green().to_string())
            }
            Err(e) => CommandResult::Error(e.to_string()),
        },
        Err(e) => CommandResult::Error(e.to_string()),
    }
}

/// Handle `audio stop`
pub fn cmd_audio_stop(_args: &str, ctx: &mut CommandContext) -> CommandResult {
    // Clear notes on default track
    let _ = ctx.audio_handle.set_track_notes(1, vec![]);
    CommandResult::Message("🔇 Audio playback stopped.".bright_green().to_string())
}

/// Handle `audio volume [level]`
pub fn cmd_audio_volume(args: &str, ctx: &mut CommandContext) -> CommandResult {
    if args.is_empty() {
        return CommandResult::Message("Volume control: use 'audio volume <0-100>'".to_string());
    }

    match args.parse::<f32>() {
        Ok(vol) => {
            let normalized_vol = if vol > 1.0 { vol / 100.0 } else { vol };
            if let Err(e) = ctx.audio_handle.set_volume(normalized_vol) {
                CommandResult::Error(e.to_string())
            } else {
                CommandResult::Message(
                    format!("🔊 Volume set to {:.0}%", normalized_vol * 100.0)
                        .bright_green()
                        .to_string(),
                )
            }
        }
        Err(_) => CommandResult::Error(
            "Invalid volume value. Use a number between 0-100 or 0.0-1.0".to_string(),
        ),
    }
}

/// Extract frequencies from a Value (Note or Chord)
fn get_frequencies_from_value(value: &Value) -> anyhow::Result<Vec<f32>> {
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
        Value::Pattern(_) => {
            return Err(anyhow::anyhow!(
                "Cannot play a pattern directly as frequencies"
            ));
        }
        Value::Boolean(_) => return Err(anyhow::anyhow!("Cannot play a boolean")),
        Value::Number(_) => return Err(anyhow::anyhow!("Cannot play a number")),
        Value::String(_) => return Err(anyhow::anyhow!("Cannot play a string")),
        Value::Function { name, .. } => {
            return Err(anyhow::anyhow!(
                "Cannot play a function '{}' - call it first",
                name
            ));
        }
        Value::Unit => return Err(anyhow::anyhow!("Cannot play unit (void)")),
        Value::Array(_) => return Err(anyhow::anyhow!("Cannot play an array directly")),
        Value::EveryPattern(_) => {
            return Err(anyhow::anyhow!(
                "Cannot play an EveryPattern directly - use 'play X loop' for cycle-based alternation"
            ));
        }
    }

    Ok(frequencies)
}
