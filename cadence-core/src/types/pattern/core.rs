//! Core Pattern struct and implementation.

use super::event::PlaybackEvent;
use super::parser::{has_non_variable_content, parse_steps};
use super::step::PatternStep;
use crate::types::audio_config::Waveform;
use crate::types::time::{beats, to_f32, Time};
use crate::types::{Chord, Note};
use anyhow::{anyhow, Result};
use num_rational::Ratio;
use std::fmt;
use std::str::FromStr;

/// Merge events that start at the same beat into combined events.
/// This is essential for polyrhythm where multiple notes from different
/// sub-patterns may trigger simultaneously.
/// Also clips durations so events don't overlap the next event's start.
fn merge_concurrent_events(events: Vec<PlaybackEvent>) -> Vec<PlaybackEvent> {
    if events.is_empty() {
        return events;
    }

    // Events should already be sorted by start_beat
    let mut merged: Vec<PlaybackEvent> = Vec::new();

    for event in events {
        // Check if last event has the same start_beat
        if let Some(last) = merged.last_mut() {
            if last.start_beat == event.start_beat {
                // Merge notes and drums into the existing event
                last.notes.extend(event.notes);
                last.drums.extend(event.drums);
                // If either is not a rest, the merged event is not a rest
                last.is_rest = last.is_rest && event.is_rest;
                // Use shorter duration (for safety)
                if event.duration < last.duration {
                    last.duration = event.duration;
                }
                continue;
            }
        }
        // Different start_beat, add as new event
        merged.push(event);
    }

    // After merging, clip each event's duration so it doesn't extend past the next event's start
    // This is critical for the dispatcher to correctly transition between events
    for i in 0..merged.len().saturating_sub(1) {
        let next_start = merged[i + 1].start_beat;
        let current_end = merged[i].start_beat + merged[i].duration;
        if current_end > next_start {
            // Clip duration to end at next event's start
            merged[i].duration = next_start - merged[i].start_beat;
        }
    }

    merged
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

            // Special handling for Polyrhythm - each sub-pattern plays at its own tempo
            if let PatternStep::Polyrhythm(sub_patterns) = step {
                // Generate events for each sub-pattern independently
                // Each sub-pattern fits within step_duration but at its own rate
                for sub_steps in sub_patterns {
                    if sub_steps.is_empty() {
                        continue;
                    }
                    // Each sub-pattern's notes are evenly distributed within step_duration
                    let sub_event_duration = step_duration / sub_steps.len() as i64;
                    let mut sub_current_beat = current_beat;

                    for sub_step in sub_steps {
                        let step_info_list = sub_step.to_step_info();
                        let sub_count = step_info_list.len() as i64;
                        let event_duration = if sub_count > 0 {
                            sub_event_duration / sub_count
                        } else {
                            sub_event_duration
                        };

                        for (notes, drums, is_rest) in step_info_list {
                            events.push(PlaybackEvent {
                                notes,
                                drums,
                                start_beat: sub_current_beat,
                                duration: event_duration,
                                is_rest,
                            });
                            sub_current_beat = sub_current_beat + event_duration;
                        }
                    }
                }
                // Advance past the entire polyrhythm step
                current_beat = current_beat + step_duration;
            } else {
                // Normal step handling
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
        }

        // Sort events by start_beat to interleave polyrhythm events properly
        events.sort_by(|a, b| a.start_beat.cmp(&b.start_beat));

        // Merge events at the same start_beat into combined events
        merge_concurrent_events(events)
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

            // Special handling for Polyrhythm - each sub-pattern plays at its own tempo
            if let PatternStep::Polyrhythm(sub_patterns) = step {
                // Generate events for each sub-pattern independently
                // Each sub-pattern fits within step_duration but at its own rate
                for sub_steps in sub_patterns {
                    if sub_steps.is_empty() {
                        continue;
                    }
                    // Each sub-pattern's notes are evenly distributed within step_duration
                    let sub_event_duration = step_duration / sub_steps.len() as i64;
                    let mut sub_current_beat = current_beat;

                    for sub_step in sub_steps {
                        let step_info_list = sub_step.to_step_info_for_cycle(cycle);
                        let sub_count = step_info_list.len() as i64;
                        let event_duration = if sub_count > 0 {
                            sub_event_duration / sub_count
                        } else {
                            sub_event_duration
                        };

                        for (notes, drums, is_rest) in step_info_list {
                            events.push(PlaybackEvent {
                                notes,
                                drums,
                                start_beat: sub_current_beat,
                                duration: event_duration,
                                is_rest,
                            });
                            sub_current_beat = sub_current_beat + event_duration;
                        }
                    }
                }
                // Advance past the entire polyrhythm step
                current_beat = current_beat + step_duration;
            } else {
                // Normal step handling
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
        }

        // Sort events by start_beat to interleave polyrhythm events properly
        events.sort_by(|a, b| a.start_beat.cmp(&b.start_beat));

        // Merge events at the same start_beat into combined events
        merge_concurrent_events(events)
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
                PatternStep::Euclidean(inner, _, _) => collect_notes(inner, notes),
                PatternStep::Polyrhythm(sub_patterns) => {
                    for sub in sub_patterns {
                        for s in sub {
                            collect_notes(s, notes);
                        }
                    }
                }
                PatternStep::Velocity(inner, _) => collect_notes(inner, notes),
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

    /// Stack multiple patterns to play simultaneously at the same tempo.
    /// Notes from corresponding steps are merged into chords.
    /// If patterns have different lengths, shorter ones cycle.
    ///
    /// Example: stack(["C D", "E F"]) plays [C+E, D+F] as chords
    pub fn stack(patterns: Vec<Pattern>) -> Self {
        if patterns.is_empty() {
            return Pattern::new();
        }
        if patterns.len() == 1 {
            return patterns.into_iter().next().unwrap();
        }

        // Find the maximum number of steps across all patterns
        let max_steps = patterns.iter().map(|p| p.steps.len()).max().unwrap_or(0);
        if max_steps == 0 {
            return Pattern::new();
        }

        // Merge steps from all patterns at each position
        let mut merged_steps = Vec::with_capacity(max_steps);
        for i in 0..max_steps {
            let mut merged_notes: Vec<Note> = Vec::new();
            let mut has_rest = true;

            for pattern in &patterns {
                if pattern.steps.is_empty() {
                    continue;
                }
                // Cycle pattern if it's shorter
                let step = &pattern.steps[i % pattern.steps.len()];

                match step {
                    PatternStep::Note(n) => {
                        merged_notes.push(*n);
                        has_rest = false;
                    }
                    PatternStep::Chord(c) => {
                        merged_notes.extend(c.notes_vec());
                        has_rest = false;
                    }
                    PatternStep::Rest => {
                        // Rest doesn't add notes but doesn't prevent playing others
                    }
                    // For complex steps, flatten to notes
                    other => {
                        for (notes_info, _, is_rest) in other.to_step_info() {
                            if !is_rest {
                                has_rest = false;
                                for note_info in notes_info {
                                    if let Ok(note) = Note::new(note_info.midi) {
                                        merged_notes.push(note);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if merged_notes.is_empty() && has_rest {
                merged_steps.push(PatternStep::Rest);
            } else if merged_notes.len() == 1 {
                merged_steps.push(PatternStep::Note(merged_notes[0]));
            } else {
                merged_steps.push(PatternStep::Chord(Chord::from_notes(merged_notes)));
            }
        }

        // Use the cycle length of the first pattern
        let beats_per_cycle = patterns[0].beats_per_cycle;
        let envelope = patterns[0].envelope;
        let waveform = patterns[0].waveform.clone();
        let pan = patterns[0].pan;

        Pattern {
            steps: merged_steps,
            beats_per_cycle,
            envelope,
            waveform,
            pan,
        }
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
