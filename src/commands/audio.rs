//! Audio-related commands

use crate::audio::playback_engine::ProgressionConfig;
use crate::audio::scheduler::Duration;
use crate::commands::{CommandContext, CommandResult};
use crate::parser::Value;
use colored::*;

/// Handle `audio play progression <expr> [loop] [queue] [duration <n>]`
pub fn cmd_audio_play_progression(args: &str, ctx: &mut CommandContext) -> CommandResult {
    let mut rest = args;

    // Parse modifiers in any order from the end
    let mut loop_enabled = false;
    let mut queue_enabled = false;
    let mut duration_beats = 1.0;

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
        let duration_str = &rest[duration_pos + 10..];
        if let Ok(dur) = duration_str.trim().parse::<f32>() {
            duration_beats = dur;
            rest = &rest[..duration_pos];
        }
    }

    let expr_str = rest.trim();

    if expr_str.is_empty() {
        return CommandResult::Error(
            "No expression provided after 'audio play progression'".to_string(),
        );
    }

    match ctx.eval(expr_str) {
        Ok(Value::Progression(prog)) => {
            // Convert progression to frequencies
            let frequencies_vec: Vec<Vec<f32>> = prog
                .chords()
                .map(|chord| chord.notes().map(|n| n.frequency()).collect())
                .collect();

            // Create progression config
            let mut config = ProgressionConfig::new(frequencies_vec)
                .with_duration(Duration::Beats(duration_beats));

            if loop_enabled {
                config = config.with_looping();
            }

            let chord_count = prog.chords().count();

            // Start playback
            let result = if queue_enabled {
                ctx.playback_engine.queue_progression(config)
            } else {
                ctx.playback_engine.play_progression(config)
            };

            match result {
                Ok(_) => {
                    let msg = if loop_enabled && queue_enabled {
                        "🔁 Queued looping progression for next beat... (use 'audio stop' to stop)"
                            .bright_green()
                            .to_string()
                    } else if loop_enabled {
                        "🔁 Looping progression... (use 'audio stop' to stop)"
                            .bright_green()
                            .to_string()
                    } else if queue_enabled {
                        format!(
                            "🎵 Queued progression ({} chords) for next beat...",
                            chord_count
                        )
                        .bright_green()
                        .to_string()
                    } else {
                        format!(
                            "🎵 Playing progression ({} chords, {:.1} BPM)...",
                            chord_count,
                            ctx.clock.get_bpm()
                        )
                        .bright_green()
                        .to_string()
                    };
                    CommandResult::Message(msg)
                }
                Err(e) => {
                    CommandResult::Error(format!("Failed to start progression playback: {}", e))
                }
            }
        }
        Ok(_) => CommandResult::Error("Expression is not a progression".to_string()),
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
                if let Err(e) = ctx.audio_handle.set_notes(frequencies) {
                    return CommandResult::Error(format!("Failed to set notes: {}", e));
                }

                if let Err(e) = ctx.audio_handle.play() {
                    return CommandResult::Error(format!("Failed to start playback: {}", e));
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
    let _ = ctx.playback_engine.stop();
    if let Err(e) = ctx.audio_handle.pause() {
        CommandResult::Error(format!("Failed to stop audio playback: {}", e))
    } else {
        CommandResult::Message("🔇 Audio playback stopped.".bright_green().to_string())
    }
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
        Value::Progression(_) => {
            return Err(anyhow::anyhow!(
                "Use 'audio play progression' for progressions"
            ));
        }
        Value::Boolean(_) => {
            return Err(anyhow::anyhow!("Cannot play a boolean value"));
        }
        Value::Pattern(_) => {
            return Err(anyhow::anyhow!("Use 'play' for patterns"));
        }
    }
    Ok(frequencies)
}
