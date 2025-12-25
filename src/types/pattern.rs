//! Pattern type for TidalCycles-inspired mini-notation
//!
//! Enables cycle-based patterns like `"C E G _"` where all steps fit into one cycle,
//! with support for rests, repetition, and grouping.

use crate::types::{Chord, Note};
use anyhow::{Result, anyhow};
use std::fmt;
use std::str::FromStr;

/// A single step in a pattern
#[derive(Clone, Debug, PartialEq)]
pub enum PatternStep {
    /// Single note: C, D#, etc.
    Note(Note),
    /// Chord: [C, E, G]
    Chord(Chord),
    /// Rest (silence): _
    Rest,
    /// Group of steps that share one slot: [C E]
    Group(Vec<PatternStep>),
    /// Repeat a step N times: C*3
    Repeat(Box<PatternStep>, usize),
}

impl PatternStep {
    /// Get the weight of this step for duration calculation
    /// Groups and repeats have weight 1 (they fit in one slot)
    pub fn weight(&self) -> f32 {
        1.0
    }

    /// Flatten this step into individual notes for playback
    /// Returns (frequencies, is_rest) pairs
    pub fn to_frequencies(&self) -> Vec<(Vec<f32>, bool)> {
        match self {
            PatternStep::Note(n) => vec![(vec![n.frequency()], false)],
            PatternStep::Chord(c) => {
                vec![(c.notes_vec().iter().map(|n| n.frequency()).collect(), false)]
            }
            PatternStep::Rest => vec![(vec![], true)],
            PatternStep::Group(steps) => steps.iter().flat_map(|s| s.to_frequencies()).collect(),
            PatternStep::Repeat(step, count) => {
                let inner = step.to_frequencies();
                (0..*count).flat_map(|_| inner.clone()).collect()
            }
        }
    }

    /// Transpose this step by the given number of semitones
    pub fn transpose(&self, semitones: i8) -> PatternStep {
        match self {
            PatternStep::Note(n) => PatternStep::Note(*n + semitones),
            PatternStep::Chord(c) => PatternStep::Chord(c.clone() + semitones),
            PatternStep::Rest => PatternStep::Rest,
            PatternStep::Group(steps) => {
                PatternStep::Group(steps.iter().map(|s| s.transpose(semitones)).collect())
            }
            PatternStep::Repeat(step, count) => {
                PatternStep::Repeat(Box::new(step.transpose(semitones)), *count)
            }
        }
    }
}

impl fmt::Display for PatternStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternStep::Note(n) => write!(f, "{}", n),
            PatternStep::Chord(c) => write!(f, "{}", c),
            PatternStep::Rest => write!(f, "_"),
            PatternStep::Group(steps) => {
                write!(f, "[")?;
                for (i, s) in steps.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", s)?;
                }
                write!(f, "]")
            }
            PatternStep::Repeat(step, count) => write!(f, "{}*{}", step, count),
        }
    }
}

/// A cycle-based pattern
///
/// All steps in a pattern fit into one cycle (default 4 beats).
/// More steps = faster per-step, fewer steps = slower per-step.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// Steps in the pattern
    pub steps: Vec<PatternStep>,
    /// Beats per cycle (default 4)
    pub beats_per_cycle: f32,
    /// Optional ADSR envelope parameters for this pattern
    pub envelope: Option<(f32, f32, f32, f32)>, // (attack, decay, sustain, release)
}

impl Pattern {
    /// Create an empty pattern
    pub fn new() -> Self {
        Pattern {
            steps: Vec::new(),
            beats_per_cycle: 4.0,
            envelope: None,
        }
    }

    /// Create a pattern with given steps
    pub fn with_steps(steps: Vec<PatternStep>) -> Self {
        Pattern {
            steps,
            beats_per_cycle: 4.0,
            envelope: None,
        }
    }

    /// Create a pattern from a progression (each chord becomes a step)
    /// This enables progressions to use pattern playback with proper envelopes
    pub fn from_progression(progression: &crate::types::Progression) -> Self {
        let steps: Vec<PatternStep> = progression
            .chords()
            .map(|chord| PatternStep::Chord(chord.clone()))
            .collect();

        // Set cycle length to match number of chords (1 beat per chord by default)
        let beats_per_cycle = steps.len() as f32;

        Pattern {
            steps,
            beats_per_cycle,
            envelope: Some((0.01, 0.1, 0.7, 0.3)), // Default smooth envelope
        }
    }

