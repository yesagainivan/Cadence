//! Pattern type for TidalCycles-inspired mini-notation
//!
//! Enables cycle-based patterns like `"C E G _"` where all steps fit into one cycle,
//! with support for rests, repetition, and grouping.

use super::audio_config::Waveform;
use super::drum::DrumSound;
use super::time::{beats, to_f32, Time};
use crate::types::{Chord, Note};
use anyhow::{anyhow, Result};
use num_rational::Ratio;
use std::fmt;
use std::str::FromStr;

// ============================================================================
// Rich Event Types for Visualization & Playback
// ============================================================================

/// Information about a single note, preserving full identity for accurate
/// MIDI output and visualization without floating-point conversion.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoteInfo {
    /// MIDI note number (0-127), computed directly from pitch_class + octave
    pub midi: u8,
    /// Frequency in Hz (for audio synthesis)
    pub frequency: f32,
    /// Display name with octave (e.g., "C#4", "Bb3")
    pub name: String,
    /// Pitch class (0-11): C=0, C#=1, D=2, etc.
    pub pitch_class: u8,
    /// Octave in scientific pitch notation (4 = middle C octave)
    pub octave: i8,
}

impl NoteInfo {
    /// Create NoteInfo from a Note
    pub fn from_note(note: &Note) -> Self {
        NoteInfo {
            midi: note.midi_note(),
            frequency: note.frequency(),
            name: note.full_name(),
            pitch_class: note.pitch_class(),
            octave: note.octave(),
        }
    }
}

/// A single playback event with full note data for visualization and playback.
/// Unlike the raw (frequencies, duration, is_rest) tuple, this preserves
/// note identity through the entire pipeline.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaybackEvent {
    /// Notes in this event (empty for rests)
    pub notes: Vec<NoteInfo>,
    /// Drum sounds in this event (for percussion)
    pub drums: Vec<DrumSound>,
    /// Start time in beats relative to pattern start (exact rational)
    pub start_beat: Time,
    /// Duration in beats (exact rational)
    pub duration: Time,
    /// Whether this is a rest (silence)
    pub is_rest: bool,
}

impl PlaybackEvent {
    /// Get start_beat as f32 for audio output
    #[inline]
    pub fn start_beat_f32(&self) -> f32 {
        to_f32(self.start_beat)
    }

    /// Get duration as f32 for audio output
    #[inline]
    pub fn duration_f32(&self) -> f32 {
        to_f32(self.duration)
    }
}

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
    /// Unresolved variable reference (resolved at evaluation time)
    Variable(String),
    /// Drum sound: kick, snare, hh, etc.
    Drum(DrumSound),
    /// Weighted step: C@2 means C takes 2 units of duration
    Weighted(Box<PatternStep>, usize),
    /// Cycle-based alternation: <C D E> plays one element per cycle
    Alternation(Vec<PatternStep>),
}

