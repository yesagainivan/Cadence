//! MIDI REPL commands

use crate::audio::midi::{MidiChannelMode, OutputMode};
use crate::commands::{CommandContext, CommandResult};
use colored::*;

/// Handle `midi devices` command - list available MIDI output ports
pub fn cmd_midi_devices(_args: &str, ctx: &mut CommandContext) -> CommandResult {
    match &ctx.midi_handle {
        Some(handle) => match handle.list_ports() {
            Ok(ports) => {
                if ports.is_empty() {
                    CommandResult::Message(
                        "No MIDI output ports found. Make sure a MIDI device or virtual port is connected."
                            .yellow()
                            .to_string(),
                    )
                } else {
                    let mut output = format!("{}\n", "🎹 Available MIDI Output Ports:".bold());
                    for (i, port) in ports.iter().enumerate() {
                        output.push_str(&format!("  {}. {}\n", i + 1, port.cyan()));
                    }
                    output.push_str(&format!(
                        "\n{} {}",
                        "Use".dimmed(),
                        "midi connect <port name>".green()
                    ));
                    CommandResult::Message(output)
                }
            }
            Err(e) => CommandResult::Error(format!("Failed to list MIDI ports: {}", e)),
        },
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi connect <port>` command - connect to a MIDI output port
pub fn cmd_midi_connect(args: &str, ctx: &mut CommandContext) -> CommandResult {
    if args.is_empty() {
        return CommandResult::Error(
            "Usage: midi connect <port name>\nUse 'midi devices' to see available ports"
                .to_string(),
        );
    }

    match &ctx.midi_handle {
        Some(handle) => match handle.connect(args) {
            Ok(()) => {
                CommandResult::Message(format!("🎹 Connected to MIDI port: {}", args.green()))
            }
            Err(e) => CommandResult::Error(format!("Failed to connect to '{}': {}", args, e)),
        },
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi disconnect` command
pub fn cmd_midi_disconnect(_args: &str, ctx: &mut CommandContext) -> CommandResult {
    match &ctx.midi_handle {
        Some(handle) => match handle.disconnect() {
            Ok(()) => CommandResult::Message("🎹 Disconnected from MIDI".to_string()),
            Err(e) => CommandResult::Error(format!("Failed to disconnect: {}", e)),
        },
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi channel <n>` command - set channel mode
pub fn cmd_midi_channel(args: &str, ctx: &mut CommandContext) -> CommandResult {
    if args.is_empty() {
        // Show current channel mode
        match &ctx.midi_handle {
            Some(handle) => {
                let mode_desc = match handle.channel_mode() {
                    MidiChannelMode::PerTrack => {
                        "Per-track (Track 1→Ch 1, Track 2→Ch 2, etc.)".to_string()
                    }
                    MidiChannelMode::Mono(ch) => format!("Mono (all tracks→Channel {})", ch + 1),
                };
                return CommandResult::Message(format!(
                    "🎹 Current MIDI channel mode: {}",
                    mode_desc
                ));
            }
            None => return CommandResult::Error("MIDI output not initialized".to_string()),
        }
    }

    // Parse channel argument
    let channel_arg = args.to_lowercase();

    match &ctx.midi_handle {
        Some(handle) => {
            if channel_arg == "auto" || channel_arg == "per-track" || channel_arg == "pertrack" {
                handle.set_channel_mode(MidiChannelMode::PerTrack);
                CommandResult::Message(
                    "🎹 MIDI channel mode: Per-track (Track 1→Ch 1, Track 2→Ch 2, etc.)"
                        .green()
                        .to_string(),
                )
            } else if let Ok(ch) = channel_arg.parse::<u8>() {
                if ch >= 1 && ch <= 16 {
                    handle.set_channel_mode(MidiChannelMode::Mono(ch - 1)); // Convert to 0-indexed
                    CommandResult::Message(
                        format!("🎹 MIDI channel mode: Mono (all tracks→Channel {})", ch)
                            .green()
                            .to_string(),
                    )
                } else {
                    CommandResult::Error(
                        "Channel must be 1-16, or 'auto' for per-track mode".to_string(),
                    )
                }
            } else {
                CommandResult::Error(
                    "Usage: midi channel <1-16|auto>\n  1-16: Send all tracks to this channel\n  auto: Each track uses its own channel"
                        .to_string(),
                )
            }
        }
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi status` command - show MIDI connection status
pub fn cmd_midi_status(_args: &str, ctx: &mut CommandContext) -> CommandResult {
    match &ctx.midi_handle {
        Some(handle) => {
            let connected = handle.is_connected();
            let port_name = handle.connected_port();
            let mode = handle.channel_mode();

            let mut output = format!("{}\n", "🎹 MIDI Status:".bold());

            if connected {
                output.push_str(&format!("  Status: {}\n", "Connected".green().bold()));
                if let Some(name) = port_name {
                    output.push_str(&format!("  Port: {}\n", name.cyan()));
                }
            } else {
                output.push_str(&format!("  Status: {}\n", "Not connected".yellow()));
            }

            let mode_desc = match mode {
                MidiChannelMode::PerTrack => "Per-track (Track N → Channel N)".to_string(),
                MidiChannelMode::Mono(ch) => format!("Mono (all → Channel {})", ch + 1),
            };
            output.push_str(&format!("  Channel mode: {}\n", mode_desc));

            CommandResult::Message(output)
        }
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi panic` command - send All Notes Off to all channels
pub fn cmd_midi_panic(_args: &str, ctx: &mut CommandContext) -> CommandResult {
    match &ctx.midi_handle {
        Some(handle) => match handle.panic_all() {
            Ok(()) => CommandResult::Message(
                "🎹 MIDI Panic: All Notes Off sent to all channels"
                    .yellow()
                    .to_string(),
            ),
            Err(e) => CommandResult::Error(format!("Failed to send MIDI panic: {}", e)),
        },
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi cc <controller> <value> [channel]` command - send Control Change message
/// controller: 0-127 (standard MIDI CC numbers)
/// value: 0-127
/// channel: 1-16 (optional, defaults to 1)
pub fn cmd_midi_cc(args: &str, ctx: &mut CommandContext) -> CommandResult {
    let parts: Vec<&str> = args.split_whitespace().collect();

    if parts.len() < 2 {
        return CommandResult::Error(
            "Usage: midi cc <controller> <value> [channel]\n  controller: 0-127 (CC number)\n  value: 0-127\n  channel: 1-16 (default: 1)\n\nCommon CC numbers:\n  1 = Mod Wheel, 7 = Volume, 10 = Pan, 64 = Sustain".to_string(),
        );
    }

    let controller: u8 = match parts[0].parse() {
        Ok(c) if c <= 127 => c,
        _ => return CommandResult::Error("Controller must be 0-127".to_string()),
    };

    let value: u8 = match parts[1].parse() {
        Ok(v) if v <= 127 => v,
        _ => return CommandResult::Error("Value must be 0-127".to_string()),
    };

    let channel: u8 = if parts.len() >= 3 {
        match parts[2].parse::<u8>() {
            Ok(ch) if ch >= 1 && ch <= 16 => ch - 1, // Convert to 0-indexed
            _ => return CommandResult::Error("Channel must be 1-16".to_string()),
        }
    } else {
        0 // Default to channel 1 (0-indexed)
    };

    match &ctx.midi_handle {
        Some(handle) => {
            if !handle.is_connected() {
                return CommandResult::Error(
                    "Not connected to MIDI. Use 'midi connect <port>' first.".to_string(),
                );
            }

            match handle.cc_on_channel(channel, controller, value) {
                Ok(()) => CommandResult::Message(format!(
                    "🎹 Sent CC: controller={}, value={}, channel={}",
                    controller.to_string().cyan(),
                    value.to_string().green(),
                    (channel + 1).to_string().yellow()
                )),
                Err(e) => CommandResult::Error(format!("Failed to send CC: {}", e)),
            }
        }
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `midi test [note]` command - send a test note to verify MIDI output
/// Default note is 60 (C4), or specify a MIDI note number 0-127
pub fn cmd_midi_test(args: &str, ctx: &mut CommandContext) -> CommandResult {
    let note: u8 = if args.is_empty() {
        60 // C4 = middle C
    } else {
        match args.parse::<u8>() {
            Ok(n) if n <= 127 => n,
            _ => return CommandResult::Error("Note must be 0-127 (e.g., 60 = C4)".to_string()),
        }
    };

    match &ctx.midi_handle {
        Some(handle) => {
            if !handle.is_connected() {
                return CommandResult::Error(
                    "Not connected to MIDI. Use 'midi connect <port>' first.".to_string(),
                );
            }

            // Send Note On
            if let Err(e) = handle.note_on(0, note, 100) {
                return CommandResult::Error(format!("Failed to send note on: {}", e));
            }

            // Note name for display
            let note_names = [
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ];
            let note_name = note_names[(note % 12) as usize];
            let octave = (note / 12) as i8 - 1;

            println!(
                "🎹 Sending MIDI Note On: {} (note {}, channel 1, velocity 100)",
                format!("{}{}", note_name, octave).cyan(),
                note
            );

            // Wait 500ms then send Note Off
            std::thread::sleep(std::time::Duration::from_millis(500));

            if let Err(e) = handle.note_off(0, note) {
                return CommandResult::Error(format!("Failed to send note off: {}", e));
            }

            CommandResult::Message(format!(
                "🎹 Test complete: sent {}{} (MIDI note {}) for 500ms",
                note_name, octave, note
            ))
        }
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}

/// Handle `output <mode>` command - set output mode (midi, audio, both)
pub fn cmd_output_mode(args: &str, ctx: &mut CommandContext) -> CommandResult {
    let mode_arg = args.to_lowercase().trim().to_string();

    match &ctx.midi_handle {
        Some(handle) => {
            if mode_arg.is_empty() {
                // Show current mode
                let mode = handle.output_mode();
                let mode_desc = match mode {
                    OutputMode::Both => "Both audio + MIDI",
                    OutputMode::MidiOnly => "MIDI only (internal synth muted)",
                    OutputMode::AudioOnly => "Audio only (no MIDI output)",
                };
                return CommandResult::Message(format!("🔊 Output mode: {}", mode_desc.cyan()));
            }

            let new_mode = match mode_arg.as_str() {
                "midi" | "midi-only" | "midionly" => OutputMode::MidiOnly,
                "audio" | "audio-only" | "audioonly" => OutputMode::AudioOnly,
                "both" | "all" | "audio+midi" | "midi+audio" => OutputMode::Both,
                _ => {
                    return CommandResult::Error(
                        "Usage: output <midi|audio|both>\n  midi  - MIDI output only\n  audio - Internal synth only\n  both  - Both (default)".to_string(),
                    )
                }
            };

            handle.set_output_mode(new_mode);
            let mode_desc = match new_mode {
                OutputMode::Both => "🔊 Output mode: Both audio + MIDI".green(),
                OutputMode::MidiOnly => "🎹 Output mode: MIDI only (internal synth muted)".cyan(),
                OutputMode::AudioOnly => "🔈 Output mode: Audio only (no MIDI)".yellow(),
            };
            CommandResult::Message(mode_desc.to_string())
        }
        None => CommandResult::Error("MIDI output not initialized".to_string()),
    }
}