    /// Convert a pattern back to a Progression (only works if all steps are chords)
    /// This enables voice leading analysis on patterns that contain chords
    pub fn to_progression(&self) -> Option<crate::types::Progression> {
        let mut chords = Vec::new();

        for step in &self.steps {
            match step {
                PatternStep::Chord(chord) => chords.push(chord.clone()),
                PatternStep::Note(note) => {
                    // Single notes become single-note chords
                    chords.push(crate::types::Chord::from_notes(vec![*note]));
                }
                PatternStep::Repeat(inner, count) => {
                    // Expand repeats
                    if let PatternStep::Chord(chord) = inner.as_ref() {
                        for _ in 0..*count {
                            chords.push(chord.clone());
                        }
                    } else if let PatternStep::Note(note) = inner.as_ref() {
                        for _ in 0..*count {
                            chords.push(crate::types::Chord::from_notes(vec![*note]));
                        }
                    } else {
                        return None; // Can't convert groups of groups
                    }
                }
                _ => return None, // Rests and groups can't become progressions
            }
        }

        if chords.is_empty() {
            None
        } else {
            Some(crate::types::Progression::from_chords(chords))
        }
    }

    /// Set beats per cycle
    pub fn with_cycle_length(mut self, beats: f32) -> Self {
        self.beats_per_cycle = beats;
        self
    }

    /// Get the duration of each step in beats
    pub fn step_beats(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        self.beats_per_cycle / self.steps.len() as f32
    }

    /// Total number of playable events (expanding groups and repeats)
    pub fn event_count(&self) -> usize {
        self.steps.iter().map(|s| s.to_frequencies().len()).sum()
    }

    /// Get all frequencies with their durations
    /// Returns: Vec of (frequencies, duration_beats, is_rest)
    pub fn to_events(&self) -> Vec<(Vec<f32>, f32, bool)> {
        let mut events = Vec::new();
        let step_beats = self.step_beats();

        for step in &self.steps {
            let freqs_list = step.to_frequencies();
            let count = freqs_list.len();
            let event_duration = step_beats / count as f32;

            for (freqs, is_rest) in freqs_list {
                events.push((freqs, event_duration, is_rest));
            }
        }

        events
    }

    /// Transform: speed up by factor (plays N times per cycle)
    pub fn fast(mut self, factor: usize) -> Self {
        self.beats_per_cycle /= factor as f32;
        self
    }

    /// Transform: slow down by factor (takes N cycles to complete)
    pub fn slow(mut self, factor: usize) -> Self {
        self.beats_per_cycle *= factor as f32;
        self
    }

    /// Transform: reverse order
    pub fn rev(mut self) -> Self {
        self.steps.reverse();
        self
    }

    /// Set custom ADSR envelope (attack, decay, sustain, release in seconds)
    /// sustain is a level 0.0-1.0, others are times in seconds
    pub fn env(mut self, attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        self.envelope = Some((attack, decay, sustain, release));
        self
    }

    /// Set envelope from preset name
    pub fn env_preset(mut self, preset: &str) -> Self {
        self.envelope = match preset {
            "pluck" => Some((0.001, 0.15, 0.0, 0.1)),
            "pad" => Some((0.3, 0.2, 0.8, 0.5)),
            "perc" => Some((0.001, 0.2, 0.0, 0.05)),
            "organ" => Some((0.005, 0.0, 1.0, 0.01)),
            _ => Some((0.01, 0.1, 0.7, 0.2)), // default
        };
        self
    }

    /// Transpose all notes in the pattern by the given number of semitones
    pub fn transpose(mut self, semitones: i8) -> Self {
        self.steps = self
            .steps
            .into_iter()
            .map(|s| s.transpose(semitones))
            .collect();
        self
    }

    /// Parse from mini-notation string
    ///
    /// Syntax:
    /// - Notes: `C`, `D#`, `Bb`
    /// - Rests: `_`
    /// - Repetition: `C*3`
    /// - Groups: `[C E]`
    pub fn parse(notation: &str) -> Result<Pattern> {
        let notation = notation.trim();
        if notation.is_empty() {
            return Ok(Pattern::new());
        }

        let steps = parse_steps(notation)?;
        Ok(Pattern::with_steps(steps))
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for Pattern {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Pattern::parse(s)
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for (i, step) in self.steps.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", step)?;
        }
        write!(f, "\"")
    }
}

// ============================================================================
// Mini-notation parser
// ============================================================================