impl PatternStep {
    /// Get the weight of this step for duration calculation.
    /// Weighted steps return their weight, all others return 1.
    pub fn weight(&self) -> usize {
        match self {
            PatternStep::Weighted(_, w) => *w,
            _ => 1,
        }
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
            PatternStep::Variable(name) => {
                panic!(
                    "Unresolved variable '{}' in pattern - call resolve_variables() first",
                    name
                )
            }
            // Drums return empty frequencies - the sound comes from DrumOscillator, not melodic oscillators
            PatternStep::Drum(_) => vec![(vec![], false)],
            // Weighted delegates to inner (weight is handled at duration calculation)
            PatternStep::Weighted(inner, _) => inner.to_frequencies(),
            // Alternation returns first step for static contexts (real selection happens at playback)
            PatternStep::Alternation(steps) => steps
                .first()
                .map(|s| s.to_frequencies())
                .unwrap_or_default(),
        }
    }

    /// Flatten this step into rich note info for visualization and accurate MIDI
    /// Returns (Vec<NoteInfo>, is_rest) pairs preserving full note identity
    pub fn to_note_infos(&self) -> Vec<(Vec<NoteInfo>, bool)> {
        match self {
            PatternStep::Note(n) => vec![(vec![NoteInfo::from_note(n)], false)],
            PatternStep::Chord(c) => {
                let notes: Vec<NoteInfo> = c.notes_vec().iter().map(NoteInfo::from_note).collect();
                vec![(notes, false)]
            }
            PatternStep::Rest => vec![(vec![], true)],
            PatternStep::Group(steps) => steps.iter().flat_map(|s| s.to_note_infos()).collect(),
            PatternStep::Repeat(step, count) => {
                let inner = step.to_note_infos();
                (0..*count).flat_map(|_| inner.clone()).collect()
            }
            PatternStep::Variable(name) => {
                panic!(
                    "Unresolved variable '{}' in pattern - call resolve_variables() first",
                    name
                )
            }
            PatternStep::Drum(d) => {
                // Create a pseudo-NoteInfo for drum visualization
                vec![(
                    vec![NoteInfo {
                        midi: d.midi_note(),
                        frequency: d.display_frequency(),
                        name: d.short_name().to_string(),
                        pitch_class: d.midi_note() % 12,
                        octave: (d.midi_note() / 12) as i8 - 1,
                    }],
                    false,
                )]
            }
            // Weighted delegates to inner (weight is handled at duration calculation)
            PatternStep::Weighted(inner, _) => inner.to_note_infos(),
            // Alternation returns first step for static contexts
            PatternStep::Alternation(steps) => {
                steps.first().map(|s| s.to_note_infos()).unwrap_or_default()
            }
        }
    }

    /// Flatten this step into separate notes and drums for playback
    /// Returns (Vec<NoteInfo>, Vec<DrumSound>, is_rest) preserving type distinction
    pub fn to_step_info(&self) -> Vec<(Vec<NoteInfo>, Vec<DrumSound>, bool)> {
        match self {
            PatternStep::Note(n) => vec![(vec![NoteInfo::from_note(n)], vec![], false)],
            PatternStep::Chord(c) => {
                let notes: Vec<NoteInfo> = c.notes_vec().iter().map(NoteInfo::from_note).collect();
                vec![(notes, vec![], false)]
            }
            PatternStep::Rest => vec![(vec![], vec![], true)],
            PatternStep::Group(steps) => steps.iter().flat_map(|s| s.to_step_info()).collect(),
            PatternStep::Repeat(step, count) => {
                let inner = step.to_step_info();
                (0..*count).flat_map(|_| inner.clone()).collect()
            }
            PatternStep::Variable(name) => {
                panic!(
                    "Unresolved variable '{}' in pattern - call resolve_variables() first",
                    name
                )
            }
            PatternStep::Drum(d) => vec![(vec![], vec![*d], false)],
            // Weighted delegates to inner (weight is handled at duration calculation)
            PatternStep::Weighted(inner, _) => inner.to_step_info(),
            // Alternation returns first step for static contexts
            PatternStep::Alternation(steps) => {
                steps.first().map(|s| s.to_step_info()).unwrap_or_default()
            }
        }
    }

    /// Flatten this step into separate notes and drums for playback, with cycle-awareness.
    /// For Alternation steps, selects the appropriate element based on the current cycle.
    /// Returns (Vec<NoteInfo>, Vec<DrumSound>, is_rest) preserving type distinction.
    pub fn to_step_info_for_cycle(
        &self,
        cycle: usize,
    ) -> Vec<(Vec<NoteInfo>, Vec<DrumSound>, bool)> {
        match self {
            PatternStep::Note(n) => vec![(vec![NoteInfo::from_note(n)], vec![], false)],
            PatternStep::Chord(c) => {
                let notes: Vec<NoteInfo> = c.notes_vec().iter().map(NoteInfo::from_note).collect();
                vec![(notes, vec![], false)]
            }
            PatternStep::Rest => vec![(vec![], vec![], true)],
            PatternStep::Group(steps) => steps
                .iter()
                .flat_map(|s| s.to_step_info_for_cycle(cycle))
                .collect(),
            PatternStep::Repeat(step, count) => {
                let inner = step.to_step_info_for_cycle(cycle);
                (0..*count).flat_map(|_| inner.clone()).collect()
            }
            PatternStep::Variable(name) => {
                panic!(
                    "Unresolved variable '{}' in pattern - call resolve_variables() first",
                    name
                )
            }
            PatternStep::Drum(d) => vec![(vec![], vec![*d], false)],
            PatternStep::Weighted(inner, _) => inner.to_step_info_for_cycle(cycle),
            // Alternation: select element based on cycle
            PatternStep::Alternation(steps) => {
                if steps.is_empty() {
                    return vec![];
                }
                let idx = cycle % steps.len();
                steps[idx].to_step_info_for_cycle(cycle)
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
            PatternStep::Variable(name) => PatternStep::Variable(name.clone()),
            PatternStep::Drum(d) => PatternStep::Drum(*d), // Drums don't transpose
            PatternStep::Weighted(inner, weight) => {
                PatternStep::Weighted(Box::new(inner.transpose(semitones)), *weight)
            }
            PatternStep::Alternation(steps) => {
                PatternStep::Alternation(steps.iter().map(|s| s.transpose(semitones)).collect())
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
            PatternStep::Variable(name) => write!(f, "{}", name),
            PatternStep::Drum(d) => write!(f, "{}", d),
            PatternStep::Weighted(inner, weight) => write!(f, "{}@{}", inner, weight),
            PatternStep::Alternation(steps) => {
                write!(f, "<")?;
                for (i, s) in steps.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", s)?;
                }
                write!(f, ">")
            }
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
    /// Beats per cycle (default 4) - exact rational for drift-free timing
    pub beats_per_cycle: Time,
    /// Optional ADSR envelope parameters for this pattern
    pub envelope: Option<(f32, f32, f32, f32)>, // (attack, decay, sustain, release)
    /// Optional waveform for this pattern
    pub waveform: Option<Waveform>,
    /// Optional stereo pan (0.0 = left, 0.5 = center, 1.0 = right)
    pub pan: Option<f32>,
}

impl Pattern {
    /// Create an empty pattern
    pub fn new() -> Self {
        Pattern {
            steps: Vec::new(),
            beats_per_cycle: beats(4),
            envelope: None,
            waveform: None,
            pan: None,
        }
    }

    /// Create a pattern with given steps
    pub fn with_steps(steps: Vec<PatternStep>) -> Self {
        Pattern {
            steps,
            beats_per_cycle: beats(4),
            envelope: None,
            waveform: None,
            pan: None,
        }
    }

    /// Set beats per cycle (accepts integer for convenience)
    pub fn with_cycle_length(mut self, beats_count: i64) -> Self {
        self.beats_per_cycle = beats(beats_count);
        self
    }

    /// Set beats per cycle from a Time value
    pub fn with_cycle_length_time(mut self, cycle_time: Time) -> Self {
        self.beats_per_cycle = cycle_time;
        self
    }

    /// Get the duration of each step in beats (exact rational)
    pub fn step_beats(&self) -> Time {
        if self.steps.is_empty() {
            return Ratio::from_integer(0);
        }
        self.beats_per_cycle / self.steps.len() as i64
    }

    /// Get beats_per_cycle as f32 for audio output
    #[inline]
    pub fn beats_per_cycle_f32(&self) -> f32 {
        to_f32(self.beats_per_cycle)
    }

    /// Total number of playable events (expanding groups and repeats)
    pub fn event_count(&self) -> usize {
        self.steps.iter().map(|s| s.to_frequencies().len()).sum()
    }

    /// Get all frequencies with their durations (f32 for audio output)
    /// Returns: Vec of (frequencies, duration_beats_f32, is_rest)
    pub fn to_events(&self) -> Vec<(Vec<f32>, f32, bool)> {
        let mut events = Vec::new();
        let step_beats = self.step_beats();

        for step in &self.steps {
            let freqs_list = step.to_frequencies();
            let count = freqs_list.len();
            let event_duration = step_beats / count as i64;

            for (freqs, is_rest) in freqs_list {
                events.push((freqs, to_f32(event_duration), is_rest));
            }
        }

        events
    }

    /// Get rich playback events with full note identity for visualization and accurate MIDI.
    /// Unlike `to_events()`, this preserves note names, MIDI numbers, and computes start times.
    ///
    /// Supports weighted steps: `C@2 D` means C gets 2/3 of the cycle, D gets 1/3.
    ///
    /// # Returns
    /// Vec of `PlaybackEvent` with:
    /// - `notes`: Full `NoteInfo` for each note (MIDI, frequency, name, pitch_class, octave)
    /// - `start_beat`: When this event starts relative to pattern beginning
    /// - `duration`: How long this event lasts in beats
    /// - `is_rest`: Whether this is silence
    pub fn to_rich_events(&self) -> Vec<PlaybackEvent> {
        let mut events = Vec::new();

        // Calculate total weight of all steps
        let total_weight: i64 = self.steps.iter().map(|s| s.weight() as i64).sum();

        // Duration per weight unit (exact rational)
        // If no steps, avoid division by zero
        if total_weight == 0 {
            return events;
        }
        let unit_duration = self.beats_per_cycle / total_weight;

        let mut current_beat: Time = Ratio::from_integer(0);

        for step in &self.steps {
            let step_weight = step.weight() as i64;
            let step_duration = unit_duration * step_weight;

            let step_info_list = step.to_step_info();
            let sub_count = step_info_list.len() as i64;
            let event_duration = if sub_count > 0 {
                step_duration / sub_count
            } else {
                step_duration
            };

            for (notes, drums, is_rest) in step_info_list {
                events.push(PlaybackEvent {
                    notes,
                    drums,
                    start_beat: current_beat,
                    duration: event_duration,
                    is_rest,
                });
                current_beat = current_beat + event_duration;
            }
        }

        events
    }

    /// Get rich playback events with cycle-aware alternation selection.
    /// This is the method to use for actual playback, where Alternation steps
    /// need to select the correct element based on the current cycle.
    ///
    /// # Arguments
    /// * `cycle` - The current cycle number (0-indexed), used to select alternation elements
    pub fn to_rich_events_for_cycle(&self, cycle: usize) -> Vec<PlaybackEvent> {
        let mut events = Vec::new();

        let total_weight: i64 = self.steps.iter().map(|s| s.weight() as i64).sum();
        if total_weight == 0 {
            return events;
        }
        let unit_duration = self.beats_per_cycle / total_weight;

        let mut current_beat: Time = Ratio::from_integer(0);

        for step in &self.steps {
            let step_weight = step.weight() as i64;
            let step_duration = unit_duration * step_weight;

            // Use cycle-aware step info for alternation support
            let step_info_list = step.to_step_info_for_cycle(cycle);
            let sub_count = step_info_list.len() as i64;
            let event_duration = if sub_count > 0 {
                step_duration / sub_count
            } else {
                step_duration
            };

            for (notes, drums, is_rest) in step_info_list {
                events.push(PlaybackEvent {
                    notes,
                    drums,
                    start_beat: current_beat,
                    duration: event_duration,
                    is_rest,
                });
                current_beat = current_beat + event_duration;
            }
        }

        events
    }

    /// Transform: speed up by factor (plays N times per cycle)
    pub fn fast(mut self, factor: usize) -> Self {
        self.beats_per_cycle = self.beats_per_cycle / factor as i64;
        self
    }

    /// Transform: slow down by factor (takes N cycles to complete)
    pub fn slow(mut self, factor: usize) -> Self {
        self.beats_per_cycle = self.beats_per_cycle * factor as i64;
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

    /// Set waveform for this pattern
    pub fn wave(mut self, waveform: Waveform) -> Self {
        self.waveform = Some(waveform);
        self
    }

    /// Set waveform from preset name (sine, saw, square, triangle)
    pub fn wave_preset(mut self, preset: &str) -> Self {
        self.waveform = Waveform::from_str(preset);
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

    // ========================================================================
    // Variable Resolution
    // ========================================================================

    /// Check if this pattern contains any unresolved variable references
    pub fn has_variables(&self) -> bool {
        fn step_has_variables(step: &PatternStep) -> bool {
            match step {
                PatternStep::Variable(_) => true,
                PatternStep::Group(steps) => steps.iter().any(step_has_variables),
                PatternStep::Repeat(inner, _) => step_has_variables(inner),
                PatternStep::Weighted(inner, _) => step_has_variables(inner),
                PatternStep::Alternation(steps) => steps.iter().any(step_has_variables),
                _ => false,
            }
        }
        self.steps.iter().any(step_has_variables)
    }

    /// Get all unresolved variable names in this pattern
    pub fn get_variable_names(&self) -> Vec<String> {
        fn collect_vars(step: &PatternStep, vars: &mut Vec<String>) {
            match step {
                PatternStep::Variable(name) => vars.push(name.clone()),
                PatternStep::Group(steps) => {
                    for s in steps {
                        collect_vars(s, vars);
                    }
                }
                PatternStep::Repeat(inner, _) => collect_vars(inner, vars),
                PatternStep::Weighted(inner, _) => collect_vars(inner, vars),
                PatternStep::Alternation(steps) => {
                    for s in steps {
                        collect_vars(s, vars);
                    }
                }
                _ => {}
            }
        }
        let mut vars = Vec::new();
        for step in &self.steps {
            collect_vars(step, &mut vars);
        }
        vars
    }

    /// Resolve variables in this pattern using a lookup function.
    /// The lookup function takes a variable name and returns the resolved PatternStep(s).
    /// Returns Err if any variable cannot be resolved.
    pub fn resolve_variables_with<F>(&self, lookup: F) -> Result<Pattern>
    where
        F: Fn(&str) -> Option<Vec<PatternStep>> + Clone,
    {
        fn resolve_step<F>(step: &PatternStep, lookup: &F) -> Result<Vec<PatternStep>>
        where
            F: Fn(&str) -> Option<Vec<PatternStep>>,
        {
            match step {
                PatternStep::Variable(name) => {
                    lookup(name).ok_or_else(|| anyhow!("Undefined variable '{}' in pattern", name))
                }
                PatternStep::Group(steps) => {
                    let mut resolved = Vec::new();
                    for s in steps {
                        resolved.extend(resolve_step(s, lookup)?);
                    }
                    Ok(vec![PatternStep::Group(resolved)])
                }
                PatternStep::Repeat(inner, count) => {
                    let resolved_inner = resolve_step(inner, lookup)?;
                    if resolved_inner.len() == 1 {
                        Ok(vec![PatternStep::Repeat(
                            Box::new(resolved_inner.into_iter().next().unwrap()),
                            *count,
                        )])
                    } else {
                        // If variable resolved to multiple steps, repeat the group
                        Ok(vec![PatternStep::Repeat(
                            Box::new(PatternStep::Group(resolved_inner)),
                            *count,
                        )])
                    }
                }
                PatternStep::Weighted(inner, weight) => {
                    let resolved_inner = resolve_step(inner, lookup)?;
                    if resolved_inner.len() == 1 {
                        Ok(vec![PatternStep::Weighted(
                            Box::new(resolved_inner.into_iter().next().unwrap()),
                            *weight,
                        )])
                    } else {
                        // If variable resolved to multiple steps, weight the group
                        Ok(vec![PatternStep::Weighted(
                            Box::new(PatternStep::Group(resolved_inner)),
                            *weight,
                        )])
                    }
                }
                PatternStep::Alternation(steps) => {
                    let mut resolved = Vec::new();
                    for s in steps {
                        resolved.extend(resolve_step(s, lookup)?);
                    }
                    Ok(vec![PatternStep::Alternation(resolved)])
                }
                // Non-variable steps pass through unchanged
                other => Ok(vec![other.clone()]),
            }
        }

        let mut resolved_steps = Vec::new();
        for step in &self.steps {
            resolved_steps.extend(resolve_step(step, &lookup)?);
        }

        Ok(Pattern {
            steps: resolved_steps,
            beats_per_cycle: self.beats_per_cycle,
            envelope: self.envelope,
            waveform: self.waveform.clone(),
            pan: self.pan,
        })
    }

    // ========================================================================
    // Voice Leading & Analysis Methods (from Progression)
    // ========================================================================

    /// Check if this pattern contains only chords (no rests, groups, or single notes)
    pub fn is_chord_pattern(&self) -> bool {
        self.steps.iter().all(|step| match step {
            PatternStep::Chord(_) => true,
            PatternStep::Note(_) => true, // Single notes can be treated as chords
            PatternStep::Repeat(inner, _) => {
                matches!(**inner, PatternStep::Chord(_) | PatternStep::Note(_))
            }
            _ => false,
        })
    }

    /// Extract chords from pattern, expanding repeats.
    /// Returns None if pattern contains rests or groups.
    pub fn as_chords(&self) -> Option<Vec<Chord>> {
        let mut chords = Vec::new();

        for step in &self.steps {
            match step {
                PatternStep::Chord(chord) => chords.push(chord.clone()),
                PatternStep::Note(note) => {
                    chords.push(Chord::from_notes(vec![*note]));
                }
                PatternStep::Repeat(inner, count) => match inner.as_ref() {
                    PatternStep::Chord(chord) => {
                        for _ in 0..*count {
                            chords.push(chord.clone());
                        }
                    }
                    PatternStep::Note(note) => {
                        for _ in 0..*count {
                            chords.push(Chord::from_notes(vec![*note]));
                        }
                    }
                    _ => return None,
                },
                _ => return None,
            }
        }

        if chords.is_empty() {
            None
        } else {
            Some(chords)
        }
    }

    /// Create a pattern from a vector of chords
    pub fn from_chords(chords: Vec<Chord>) -> Self {
        let step_count = chords.len() as i64;
        let steps: Vec<PatternStep> = chords.into_iter().map(PatternStep::Chord).collect();

        Pattern {
            steps,
            beats_per_cycle: beats(step_count),
            envelope: Some((0.01, 0.1, 0.7, 0.3)),
            waveform: None,
            pan: None,
        }
    }

    /// Apply a function to all chords in the pattern
    pub fn map_chords<F>(mut self, f: F) -> Self
    where
        F: Fn(Chord) -> Chord + Clone,
    {
        self.steps = self
            .steps
            .into_iter()
            .map(|step| match step {
                PatternStep::Chord(chord) => PatternStep::Chord(f(chord)),
                PatternStep::Note(note) => {
                    let chord = Chord::from_notes(vec![note]);
                    PatternStep::Chord(f(chord))
                }
                PatternStep::Repeat(inner, count) => {
                    let mapped_inner = match *inner {
                        PatternStep::Chord(chord) => PatternStep::Chord(f(chord)),
                        PatternStep::Note(note) => {
                            let chord = Chord::from_notes(vec![note]);
                            PatternStep::Chord(f(chord))
                        }
                        other => other,
                    };
                    PatternStep::Repeat(Box::new(mapped_inner), count)
                }
                other => other,
            })
            .collect();
        self
    }

    /// Optimize voice leading for this pattern.
    /// Only works on chord-only patterns.
    pub fn optimize_voice_leading(self) -> Self {
        use crate::types::voice_leading;

        if let Some(chords) = self.as_chords() {
            let optimized = voice_leading::optimize_chord_sequence(chords);
            let mut result = Pattern::from_chords(optimized);
            result.beats_per_cycle = self.beats_per_cycle;
            result.envelope = self.envelope;
            result.waveform = self.waveform;
            result
        } else {
            println!("Cannot optimize voice leading: pattern contains rests or groups");
            self
        }
    }

    /// Analyze voice leading between adjacent chords
    pub fn analyze_voice_leading(&self) -> Vec<crate::types::voice_leading::VoiceLeading> {
        use crate::types::voice_leading;

        if let Some(chords) = self.as_chords() {
            voice_leading::analyze_chord_sequence(&chords)
        } else {
            Vec::new()
        }
    }

    /// Get average voice leading quality score (lower is better)
    pub fn average_voice_leading_quality(&self) -> f32 {
        use crate::types::voice_leading;

        if let Some(chords) = self.as_chords() {
            voice_leading::average_quality(&chords)
        } else {
            0.0
        }
    }

    /// Check if this pattern has good voice leading
    pub fn has_good_voice_leading(&self) -> bool {
        use crate::types::voice_leading;

        if let Some(chords) = self.as_chords() {
            voice_leading::has_good_voice_leading(&chords)
        } else {
            false
        }
    }

    /// Get detailed voice leading analysis
    pub fn detailed_voice_leading_analysis(
        &self,
    ) -> Vec<crate::types::voice_leading::VoiceLeadingAnalysis> {
        use crate::types::voice_leading;

        if let Some(chords) = self.as_chords() {
            voice_leading::detailed_analysis(&chords)
        } else {
            Vec::new()
        }
    }

    /// Get a comprehensive voice leading report
    pub fn voice_leading_report(&self) -> String {
        use crate::types::voice_leading::VoiceLeadingViolation;

        let chords = match self.as_chords() {
            Some(c) => c,
            None => {
                return "Pattern contains rests or groups - voice leading analysis not available"
                    .to_string();
            }
        };

        if chords.len() < 2 {
            return "Pattern too short for voice leading analysis".to_string();
        }

        let mut report = String::new();
        let analysis = self.detailed_voice_leading_analysis();
        let avg_quality = self.average_voice_leading_quality();
        let has_good_vl = self.has_good_voice_leading();

        report.push_str("=== Voice Leading Report ===\n");
        report.push_str(&format!("Pattern: {}\n\n", self));

        report.push_str("Transitions:\n");
        for item in &analysis {
            report.push_str(&format!("  {}\n", item));

            if !item.voice_leading.violations.is_empty() {
                for violation in &item.voice_leading.violations {
                    let violation_desc = match violation {
                        VoiceLeadingViolation::ParallelFifths { voice1, voice2 } => {
                            format!("    ‖5: voices {} and {}", voice1, voice2)
                        }
                        VoiceLeadingViolation::ParallelOctaves { voice1, voice2 } => {
                            format!("    ‖8: voices {} and {}", voice1, voice2)
                        }
                        VoiceLeadingViolation::LargeLeap { voice, semitones } => {
                            format!("    Large leap: voice {} ({} semitones)", voice, semitones)
                        }
                        _ => format!("    Other violation: {:?}", violation),
                    };
                    report.push_str(&format!("{}\n", violation_desc));
                }
            }
        }

        report.push_str("\nSummary:\n");
        report.push_str(&format!("  Average quality score: {:.1}\n", avg_quality));
        report.push_str(&format!(
            "  Overall assessment: {}\n",
            if has_good_vl {
                "✓ Good voice leading"
            } else {
                "⚠ Needs improvement"
            }
        ));

        if !has_good_vl {
            report.push_str("\nSuggestions:\n");
            report.push_str("  - Try running smooth_voice_leading() to optimize inversions\n");
            report.push_str("  - Look for common tones between chords\n");
            report.push_str("  - Minimize large leaps in individual voices\n");
        }

        report
    }

    /// Get the key signature that best fits this pattern
    pub fn analyze_key(&self) -> Option<Note> {
        let chords = self.as_chords()?;

        if chords.is_empty() {
            return None;
        }

        let mut root_counts = std::collections::HashMap::new();

        for chord in &chords {
            if let Some(root) = chord.root() {
                *root_counts.entry(root).or_insert(0) += 1;
            }
        }

        root_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(note, _)| note)
    }

    /// Get all unique notes used in this pattern
    pub fn get_all_notes(&self) -> Vec<Note> {
        let mut all_notes = std::collections::BTreeSet::new();

        fn collect_notes(step: &PatternStep, notes: &mut std::collections::BTreeSet<Note>) {
            match step {
                PatternStep::Note(n) => {
                    notes.insert(*n);
                }
                PatternStep::Chord(c) => {
                    for note in c.notes() {
                        notes.insert(*note);
                    }
                }
                PatternStep::Group(steps) => {
                    for s in steps {
                        collect_notes(s, notes);
                    }
                }
                PatternStep::Repeat(inner, _) => {
                    collect_notes(inner, notes);
                }
                PatternStep::Rest => {}
                PatternStep::Variable(_) => {} // Variables don't contribute notes until resolved
                PatternStep::Drum(_) => {}     // Drums don't contribute melodic notes
                PatternStep::Weighted(inner, _) => collect_notes(inner, notes), // Delegate to inner
                PatternStep::Alternation(steps) => {
                    for s in steps {
                        collect_notes(s, notes);
                    }
                }
            }
        }

        for step in &self.steps {
            collect_notes(step, &mut all_notes);
        }

        all_notes.into_iter().collect()
    }

    /// Get the number of steps in this pattern (not counting expanded repeats)
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if this pattern has no steps
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Reverse the order of steps in the pattern (retrograde)
    pub fn retrograde(mut self) -> Self {
        self.steps.reverse();
        self
    }

    /// Rotate steps by n positions
    /// Positive n rotates right (last element moves to front)
    /// Negative n rotates left (first element moves to end)
    pub fn rotate(mut self, n: i32) -> Self {
        if self.steps.is_empty() {
            return self;
        }
        let len = self.steps.len() as i32;
        // Normalize n to positive rotation amount
        let n = ((n % len) + len) % len;
        if n == 0 {
            return self;
        }
        let n = n as usize;
        // Rotate right by n: take last n elements and move to front
        let mut rotated = self.steps.split_off(self.steps.len() - n);
        rotated.append(&mut self.steps);
        self.steps = rotated;
        self
    }

    /// Take the first n steps of the pattern
    pub fn take(mut self, n: usize) -> Self {
        self.steps.truncate(n);
        self
    }

    /// Drop the first n steps of the pattern
    pub fn drop_steps(mut self, n: usize) -> Self {
        if n >= self.steps.len() {
            self.steps.clear();
        } else {
            self.steps = self.steps.split_off(n);
        }
        self
    }

    /// Create a palindrome: pattern followed by its reverse
    pub fn palindrome(mut self) -> Self {
        let reversed: Vec<PatternStep> = self.steps.iter().rev().cloned().collect();
        self.steps.extend(reversed);
        // Double the cycle length to accommodate the palindrome
        self.beats_per_cycle = self.beats_per_cycle * 2;
        self
    }

    /// Repeat each step n times
    pub fn stutter(mut self, n: usize) -> Self {
        if n <= 1 {
            return self;
        }
        let mut new_steps = Vec::with_capacity(self.steps.len() * n);
        for step in self.steps {
            for _ in 0..n {
                new_steps.push(step.clone());
            }
        }
        self.steps = new_steps;
        self
    }

    /// Concatenate another pattern onto this one
    pub fn concat(mut self, other: Pattern) -> Self {
        self.steps.extend(other.steps);
        self.beats_per_cycle += other.beats_per_cycle;
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

        // Check if pattern has actual content or only variables
        let has_pattern_content = steps.iter().any(|step| has_non_variable_content(step));

        if !has_pattern_content && !steps.is_empty() {
            // Pattern has ONLY variable references
            // Single-word patterns like "pluck" or "rev" should be treated as strings
            // Multi-word patterns like "cmaj fmaj" should be valid (pattern of variables)
            if steps.len() == 1 {
                return Err(anyhow!(
                    "Single word '{}' is not a valid pattern - no notes, rests, or chords found",
                    notation
                ));
            }
            // Multi-word variable-only patterns are allowed - they'll be resolved at runtime
        }

        Ok(Pattern::with_steps(steps))
    }
}

// Arithmetic operations for transposition
impl std::ops::Add<i8> for Pattern {
    type Output = Pattern;

    fn add(self, semitones: i8) -> Self::Output {
        self.transpose(semitones)
    }
}

impl std::ops::Sub<i8> for Pattern {
    type Output = Pattern;

    fn sub(self, semitones: i8) -> Self::Output {
        self.transpose(-semitones)
    }
}

// Index access to steps
impl std::ops::Index<usize> for Pattern {
    type Output = PatternStep;

    fn index(&self, index: usize) -> &Self::Output {
        &self.steps[index]
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
// EveryPattern - TidalCycles-style cycle-based pattern alternation
// ============================================================================

/// A pattern combinator that applies a transformation every N cycles.
/// This is the TidalCycles-style approach where the pattern itself
/// tracks which variant to use based on cycle position.
///
/// Unlike lazy evaluation, both patterns are pre-computed at creation time,
/// making the runtime selection fast and predictable.
#[derive(Clone, Debug, PartialEq)]
pub struct EveryPattern {
    /// How often to apply the transformation (every N cycles)
    pub interval: usize,
    /// The base (untransformed) pattern
    pub base: Pattern,
    /// The transformed pattern (pre-computed at creation time)
    pub transformed: Pattern,
}

impl EveryPattern {
    /// Create a new EveryPattern combinator
    ///
    /// # Arguments
    /// * `interval` - Apply the transformation every N cycles (1 = every cycle, 2 = every other cycle)
    /// * `base` - The original, untransformed pattern
    /// * `transformed` - The pattern with the transformation applied
    pub fn new(interval: usize, base: Pattern, transformed: Pattern) -> Self {
        Self {
            interval: interval.max(1), // Ensure interval is at least 1
            base,
            transformed,
        }
    }

    /// Get the appropriate pattern for the given absolute cycle number.
    ///
    /// For `every(N, transform, pattern)`:
    /// - Transform is applied every Nth cycle, starting from cycle N-1
    /// - `every(2, rev, p)`: base on 0, transformed on 1, base on 2, transformed on 3...
    /// - `every(3, rev, p)`: base on 0, 1, transformed on 2, base on 3, 4, transformed on 5...
    ///
    /// # Arguments
    /// * `cycle` - The current cycle number (0-indexed)
    ///
    /// # Returns
    /// A reference to either the transformed or base pattern
    pub fn get_pattern_for_cycle(&self, cycle: usize) -> &Pattern {
        // Transform on cycles where (cycle + 1) is divisible by interval
        // This gives: for interval 2, transform on cycles 1, 3, 5, 7...
        // For interval 3, transform on cycles 2, 5, 8...
        if (cycle + 1) % self.interval == 0 {
            &self.transformed
        } else {
            &self.base
        }
    }

    /// Get a clone of the pattern for the given cycle
    pub fn pattern_for_cycle(&self, cycle: usize) -> Pattern {
        self.get_pattern_for_cycle(cycle).clone()
    }

    /// Get the interval (how often the transformation is applied)
    pub fn interval(&self) -> usize {
        self.interval
    }
}

impl fmt::Display for EveryPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "every({}, transform, {})", self.interval, self.base)
    }
}

// ============================================================================
// Mini-notation parser
// ============================================================================

/// Check if a pattern step contains actual pattern content (not just variable references)
fn has_non_variable_content(step: &PatternStep) -> bool {
    match step {
        PatternStep::Note(_) | PatternStep::Chord(_) | PatternStep::Rest | PatternStep::Drum(_) => {
            true
        }
        PatternStep::Group(steps) => steps.iter().any(has_non_variable_content),
        PatternStep::Repeat(inner, _) => has_non_variable_content(inner),
        PatternStep::Weighted(inner, _) => has_non_variable_content(inner),
        PatternStep::Alternation(steps) => steps.iter().any(has_non_variable_content),
        PatternStep::Variable(_) => false,
    }
}

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
                let step = maybe_parse_weight_and_repeat(&mut chars, PatternStep::Rest)?;
                steps.push(step);
            }
            // Alternation (slow): <C D E> plays one element per cycle
            '<' => {
                chars.next(); // consume '<'
                let alt_content = take_until_angle_bracket(&mut chars)?;
                let inner_steps = parse_steps(&alt_content)?;
                if inner_steps.is_empty() {
                    return Err(anyhow!("Alternation <> cannot be empty"));
                }
                let step = maybe_parse_weight_and_repeat(
                    &mut chars,
                    PatternStep::Alternation(inner_steps),
                )?;
                steps.push(step);
            }
            // Group start
            '[' => {
                chars.next(); // consume '['
                let group_content = take_until_bracket(&mut chars)?;

                // Check if it's a nested group first (starts with '[' after whitespace)
                // This handles [[Bb4,D5,F5] [F4,A4,C5]] as a group containing chords
                let trimmed = group_content.trim_start();
                if trimmed.starts_with('[') {
                    // It's a nested group - parse recursively
                    let inner_steps = parse_steps(&group_content)?;
                    let step =
                        maybe_parse_weight_and_repeat(&mut chars, PatternStep::Group(inner_steps))?;
                    steps.push(step);
                } else if group_content.contains(',') {
                    // It's a chord - parse comma-separated notes
                    let note_strs: Vec<&str> = group_content.split(',').map(|s| s.trim()).collect();
                    let chord = Chord::from_note_strings(note_strs)?;
                    let step =
                        maybe_parse_weight_and_repeat(&mut chars, PatternStep::Chord(chord))?;
                    steps.push(step);
                } else {
                    // It's a group
                    let inner_steps = parse_steps(&group_content)?;
                    let step =
                        maybe_parse_weight_and_repeat(&mut chars, PatternStep::Group(inner_steps))?;
                    steps.push(step);
                }
            }
            // Note (uppercase A-G) or identifier/variable (starts with letter)
            'A'..='G' => {
                let token = take_note_or_identifier(&mut chars);
                // Uppercase start means it's likely a note - try to parse
                let step = match token.parse::<Note>() {
                    Ok(note) => PatternStep::Note(note),
                    Err(_) => {
                        // Not a valid note, treat as variable
                        PatternStep::Variable(token)
                    }
                };
                let step = maybe_parse_weight_and_repeat(&mut chars, step)?;
                steps.push(step);
            }
            // Lowercase letter - could be a flat note (a-g), a drum, or a variable
            'a'..='g' => {
                let token = take_note_or_identifier(&mut chars);
                // Check if it looks like a note (single letter + optional accidental + optional octave)
                // or a drum name, or an identifier
                let step = if let Ok(note) = token.parse::<Note>() {
                    PatternStep::Note(note)
                } else if let Some(drum) = DrumSound::from_str(&token) {
                    PatternStep::Drum(drum)
                } else {
                    // Not a valid note or drum, treat as variable
                    PatternStep::Variable(token)
                };
                let step = maybe_parse_weight_and_repeat(&mut chars, step)?;
                steps.push(step);
            }
            // Identifier starting with h-z (could be drum like 'kick', 'hh', or variable)
            'h'..='z' | 'H'..='Z' => {
                let ident = take_identifier(&mut chars);
                // Check if it's a drum name first
                let step = if let Some(drum) = DrumSound::from_str(&ident) {
                    PatternStep::Drum(drum)
                } else {
                    PatternStep::Variable(ident)
                };
                let step = maybe_parse_weight_and_repeat(&mut chars, step)?;
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

/// Take content until matching '>', handling nested angle brackets
fn take_until_angle_bracket(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String> {
    let mut content = String::new();
    let mut depth = 1;

    while let Some(c) = chars.next() {
        match c {
            '<' => {
                depth += 1;
                content.push(c);
            }
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(content);
                }
                content.push(c);
            }
            _ => content.push(c),
        }
    }

    Err(anyhow!("Unclosed angle bracket in pattern"))
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

/// Take a note token OR a longer identifier (for variable names)
/// Keeps case as-is for variable names, but uppercases for notes
fn take_note_or_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut token = String::new();

    // First char is the letter
    if let Some(c) = chars.next() {
        token.push(c);
    }

    // Check for accidental (only if the first char is A-G)
    if token.len() == 1 {
        let first_upper = token.chars().next().unwrap().to_ascii_uppercase();
        if ('A'..='G').contains(&first_upper) {
            if let Some(&c) = chars.peek() {
                if c == '#' {
                    // Sharp - consume and treat as note
                    token.push(chars.next().unwrap());
                    // Rest must be octave digits
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '-' {
                            token.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    // Uppercase for note parsing
                    return token
                        .chars()
                        .next()
                        .unwrap()
                        .to_ascii_uppercase()
                        .to_string()
                        + &token[1..];
                }
            }
        }
    }

    // Continue taking alphanumeric chars (for identifiers like "cmaj", "bass")
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            token.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    token
}

/// Take an identifier (for variable names starting with h-z)
fn take_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut ident = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    ident
}

/// Parse optional @N weight and *N repetition suffixes
/// Weight is parsed first, then repeat (e.g., C@2*3 means weighted C repeated 3 times)
fn maybe_parse_weight_and_repeat(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    step: PatternStep,
) -> Result<PatternStep> {
    // Check for @N weight first
    let step = if chars.peek() == Some(&'@') {
        chars.next(); // consume '@'
        let mut weight_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                weight_str.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        if weight_str.is_empty() {
            return Err(anyhow!("Expected number after '@'"));
        }
        let weight: usize = weight_str.parse()?;
        if weight == 0 {
            return Err(anyhow!("Weight @0 is not allowed (use _ for rest)"));
        }
        PatternStep::Weighted(Box::new(step), weight)
    } else {
        step
    };

    // Then check for *N repeat
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
        assert_eq!(p.beats_per_cycle, beats(4));
        assert_eq!(p.step_beats(), beats(1)); // 4 steps, 4 beats = 1 beat each
    }

    #[test]
    fn test_fast() {
        let p = Pattern::parse("C E").unwrap().fast(2);
        assert_eq!(p.beats_per_cycle, beats(2)); // Now plays in 2 beats
    }

    #[test]
    fn test_slow() {
        let p = Pattern::parse("C E").unwrap().slow(2);
        assert_eq!(p.beats_per_cycle, beats(8)); // Now takes 8 beats
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

        let c_maj = Chord::from_note_strings(vec!["C4", "E4", "G4"]).unwrap();
        let f_maj = Chord::from_note_strings(vec!["F4", "A4", "C4"]).unwrap();
        let pattern = Pattern::from_chords(vec![c_maj.clone(), f_maj.clone()]);

        // Optimize voice leading
        let optimized = pattern.optimize_voice_leading();

        // Get the playback events
        let events = optimized.to_events();
        assert_eq!(events.len(), 2, "Should have 2 chord events");

        // Get the chord at index 1 for verification
        let chords = optimized.as_chords().expect("Should be chord-only pattern");
        let second_chord = &chords[1];
        let notes = second_chord.notes_vec();

        // Calculate expected frequencies
        let expected_freqs: Vec<f32> = notes.iter().map(|n| n.frequency()).collect();

        // Check the frequencies of the second chord
        let (freqs, _duration, _is_rest) = &events[1];
        assert_eq!(freqs.len(), 3, "Second chord should have 3 frequencies");

        // Frequencies should match the optimized chord order
        let tolerance = 0.1; // Small tolerance for float comparison
        for (i, (actual, expected)) in freqs.iter().zip(expected_freqs.iter()).enumerate() {
            assert!(
                (actual - expected).abs() < tolerance,
                "Frequency {} should be ~{}, got {}",
                i,
                expected,
                actual
            );
        }
    }

    #[test]
    fn test_parse_pattern_with_variables() {
        // "C cmaj E" should parse with a Variable step in the middle
        let p = Pattern::parse("C cmaj E").unwrap();
        assert_eq!(p.steps.len(), 3);
        assert!(matches!(&p.steps[0], PatternStep::Note(_)));
        assert!(matches!(&p.steps[1], PatternStep::Variable(name) if name == "cmaj"));
        assert!(matches!(&p.steps[2], PatternStep::Note(_)));
        assert!(p.has_variables());
    }

    #[test]
    fn test_parse_single_variable_fails() {
        // Single-word variable patterns should fail (treated as plain string)
        // This prevents "pluck" or "rev" from being parsed as patterns
        assert!(Pattern::parse("pluck").is_err());
        assert!(Pattern::parse("rev").is_err());
        assert!(Pattern::parse("cmaj").is_err());
    }

    #[test]
    fn test_parse_multi_variable_pattern() {
        // Multi-word variable-only patterns should succeed
        let p = Pattern::parse("cmaj fmaj").unwrap();
        assert_eq!(p.steps.len(), 2);
        assert!(matches!(&p.steps[0], PatternStep::Variable(name) if name == "cmaj"));
        assert!(matches!(&p.steps[1], PatternStep::Variable(name) if name == "fmaj"));
        assert!(p.has_variables());
    }

    #[test]
    fn test_resolve_variables() {
        // "C myvar E" with myvar=[D, F]
        let p = Pattern::parse("C myvar E").unwrap();
        assert!(p.has_variables());

        let resolved = p
            .resolve_variables_with(|name| {
                if name == "myvar" {
                    // Resolve to [D, F] chord
                    let chord = Chord::from_note_strings(vec!["D", "F"]).unwrap();
                    Some(vec![PatternStep::Chord(chord)])
                } else {
                    None
                }
            })
            .unwrap();

        assert!(!resolved.has_variables());
        assert_eq!(resolved.steps.len(), 3);
        // First should be C
        assert!(matches!(&resolved.steps[0], PatternStep::Note(_)));
        // Second should now be the chord
        assert!(matches!(&resolved.steps[1], PatternStep::Chord(_)));
        // Third should be E
        assert!(matches!(&resolved.steps[2], PatternStep::Note(_)));
    }

    #[test]
    fn test_resolve_undefined_variable_fails() {
        let p = Pattern::parse("C undefined E").unwrap();
        let result = p.resolve_variables_with(|_| None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_variable_names() {
        let p = Pattern::parse("C foo E bar G").unwrap();
        let vars = p.get_variable_names();
        assert_eq!(vars, vec!["foo", "bar"]);
    }

    // Tests for new pattern manipulation methods
    #[test]
    fn test_rotate_right() {
        let p = Pattern::parse("C D E F").unwrap().rotate(1);
        // Should rotate right: F C D E
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 5), // F
            _ => panic!("Expected Note F"),
        }
        match &p.steps[1] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }
    }

    #[test]
    fn test_rotate_left() {
        let p = Pattern::parse("C D E F").unwrap().rotate(-1);
        // Should rotate left: D E F C
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
            _ => panic!("Expected Note D"),
        }
        match &p.steps[3] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }
    }

    #[test]
    fn test_take() {
        let p = Pattern::parse("C D E F").unwrap().take(2);
        assert_eq!(p.steps.len(), 2);
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }
        match &p.steps[1] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
            _ => panic!("Expected Note D"),
        }
    }

    #[test]
    fn test_drop_steps() {
        let p = Pattern::parse("C D E F").unwrap().drop_steps(2);
        assert_eq!(p.steps.len(), 2);
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
            _ => panic!("Expected Note E"),
        }
        match &p.steps[1] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 5), // F
            _ => panic!("Expected Note F"),
        }
    }

    #[test]
    fn test_palindrome() {
        let p = Pattern::parse("C D E").unwrap().palindrome();
        assert_eq!(p.steps.len(), 6); // C D E E D C
        assert_eq!(p.beats_per_cycle, beats(8)); // Doubled from 4
        match &p.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }
        // Last three: E D C
        match &p.steps[3] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
            _ => panic!("Expected Note E"),
        }
        match &p.steps[5] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }
    }

    #[test]
    fn test_stutter() {
        let p = Pattern::parse("C D").unwrap().stutter(3);
        assert_eq!(p.steps.len(), 6); // C C C D D D
        for i in 0..3 {
            match &p.steps[i] {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
                _ => panic!("Expected Note C"),
            }
        }
        for i in 3..6 {
            match &p.steps[i] {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
                _ => panic!("Expected Note D"),
            }
        }
    }

    #[test]
    fn test_concat() {
        let p1 = Pattern::parse("C D").unwrap();
        let p2 = Pattern::parse("E F").unwrap();
        let combined = p1.concat(p2);
        assert_eq!(combined.steps.len(), 4);
        assert_eq!(combined.beats_per_cycle, beats(8)); // 4 + 4
        match &combined.steps[2] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
            _ => panic!("Expected Note E"),
        }
    }

    // ========================================================================
    // Alternation Tests
    // ========================================================================

    #[test]
    fn test_parse_alternation_simple() {
        // <C D E> should parse to Alternation with 3 notes
        let p = Pattern::parse("<C D E>").unwrap();
        assert_eq!(p.steps.len(), 1);
        match &p.steps[0] {
            PatternStep::Alternation(steps) => {
                assert_eq!(steps.len(), 3);
                // First should be C
                match &steps[0] {
                    PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
                    _ => panic!("Expected Note C"),
                }
                // Second should be D
                match &steps[1] {
                    PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
                    _ => panic!("Expected Note D"),
                }
                // Third should be E
                match &steps[2] {
                    PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
                    _ => panic!("Expected Note E"),
                }
            }
            _ => panic!("Expected Alternation"),
        }
    }

    #[test]
    fn test_parse_alternation_mixed() {
        // C <D E> F should parse to Note, Alternation, Note
        let p = Pattern::parse("C <D E> F").unwrap();
        assert_eq!(p.steps.len(), 3);
        assert!(matches!(&p.steps[0], PatternStep::Note(_)));
        assert!(matches!(&p.steps[1], PatternStep::Alternation(_)));
        assert!(matches!(&p.steps[2], PatternStep::Note(_)));
    }

    #[test]
    fn test_parse_alternation_with_repeat() {
        // <C D>*2 should parse to alternation with repeat modifier
        let p = Pattern::parse("<C D>*2").unwrap();
        assert_eq!(p.steps.len(), 1);
        match &p.steps[0] {
            PatternStep::Repeat(inner, count) => {
                assert_eq!(*count, 2);
                assert!(matches!(**inner, PatternStep::Alternation(_)));
            }
            _ => panic!("Expected Repeat"),
        }
    }

    #[test]
    fn test_alternation_display() {
        let p = Pattern::parse("<C D E>").unwrap();
        let display = format!("{}", p);
        assert!(display.contains("<C"));
        assert!(display.contains(">"));
    }

    #[test]
    fn test_alternation_cycle_selection() {
        // Test that to_step_info_for_cycle returns different elements for different cycles
        let p = Pattern::parse("<C D E>").unwrap();
        let alt_step = &p.steps[0];

        // Cycle 0 should return C (pitch_class 0)
        let info_0 = alt_step.to_step_info_for_cycle(0);
        assert_eq!(info_0.len(), 1);
        assert_eq!(info_0[0].0[0].pitch_class, 0); // C

        // Cycle 1 should return D (pitch_class 2)
        let info_1 = alt_step.to_step_info_for_cycle(1);
        assert_eq!(info_1.len(), 1);
        assert_eq!(info_1[0].0[0].pitch_class, 2); // D

        // Cycle 2 should return E (pitch_class 4)
        let info_2 = alt_step.to_step_info_for_cycle(2);
        assert_eq!(info_2.len(), 1);
        assert_eq!(info_2[0].0[0].pitch_class, 4); // E

        // Cycle 3 should wrap back to C
        let info_3 = alt_step.to_step_info_for_cycle(3);
        assert_eq!(info_3[0].0[0].pitch_class, 0); // C
    }

    #[test]
    fn test_alternation_rich_events_for_cycle() {
        // Test that to_rich_events_for_cycle produces correct events
        let p = Pattern::parse("<C D E>").unwrap();

        // Cycle 0: should have C
        let events_0 = p.to_rich_events_for_cycle(0);
        assert_eq!(events_0.len(), 1);
        assert_eq!(events_0[0].notes[0].pitch_class, 0); // C

        // Cycle 1: should have D
        let events_1 = p.to_rich_events_for_cycle(1);
        assert_eq!(events_1[0].notes[0].pitch_class, 2); // D
    }

    #[test]
    fn test_alternation_empty_fails() {
        let result = Pattern::parse("<>");
        assert!(result.is_err());
    }

    #[test]
    fn test_alternation_unclosed_fails() {
        let result = Pattern::parse("<C D E");
        assert!(result.is_err());
    }

    // ========================================================================
    // EveryPattern Tests
    // ========================================================================

    #[test]
    fn test_every_pattern_creation() {
        let base = Pattern::parse("C D E F").unwrap();
        let transformed = base.clone().rev();
        let every = EveryPattern::new(2, base.clone(), transformed.clone());

        assert_eq!(every.interval, 2);
        assert_eq!(every.base.steps.len(), 4);
        assert_eq!(every.transformed.steps.len(), 4);

        // Verify base pattern is C D E F
        match &every.base.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }

        // Verify transformed pattern is F E D C (reversed)
        match &every.transformed.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 5), // F
            _ => panic!("Expected Note F"),
        }
    }

    #[test]
    fn test_every_pattern_cycle_selection_interval_2() {
        let base = Pattern::parse("C D E F").unwrap();
        let transformed = base.clone().rev(); // F E D C
        let every = EveryPattern::new(2, base.clone(), transformed.clone());

        // New behavior: every(2) = base, transformed, base, transformed...
        // Cycle 0: base ((0+1) % 2 = 1, not 0)
        let pattern_at_0 = every.get_pattern_for_cycle(0);
        match &pattern_at_0.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0, "Cycle 0 should be base (C)"),
            _ => panic!("Expected Note"),
        }

        // Cycle 1: transformed ((1+1) % 2 = 0)
        let pattern_at_1 = every.get_pattern_for_cycle(1);
        match &pattern_at_1.steps[0] {
            PatternStep::Note(n) => {
                assert_eq!(n.pitch_class(), 5, "Cycle 1 should be transformed (F)")
            }
            _ => panic!("Expected Note"),
        }

        // Cycle 2: base ((2+1) % 2 = 1, not 0)
        let pattern_at_2 = every.get_pattern_for_cycle(2);
        match &pattern_at_2.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0, "Cycle 2 should be base (C)"),
            _ => panic!("Expected Note"),
        }

        // Cycle 3: transformed ((3+1) % 2 = 0)
        let pattern_at_3 = every.get_pattern_for_cycle(3);
        match &pattern_at_3.steps[0] {
            PatternStep::Note(n) => {
                assert_eq!(n.pitch_class(), 5, "Cycle 3 should be transformed (F)")
            }
            _ => panic!("Expected Note"),
        }

        // Cycle 4: base ((4+1) % 2 = 1, not 0)
        let pattern_at_4 = every.get_pattern_for_cycle(4);
        match &pattern_at_4.steps[0] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0, "Cycle 4 should be base (C)"),
            _ => panic!("Expected Note"),
        }
    }

    #[test]
    fn test_every_pattern_cycle_selection_interval_3() {
        let base = Pattern::parse("C D E F").unwrap();
        let transformed = base.clone().rev();
        let every = EveryPattern::new(3, base.clone(), transformed.clone());

        // Interval 3: every(3) = base, base, transformed, base, base, transformed...
        // Cycle 0: base ((0+1) % 3 = 1, not 0)
        assert_eq!(every.get_pattern_for_cycle(0).steps[0], every.base.steps[0]);
        // Cycle 1: base ((1+1) % 3 = 2, not 0)
        assert_eq!(every.get_pattern_for_cycle(1).steps[0], every.base.steps[0]);
        // Cycle 2: transformed ((2+1) % 3 = 0)
        assert_eq!(
            every.get_pattern_for_cycle(2).steps[0],
            every.transformed.steps[0]
        );
        // Cycle 3: base ((3+1) % 3 = 1, not 0)
        assert_eq!(every.get_pattern_for_cycle(3).steps[0], every.base.steps[0]);
        // Cycle 4: base ((4+1) % 3 = 2, not 0)
        assert_eq!(every.get_pattern_for_cycle(4).steps[0], every.base.steps[0]);
        // Cycle 5: transformed ((5+1) % 3 = 0)
        assert_eq!(
            every.get_pattern_for_cycle(5).steps[0],
            every.transformed.steps[0]
        );
        // Cycle 6: base ((6+1) % 3 = 1, not 0)
        assert_eq!(every.get_pattern_for_cycle(6).steps[0], every.base.steps[0]);
    }

    #[test]
    fn test_every_pattern_interval_1_always_transformed() {
        let base = Pattern::parse("C D").unwrap();
        let transformed = base.clone().rev();
        let every = EveryPattern::new(1, base.clone(), transformed.clone());

        // Interval 1: every(1) = transform every cycle (all cycles have (cycle+1) % 1 == 0)
        for cycle in 0..10 {
            assert_eq!(
                every.get_pattern_for_cycle(cycle).steps[0],
                every.transformed.steps[0],
                "Interval 1 should return transformed at cycle {}",
                cycle
            );
        }
    }

    #[test]
    fn test_every_pattern_interval_0_becomes_1() {
        // Interval 0 should be clamped to 1 to avoid division by zero
        let base = Pattern::parse("C D").unwrap();
        let transformed = base.clone().rev();
        let every = EveryPattern::new(0, base.clone(), transformed.clone());

        assert_eq!(every.interval, 1, "Interval 0 should be clamped to 1");
    }

    #[test]
    fn test_every_pattern_display() {
        let base = Pattern::parse("C D").unwrap();
        let transformed = base.clone().rev();
        let every = EveryPattern::new(2, base, transformed);

        let display = format!("{}", every);
        assert!(display.contains("every(2"), "Display should show interval");
    }

    #[test]
    fn test_every_pattern_pattern_for_cycle_clone() {
        let base = Pattern::parse("C D E F").unwrap();
        let transformed = base.clone().rev();
        let every = EveryPattern::new(2, base.clone(), transformed.clone());

        // Test pattern_for_cycle (returns clone)
        // Cycle 0 returns base, cycle 1 returns transformed (new logic)
        let cloned_base = every.pattern_for_cycle(0);
        assert_eq!(cloned_base.steps.len(), 4);
        assert_eq!(cloned_base.steps[0], every.base.steps[0]);

        let cloned_transformed = every.pattern_for_cycle(1);
        assert_eq!(cloned_transformed.steps.len(), 4);
        assert_eq!(cloned_transformed.steps[0], every.transformed.steps[0]);
    }

    // ========================================================================
    // Weighted Steps Tests
    // ========================================================================

    #[test]
    fn test_weighted_parse_simple() {
        let p = Pattern::parse("C@2 D").unwrap();
        assert_eq!(p.steps.len(), 2);
        // First step should be Weighted(Note(C), 2)
        match &p.steps[0] {
            PatternStep::Weighted(inner, weight) => {
                assert_eq!(*weight, 2);
                assert!(matches!(**inner, PatternStep::Note(_)));
            }
            _ => panic!("Expected Weighted step"),
        }
        // Second step should be Note(D) with weight 1
        assert!(matches!(&p.steps[1], PatternStep::Note(_)));
        assert_eq!(p.steps[1].weight(), 1);
    }

    #[test]
    fn test_weighted_durations() {
        let p = Pattern::parse("C@2 D").unwrap();
        let events = p.to_rich_events();

        // Total weight = 2 + 1 = 3
        // C gets 2/3 of 4 beats = 8/3 beats
        // D gets 1/3 of 4 beats = 4/3 beats
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].duration, Ratio::new(8, 3));
        assert_eq!(events[1].duration, Ratio::new(4, 3));

        // Check start times
        assert_eq!(events[0].start_beat, Ratio::from_integer(0));
        assert_eq!(events[1].start_beat, Ratio::new(8, 3));
    }

    #[test]
    fn test_weighted_chord() {
        let p = Pattern::parse("[C,E]@3 G").unwrap();
        let events = p.to_rich_events();

        // Total weight = 3 + 1 = 4
        // Chord gets 3/4 of 4 beats = 3 beats
        // G gets 1/4 of 4 beats = 1 beat
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].duration, beats(3));
        assert_eq!(events[1].duration, beats(1));
    }

    #[test]
    fn test_weighted_rest() {
        let p = Pattern::parse("C@2 _@2 D").unwrap();
        let events = p.to_rich_events();

        // Total weight = 2 + 2 + 1 = 5
        // C gets 2/5 of 4 = 8/5 beats
        // Rest gets 2/5 of 4 = 8/5 beats
        // D gets 1/5 of 4 = 4/5 beats
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].duration, Ratio::new(8, 5));
        assert_eq!(events[1].duration, Ratio::new(8, 5));
        assert!(events[1].is_rest);
        assert_eq!(events[2].duration, Ratio::new(4, 5));
    }

    #[test]
    fn test_weighted_with_repeat() {
        // C@2*3 means: weight 2, repeated 3 times
        let p = Pattern::parse("C@2*3 D").unwrap();

        // Should have 2 steps: Repeat(Weighted(C, 2), 3) and Note(D)
        assert_eq!(p.steps.len(), 2);
        match &p.steps[0] {
            PatternStep::Repeat(inner, count) => {
                assert_eq!(*count, 3);
                match inner.as_ref() {
                    PatternStep::Weighted(note, weight) => {
                        assert_eq!(*weight, 2);
                        assert!(matches!(**note, PatternStep::Note(_)));
                    }
                    _ => panic!("Expected Weighted inside Repeat"),
                }
            }
            _ => panic!("Expected Repeat step"),
        }
    }

    #[test]
    fn test_weighted_display() {
        let p = Pattern::parse("C@2 D").unwrap();
        let display = format!("{}", p);
        assert!(
            display.contains("C@2"),
            "Display should show weight: got {}",
            display
        );
        assert!(
            display.contains("D"),
            "Display should show D: got {}",
            display
        );
    }

    #[test]
    fn test_weighted_transpose() {
        let p = Pattern::parse("C@2 D").unwrap();
        let transposed = p.transpose(2);

        // Weight should be preserved after transpose
        match &transposed.steps[0] {
            PatternStep::Weighted(inner, weight) => {
                assert_eq!(*weight, 2);
                // D (C + 2) should be pitch class 2
                match inner.as_ref() {
                    PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2),
                    _ => panic!("Expected Note inside Weighted"),
                }
            }
            _ => panic!("Expected Weighted step"),
        }
    }

    #[test]
    fn test_weighted_zero_error() {
        // @0 should be an error
        let result = Pattern::parse("C@0 D");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("@0"));
    }
}
