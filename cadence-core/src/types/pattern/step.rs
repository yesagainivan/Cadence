//! PatternStep enum - a single step in a pattern.

use super::euclidean::bjorklund;
use super::event::NoteInfo;
use crate::types::{Chord, DrumSound, Note};
use std::fmt;

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
    /// Euclidean rhythm: C(3,8) distributes 3 pulses evenly across 8 slots
    Euclidean(Box<PatternStep>, usize, usize), // (inner, pulses, steps)
    /// Polyrhythm: {C D E, F G} plays multiple patterns simultaneously,
    /// each at its own tempo (3-step pattern plays 3 notes/cycle, 2-step plays 2 notes/cycle)
    Polyrhythm(Vec<Vec<PatternStep>>), // Each inner Vec is a sub-pattern's steps
    /// Velocity modifier: C5(0.5) or C5(100) sets MIDI velocity (0-127)
    Velocity(Box<PatternStep>, u8),
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
            // Euclidean: expand using Bjorklund algorithm
            PatternStep::Euclidean(inner, pulses, steps) => {
                let rhythm = bjorklund(*pulses, *steps);
                let inner_freq = inner.to_frequencies();
                rhythm
                    .into_iter()
                    .map(|is_pulse| {
                        if is_pulse {
                            inner_freq.first().cloned().unwrap_or((vec![], true))
                        } else {
                            (vec![], true) // rest
                        }
                    })
                    .collect()
            }
            // Polyrhythm: merge frequencies from all sub-patterns
            PatternStep::Polyrhythm(sub_patterns) => {
                // For static evaluation, merge all first events from each sub-pattern
                let mut merged_freqs: Vec<f32> = Vec::new();
                for sub in sub_patterns {
                    for step in sub {
                        for (freqs, is_rest) in step.to_frequencies() {
                            if !is_rest {
                                merged_freqs.extend(freqs);
                            }
                        }
                    }
                }
                if merged_freqs.is_empty() {
                    vec![(vec![], true)]
                } else {
                    vec![(merged_freqs, false)]
                }
            }
            // Velocity: delegate to inner (velocity is handled in NoteInfo conversion)
            PatternStep::Velocity(inner, _) => inner.to_frequencies(),
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
                        velocity: 100,
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
            // Euclidean: expand using Bjorklund algorithm
            PatternStep::Euclidean(inner, pulses, steps) => {
                let rhythm = bjorklund(*pulses, *steps);
                let inner_info = inner.to_note_infos();
                rhythm
                    .into_iter()
                    .map(|is_pulse| {
                        if is_pulse {
                            inner_info.first().cloned().unwrap_or((vec![], true))
                        } else {
                            (vec![], true) // rest
                        }
                    })
                    .collect()
            }
            // Polyrhythm: merge note info from all sub-patterns
            PatternStep::Polyrhythm(sub_patterns) => {
                let mut merged_notes: Vec<NoteInfo> = Vec::new();
                for sub in sub_patterns {
                    for step in sub {
                        for (notes, is_rest) in step.to_note_infos() {
                            if !is_rest {
                                merged_notes.extend(notes);
                            }
                        }
                    }
                }
                if merged_notes.is_empty() {
                    vec![(vec![], true)]
                } else {
                    vec![(merged_notes, false)]
                }
            }
            // Velocity: apply velocity to all notes from inner step
            PatternStep::Velocity(inner, vel) => inner
                .to_note_infos()
                .into_iter()
                .map(|(notes, is_rest)| {
                    let notes_with_vel: Vec<NoteInfo> =
                        notes.into_iter().map(|n| n.with_velocity(*vel)).collect();
                    (notes_with_vel, is_rest)
                })
                .collect(),
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
            // Euclidean: expand using Bjorklund algorithm
            PatternStep::Euclidean(inner, pulses, steps) => {
                let rhythm = bjorklund(*pulses, *steps);
                let inner_info = inner.to_step_info();
                rhythm
                    .into_iter()
                    .map(|is_pulse| {
                        if is_pulse {
                            inner_info
                                .first()
                                .cloned()
                                .unwrap_or((vec![], vec![], true))
                        } else {
                            (vec![], vec![], true) // rest
                        }
                    })
                    .collect()
            }
            // Polyrhythm: merge step info from all sub-patterns
            PatternStep::Polyrhythm(sub_patterns) => {
                let mut merged_notes: Vec<NoteInfo> = Vec::new();
                let mut merged_drums: Vec<DrumSound> = Vec::new();
                for sub in sub_patterns {
                    for step in sub {
                        for (notes, drums, is_rest) in step.to_step_info() {
                            if !is_rest {
                                merged_notes.extend(notes);
                                merged_drums.extend(drums);
                            }
                        }
                    }
                }
                if merged_notes.is_empty() && merged_drums.is_empty() {
                    vec![(vec![], vec![], true)]
                } else {
                    vec![(merged_notes, merged_drums, false)]
                }
            }
            // Velocity: apply velocity to all notes from inner step
            PatternStep::Velocity(inner, vel) => inner
                .to_step_info()
                .into_iter()
                .map(|(notes, drums, is_rest)| {
                    let notes_with_vel: Vec<NoteInfo> =
                        notes.into_iter().map(|n| n.with_velocity(*vel)).collect();
                    (notes_with_vel, drums, is_rest)
                })
                .collect(),
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
            // Euclidean: expand using Bjorklund algorithm
            PatternStep::Euclidean(inner, pulses, steps) => {
                let rhythm = bjorklund(*pulses, *steps);
                let inner_info = inner.to_step_info_for_cycle(cycle);
                rhythm
                    .into_iter()
                    .map(|is_pulse| {
                        if is_pulse {
                            inner_info
                                .first()
                                .cloned()
                                .unwrap_or((vec![], vec![], true))
                        } else {
                            (vec![], vec![], true) // rest
                        }
                    })
                    .collect()
            }
            // Polyrhythm: merge step info from all sub-patterns, cycle-aware
            PatternStep::Polyrhythm(sub_patterns) => {
                let mut merged_notes: Vec<NoteInfo> = Vec::new();
                let mut merged_drums: Vec<DrumSound> = Vec::new();
                for sub in sub_patterns {
                    for step in sub {
                        for (notes, drums, is_rest) in step.to_step_info_for_cycle(cycle) {
                            if !is_rest {
                                merged_notes.extend(notes);
                                merged_drums.extend(drums);
                            }
                        }
                    }
                }
                if merged_notes.is_empty() && merged_drums.is_empty() {
                    vec![(vec![], vec![], true)]
                } else {
                    vec![(merged_notes, merged_drums, false)]
                }
            }
            // Velocity: apply velocity to all notes from inner step
            PatternStep::Velocity(inner, vel) => inner
                .to_step_info_for_cycle(cycle)
                .into_iter()
                .map(|(notes, drums, is_rest)| {
                    let notes_with_vel: Vec<NoteInfo> =
                        notes.into_iter().map(|n| n.with_velocity(*vel)).collect();
                    (notes_with_vel, drums, is_rest)
                })
                .collect(),
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
            PatternStep::Euclidean(inner, pulses, steps) => {
                PatternStep::Euclidean(Box::new(inner.transpose(semitones)), *pulses, *steps)
            }
            PatternStep::Polyrhythm(sub_patterns) => PatternStep::Polyrhythm(
                sub_patterns
                    .iter()
                    .map(|sub| sub.iter().map(|s| s.transpose(semitones)).collect())
                    .collect(),
            ),
            PatternStep::Velocity(inner, vel) => {
                PatternStep::Velocity(Box::new(inner.transpose(semitones)), *vel)
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
            PatternStep::Euclidean(inner, pulses, steps) => {
                write!(f, "{}({},{})", inner, pulses, steps)
            }
            PatternStep::Polyrhythm(sub_patterns) => {
                write!(f, "{{")?;
                for (i, sub) in sub_patterns.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    for (j, step) in sub.iter().enumerate() {
                        if j > 0 {
                            write!(f, " ")?;
                        }
                        write!(f, "{}", step)?;
                    }
                }
                write!(f, "}}")
            }
            PatternStep::Velocity(inner, vel) => {
                write!(f, "{}({})", inner, vel)
            }
        }
    }
}