fn parse_steps(notation: &str) -> Result<Vec<PatternStep>> {
    let mut steps = Vec::new();
    let mut chars = notation.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            // Whitespace - skip
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            // Rest
            '_' => {
                chars.next();
                let step = maybe_parse_repeat(&mut chars, PatternStep::Rest)?;
                steps.push(step);
            }
            // Group start
            '[' => {
                chars.next(); // consume '['
                let group_content = take_until_bracket(&mut chars)?;

                // Check if it looks like a chord [C, E, G] or a group [C E G]
                if group_content.contains(',') {
                    // It's a chord - parse comma-separated notes
                    let note_strs: Vec<&str> = group_content.split(',').map(|s| s.trim()).collect();
                    let chord = Chord::from_note_strings(note_strs)?;
                    let step = maybe_parse_repeat(&mut chars, PatternStep::Chord(chord))?;
                    steps.push(step);
                } else {
                    // It's a group
                    let inner_steps = parse_steps(&group_content)?;
                    let step = maybe_parse_repeat(&mut chars, PatternStep::Group(inner_steps))?;
                    steps.push(step);
                }
            }
            // Note (starts with letter)
            'A'..='G' | 'a'..='g' => {
                let note_str = take_note(&mut chars);
                let note: Note = note_str.parse()?;
                let step = maybe_parse_repeat(&mut chars, PatternStep::Note(note))?;
                steps.push(step);
            }
            // Unknown
            _ => {
                return Err(anyhow!("Unexpected character in pattern: '{}'", c));
            }
        }
    }

    Ok(steps)
}

/// Take content until matching ']', handling nested brackets
fn take_until_bracket(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String> {
    let mut content = String::new();
    let mut depth = 1;

    while let Some(c) = chars.next() {
        match c {
            '[' => {
                depth += 1;
                content.push(c);
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(content);
                }
                content.push(c);
            }
            _ => content.push(c),
        }
    }

    Err(anyhow!("Unclosed bracket in pattern"))
}

