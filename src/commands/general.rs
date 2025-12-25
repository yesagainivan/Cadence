//! General REPL commands (help, quit, tempo)

use crate::commands::{CommandContext, CommandResult};
use colored::*;

/// Handle `help` command
pub fn cmd_help(_args: &str, _ctx: &mut CommandContext) -> CommandResult {
    print_help();
    CommandResult::Success
}

/// Handle `quit` or `exit` command
pub fn cmd_quit(_args: &str, _ctx: &mut CommandContext) -> CommandResult {
    CommandResult::Exit
}

/// Handle `tempo [bpm]` command
pub fn cmd_tempo(args: &str, ctx: &mut CommandContext) -> CommandResult {
    if args.is_empty() {
        return CommandResult::Message(format!("Current tempo: {:.1} BPM", ctx.clock.get_bpm()));
    }

    match args.parse::<f32>() {
        Ok(bpm) if bpm > 0.0 && bpm <= 400.0 => {
            ctx.clock.set_bpm(bpm);
            CommandResult::Message(
                format!("🎵 Tempo set to {:.1} BPM", bpm)
                    .bright_green()
                    .to_string(),
            )
        }
        _ => CommandResult::Error("Invalid tempo. Use a value between 1-400 BPM".to_string()),
    }
}

/// Handle `watch [file]` command
pub fn cmd_watch(args: &str, _ctx: &mut CommandContext) -> CommandResult {
    if args.is_empty() {
        return CommandResult::Error("Usage: watch <file>".to_string());
    }
    CommandResult::Watch(args.to_string())
}

/// Print help information
fn print_help() {
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
    println!("{}", "Audio Commands:".green());
    println!("  {}  - Play a note or chord", "audio play <expr>".cyan());
    println!(
        "  {} - Play progression",
        "audio play progression <expr> [loop] [queue]".cyan()
    );
    println!(
        "  {}           - Stop current playback",
        "audio stop".cyan()
    );
    println!("  {}  - Set volume (0-100)", "audio volume <level>".cyan());
    println!("  {}        - Show current tempo", "tempo".cyan());
    println!("  {}    - Set tempo", "tempo <bpm>".cyan());
    println!();
    println!("{}", "MIDI Commands:".green());
    println!("  {}       - List MIDI output ports", "midi devices".cyan());
    println!("  {} - Connect to MIDI port", "midi connect <port>".cyan());
    println!("  {}    - Disconnect MIDI", "midi disconnect".cyan());
    println!(
        "  {}     - Set channel (1-16 or 'auto')",
        "midi channel".cyan()
    );
    println!("  {}        - Show MIDI status", "midi status".cyan());
    println!("  {}         - All notes off (panic)", "midi panic".cyan());
    println!();
    println!("{}", "Other Commands:".green());
    println!(
        "  {}            - List active tracks",
        "tracks".bright_green()
    );
    println!("  {}              - Show this help", "help".bright_green());
    println!("  {}              - Exit the REPL", "quit".bright_red());
}