/// Take a note token (e.g., "C", "D#", "Bb", "C4", "D#3")
fn take_note(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut note = String::new();

    // First char is the note letter
    if let Some(c) = chars.next() {
        note.push(c.to_ascii_uppercase());
    }

    // Check for accidental
    if let Some(&c) = chars.peek() {
        if c == '#' {
            // Sharp - always consume
            note.push(chars.next().unwrap());
        } else if c == 'b' {
            // Lowercase 'b' is always a flat indicator
            note.push(chars.next().unwrap());
        }
        // Note: uppercase 'B' would be parsed as a new note, not a flat
    }

    // Check for octave number
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '-' {
            note.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    note
}

/// Parse optional *N repetition suffix
fn maybe_parse_repeat(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    step: PatternStep,
) -> Result<PatternStep> {
    if chars.peek() == Some(&'*') {
        chars.next(); // consume '*'
        let mut count_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                count_str.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        if count_str.is_empty() {
            return Err(anyhow!("Expected number after '*'"));
        }
        let count: usize = count_str.parse()?;
        Ok(PatternStep::Repeat(Box::new(step), count))
    } else {
        Ok(step)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_notes() {
        let p = Pattern::parse("C E G").unwrap();
        assert_eq!(p.steps.len(), 3);
        assert!(matches!(&p.steps[0], PatternStep::Note(_)));
    }

    #[test]
    fn test_parse_rest() {
        let p = Pattern::parse("C _ E _").unwrap();
        assert_eq!(p.steps.len(), 4);
        assert!(matches!(&p.steps[1], PatternStep::Rest));
        assert!(matches!(&p.steps[3], PatternStep::Rest));
    }

    #[test]
    fn test_parse_repeat() {
        let p = Pattern::parse("C*3 E").unwrap();
        assert_eq!(p.steps.len(), 2);
        match &p.steps[0] {
            PatternStep::Repeat(_, count) => assert_eq!(*count, 3),
            _ => panic!("Expected Repeat"),
        }
    }

    #[test]
    fn test_parse_group() {
        let p = Pattern::parse("[C E] G").unwrap();
        assert_eq!(p.steps.len(), 2);
        match &p.steps[0] {
            PatternStep::Group(inner) => assert_eq!(inner.len(), 2),
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_parse_chord() {
        let p = Pattern::parse("[C, E, G] _").unwrap();
        assert_eq!(p.steps.len(), 2);
        assert!(matches!(&p.steps[0], PatternStep::Chord(_)));
    }

    #[test]
    fn test_step_beats() {
        let p = Pattern::parse("C E G _").unwrap();
        assert_eq!(p.beats_per_cycle, 4.0);
        assert_eq!(p.step_beats(), 1.0); // 4 steps, 4 beats = 1 beat each
    }

    #[test]
    fn test_fast() {
        let p = Pattern::parse("C E").unwrap().fast(2);
        assert_eq!(p.beats_per_cycle, 2.0); // Now plays in 2 beats
    }

    #[test]
    fn test_slow() {
        let p = Pattern::parse("C E").unwrap().slow(2);
        assert_eq!(p.beats_per_cycle, 8.0); // Now takes 8 beats
    }

    #[test]
    fn test_rev() {
        let p = Pattern::parse("C D E").unwrap().rev();
        // E D C now
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
            _ => panic!("Expected Note"),
        }
    }

    #[test]
    fn test_to_events() {
        let p = Pattern::parse("C E G _").unwrap();
        let events = p.to_events();
        assert_eq!(events.len(), 4);
        assert!(!events[0].2); // Not a rest
        assert!(events[3].2); // Is a rest
    }

    #[test]
    fn test_display() {
        let p = Pattern::parse("C E G").unwrap();
        assert_eq!(format!("{}", p), "\"C E G\"");
    }

    #[test]
    fn test_parse_flat_notes() {
        let p = Pattern::parse("Bb Eb Ab").unwrap();
        assert_eq!(p.steps.len(), 3);
        // Bb should be pitch class 10 (A#/Bb)
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 10),
            _ => panic!("Expected Note"),
        }
        // Eb should be pitch class 3 (D#/Eb)
        match &p.steps[1] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 3),
            _ => panic!("Expected Note"),
        }
    }

    #[test]
    fn test_chord_repeat_expansion() {
        // Test that [C,E,G]*2 [F,A,C] gives 3 events (2 C majors + 1 F major)
        let p = Pattern::parse("[C,E,G]*2 [F,A,C]").unwrap();
        assert_eq!(p.steps.len(), 2); // 2 steps: Repeat(Chord) and Chord

        // Check the first step is a Repeat containing a Chord
        match &p.steps[0] {
            PatternStep::Repeat(inner, count) => {
                assert_eq!(*count, 2);
                assert!(matches!(**inner, PatternStep::Chord(_)));
            }
            _ => panic!("Expected Repeat"),
        }

        // Check to_events expands correctly
        let events = p.to_events();
        assert_eq!(
            events.len(),
            3,
            "Should have 3 events: [C,E,G], [C,E,G], [F,A,C]"
        );

        // First two events should be C major (3 notes)
        assert_eq!(events[0].0.len(), 3);
        assert_eq!(events[1].0.len(), 3);
        // Last event should be F major (3 notes)
        assert_eq!(events[2].0.len(), 3);

        // Durations should be 2.0 beats for each (cycle=4, 2 slots, each slot has 2 and 1 events)
        // Step 1: Repeat(Chord)*2 = 2 events at step_beats/2 each
        // Step 2: Chord = 1 event at step_beats
        // With 2 steps and 4 beat cycle, step_beats = 2.0
        // Repeat expands to 2 events at 2.0/2 = 1.0 each
        // Last chord at 2.0
        assert_eq!(events[0].1, 1.0);
        assert_eq!(events[1].1, 1.0);
        assert_eq!(events[2].1, 2.0);
    }

    #[test]
    fn test_voice_leading_frequency_order_in_playback() {
        // This test verifies that after voice leading optimization,
        // the frequencies sent to MIDI/audio are in the correct order
        use crate::types::Progression;

        let c_maj = Chord::from_note_strings(vec!["C4", "E4", "G4"]).unwrap();
        let f_maj = Chord::from_note_strings(vec!["F4", "A4", "C4"]).unwrap();
        let prog = Progression::from_chords(vec![c_maj.clone(), f_maj.clone()]);

        // Optimize voice leading
        let optimized = prog.optimize_voice_leading();

        // Verify the second chord is reordered to [C, F, A]
        let second_chord = optimized.get(1).unwrap();
        let notes = second_chord.notes_vec();
        assert_eq!(notes[0].pitch_class(), 0, "First note should be C");
        assert_eq!(notes[1].pitch_class(), 5, "Second note should be F");
        assert_eq!(notes[2].pitch_class(), 9, "Third note should be A");

        // Convert to pattern (this is what playback does)
        let pattern = optimized.to_pattern();

        // Get the playback events
        let events = pattern.to_events();
        assert_eq!(events.len(), 2, "Should have 2 chord events");

        // Check the frequencies of the second chord
        let (freqs, _duration, _is_rest) = &events[1];
        assert_eq!(freqs.len(), 3, "Second chord should have 3 frequencies");

        // Calculate expected frequencies
        let c4_freq = notes[0].frequency(); // C4 ~261.63 Hz
        let f4_freq = notes[1].frequency(); // F4 ~349.23 Hz
        let a4_freq = notes[2].frequency(); // A4 ~440 Hz

        // Frequencies should be in order: C, F, A
        let tolerance = 0.1; // Small tolerance for float comparison
        assert!(
            (freqs[0] - c4_freq).abs() < tolerance,
            "First frequency should be C4 (~{}), got {}",
            c4_freq,
            freqs[0]
        );
        assert!(
            (freqs[1] - f4_freq).abs() < tolerance,
            "Second frequency should be F4 (~{}), got {}",
            f4_freq,
            freqs[1]
        );
        assert!(
            (freqs[2] - a4_freq).abs() < tolerance,
            "Third frequency should be A4 (~{}), got {}",
            a4_freq,
            freqs[2]
        );
    }
}
