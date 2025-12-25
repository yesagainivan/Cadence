//! Voice leading analysis types and functions
//!
//! This module provides analysis of voice leading between chords,
//! detecting common tones, parallel motion violations, and calculating
//! smoothness metrics.

use crate::types::{chord::Chord, note::Note};
use colored::*;
use std::fmt;

/// Represents the voice leading analysis between two chords
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceLeading {
    pub common_tones: Vec<Note>,
    pub movements: Vec<VoiceMovement>,
    pub total_movement: i8,
    pub violations: Vec<VoiceLeadingViolation>,
    pub quality_metrics: VoiceLeadingMetrics,
}

/// Tracks movement of a single voice from one chord to the next
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceMovement {
    pub from_note: Note,
    pub to_note: Note,
    pub semitones: i8,
    pub voice_index: usize, // Track which voice this is (0=bass, 1=tenor, 2=alto, 3=soprano)
}

/// Types of voice leading violations according to traditional rules
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceLeadingViolation {
    ParallelFifths {
        voice1: usize,
        voice2: usize,
    },
    ParallelOctaves {
        voice1: usize,
        voice2: usize,
    },
    ParallelUnisons {
        voice1: usize,
        voice2: usize,
    },
    HiddenFifths {
        voice1: usize,
        voice2: usize,
    },
    HiddenOctaves {
        voice1: usize,
        voice2: usize,
    },
    LargeLeap {
        voice: usize,
        semitones: i8,
    },
    VoiceCrossing {
        voice1: usize,
        voice2: usize,
    },
    WideSpacing {
        voice1: usize,
        voice2: usize,
        semitones: i8,
    },
}

/// Metrics for evaluating voice leading quality
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceLeadingMetrics {
    pub parallel_motion_count: usize,
    pub contrary_motion_count: usize,
    pub oblique_motion_count: usize,
    pub stepwise_motion_count: usize,
    pub leap_count: usize,
    pub common_tone_retention: f32,
}

/// Detailed analysis of voice leading between two specific chords
#[derive(Debug, Clone)]
pub struct VoiceLeadingAnalysis {
    pub from_chord_index: usize,
    pub to_chord_index: usize,
    pub voice_leading: VoiceLeading,
    pub quality: String,
    pub smoothness_score: f32,
    pub is_smooth: bool,
}

impl fmt::Display for VoiceLeadingAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}→{}: {} (Score: {:.1}, {})",
            self.from_chord_index,
            self.to_chord_index,
            self.quality,
            self.smoothness_score,
            if self.is_smooth { "✓" } else { "⚠" }
        )
    }
}

impl VoiceLeading {
    /// Analyze voice leading between two chords using proper voice leading rules
    pub fn analyze(from_chord: &Chord, to_chord: &Chord) -> Self {
        let from_notes = from_chord.notes_vec();
        let to_notes = to_chord.notes_vec();

        // Ensure we have the same number of voices
        let voice_count = from_notes.len().min(to_notes.len());

        // Find common tones by pitch class (ignoring octave)
        let to_pitch_classes: Vec<u8> = to_notes.iter().map(|n| n.pitch_class()).collect();
        let common_tones: Vec<Note> = from_notes
            .iter()
            .filter(|note| to_pitch_classes.contains(&note.pitch_class()))
            .copied()
            .collect();

        // Create voice movements with optimal voice assignment
        let movements = Self::create_optimal_voice_movements(&from_notes, &to_notes);

        // Calculate total movement
        let total_movement: i8 = movements.iter().map(|m| m.semitones.abs()).sum();

        // Detect violations
        let violations = Self::detect_violations(&from_notes, &to_notes, &movements);

        // Calculate quality metrics
        let quality_metrics = Self::calculate_metrics(&movements, &common_tones, voice_count);

        VoiceLeading {
            common_tones,
            movements,
            total_movement,
            violations,
            quality_metrics,
        }
    }

    fn create_optimal_voice_movements(
        from_notes: &[Note],
        to_notes: &[Note],
    ) -> Vec<VoiceMovement> {
        let voice_count = from_notes.len().min(to_notes.len());

        if voice_count == 0 {
            return Vec::new();
        }

        // For small chords (up to 4 voices), try all permutations to find optimal assignment
        if voice_count <= 4 {
            return Self::find_optimal_assignment_brute_force(from_notes, to_notes, voice_count);
        }

        // For larger chords, use greedy assignment
        Self::find_optimal_assignment_greedy(from_notes, to_notes, voice_count)
    }

    /// Find optimal voice assignment by trying all permutations (for small chords)
    fn find_optimal_assignment_brute_force(
        from_notes: &[Note],
        to_notes: &[Note],
        voice_count: usize,
    ) -> Vec<VoiceMovement> {
        let to_indices: Vec<usize> = (0..voice_count).collect();
        let mut best_movements = Vec::new();
        let mut best_total = i32::MAX;

        for perm in permutations(&to_indices) {
            let mut total = 0i32;
            let mut movements = Vec::new();

            for (i, &target_idx) in perm.iter().enumerate() {
                if i >= from_notes.len() || target_idx >= to_notes.len() {
                    continue;
                }
                let from_note = from_notes[i];
                let to_note = to_notes[target_idx];
                let semitones = Self::calculate_semitone_distance(from_note, to_note);
                total += semitones.abs() as i32;

                movements.push(VoiceMovement {
                    from_note,
                    to_note,
                    semitones,
                    voice_index: i,
                });
            }

            if total < best_total {
                best_total = total;
                best_movements = movements;
            }
        }

        best_movements
    }

    /// Greedy voice assignment for larger chords
    fn find_optimal_assignment_greedy(
        from_notes: &[Note],
        to_notes: &[Note],
        voice_count: usize,
    ) -> Vec<VoiceMovement> {
        let mut movements = Vec::new();
        let mut used_targets: Vec<bool> = vec![false; to_notes.len()];

        for (i, &from_note) in from_notes.iter().enumerate().take(voice_count) {
            let mut best_target_idx = 0;
            let mut best_distance = i8::MAX;

            for (j, &to_note) in to_notes.iter().enumerate() {
                if used_targets[j] {
                    continue;
                }
                let distance = Self::calculate_semitone_distance(from_note, to_note).abs();
                if distance < best_distance {
                    best_distance = distance;
                    best_target_idx = j;
                }
            }

            used_targets[best_target_idx] = true;
            let to_note = to_notes[best_target_idx];
            let semitones = Self::calculate_semitone_distance(from_note, to_note);

            movements.push(VoiceMovement {
                from_note,
                to_note,
                semitones,
                voice_index: i,
            });
        }

        movements
    }

    /// Calculate the shortest semitone distance between two notes
    fn calculate_semitone_distance(from: Note, to: Note) -> i8 {
        let raw_distance = to.pitch_class() as i8 - from.pitch_class() as i8;

        if raw_distance > 6 {
            raw_distance - 12
        } else if raw_distance < -6 {
            raw_distance + 12
        } else {
            raw_distance
        }
    }

    /// Detect voice leading violations according to traditional rules
    fn detect_violations(
        _from_notes: &[Note],
        _to_notes: &[Note],
        movements: &[VoiceMovement],
    ) -> Vec<VoiceLeadingViolation> {
        let mut violations = Vec::new();

        // Check for parallel motion violations
        for i in 0..movements.len() {
            for j in (i + 1)..movements.len() {
                let move1 = &movements[i];
                let move2 = &movements[j];

                if move1.semitones == 0 && move2.semitones == 0 {
                    continue;
                }

                let interval1 = Self::calculate_interval(move1.from_note, move2.from_note);
                let interval2 = Self::calculate_interval(move1.to_note, move2.to_note);

                let is_parallel = (move1.semitones > 0 && move2.semitones > 0)
                    || (move1.semitones < 0 && move2.semitones < 0);

                if is_parallel && interval1 == interval2 {
                    match interval1 % 12 {
                        0 => violations.push(VoiceLeadingViolation::ParallelUnisons {
                            voice1: move1.voice_index,
                            voice2: move2.voice_index,
                        }),
                        7 => violations.push(VoiceLeadingViolation::ParallelFifths {
                            voice1: move1.voice_index,
                            voice2: move2.voice_index,
                        }),
                        _ if interval1 % 12 == 0 => {
                            violations.push(VoiceLeadingViolation::ParallelOctaves {
                                voice1: move1.voice_index,
                                voice2: move2.voice_index,
                            })
                        }
                        _ => {}
                    }
                }

                // Check for hidden fifths/octaves
                if is_parallel && move1.semitones != 0 && move2.semitones != 0 {
                    match interval2 % 12 {
                        0 if interval1 % 12 != 0 => {
                            violations.push(VoiceLeadingViolation::HiddenOctaves {
                                voice1: move1.voice_index,
                                voice2: move2.voice_index,
                            })
                        }
                        7 if interval1 % 12 != 7 => {
                            violations.push(VoiceLeadingViolation::HiddenFifths {
                                voice1: move1.voice_index,
                                voice2: move2.voice_index,
                            })
                        }
                        _ => {}
                    }
                }
            }
        }

        // Check for large leaps
        for movement in movements {
            if movement.semitones.abs() > 4 {
                violations.push(VoiceLeadingViolation::LargeLeap {
                    voice: movement.voice_index,
                    semitones: movement.semitones.abs(),
                });
            }
        }

        violations
    }

    /// Calculate interval between two notes in semitones
    fn calculate_interval(note1: Note, note2: Note) -> i8 {
        (note2.pitch_class() as i8 - note1.pitch_class() as i8).abs()
    }

    /// Calculate quality metrics for the voice leading
    fn calculate_metrics(
        movements: &[VoiceMovement],
        common_tones: &[Note],
        voice_count: usize,
    ) -> VoiceLeadingMetrics {
        let mut parallel_motion = 0;
        let mut contrary_motion = 0;
        let mut oblique_motion = 0;
        let mut stepwise_motion = 0;
        let mut leap_count = 0;

        for i in 0..movements.len() {
            for j in (i + 1)..movements.len() {
                let move1 = &movements[i];
                let move2 = &movements[j];

                if move1.semitones == 0 && move2.semitones == 0 {
                    continue;
                } else if move1.semitones == 0 || move2.semitones == 0 {
                    oblique_motion += 1;
                } else if (move1.semitones > 0 && move2.semitones > 0)
                    || (move1.semitones < 0 && move2.semitones < 0)
                {
                    parallel_motion += 1;
                } else {
                    contrary_motion += 1;
                }
            }
        }

        for movement in movements {
            if movement.semitones.abs() <= 2 {
                stepwise_motion += 1;
            } else if movement.semitones.abs() > 2 {
                leap_count += 1;
            }
        }

        let common_tone_retention = if voice_count > 0 {
            common_tones.len() as f32 / voice_count as f32
        } else {
            0.0
        };

        VoiceLeadingMetrics {
            parallel_motion_count: parallel_motion,
            contrary_motion_count: contrary_motion,
            oblique_motion_count: oblique_motion,
            stepwise_motion_count: stepwise_motion,
            leap_count,
            common_tone_retention,
        }
    }

    /// Get a comprehensive quality score (lower is better)
    pub fn smoothness_score(&self) -> f32 {
        let mut score = 0.0;

        // Base movement penalty
        score += self.total_movement as f32 * 0.3;

        // Common tone bonus
        score -= self.common_tones.len() as f32 * 3.0;

        // Violation penalties
        for violation in &self.violations {
            match violation {
                VoiceLeadingViolation::ParallelFifths { .. } => score += 15.0,
                VoiceLeadingViolation::ParallelOctaves { .. } => score += 20.0,
                VoiceLeadingViolation::ParallelUnisons { .. } => score += 10.0,
                VoiceLeadingViolation::HiddenFifths { .. } => score += 5.0,
                VoiceLeadingViolation::HiddenOctaves { .. } => score += 5.0,
                VoiceLeadingViolation::LargeLeap { semitones, .. } => {
                    score += (*semitones as f32 - 4.0) * 2.0;
                }
                VoiceLeadingViolation::VoiceCrossing { .. } => score += 8.0,
                VoiceLeadingViolation::WideSpacing { .. } => score += 3.0,
            }
        }

        // Motion type bonuses/penalties
        score -= self.quality_metrics.contrary_motion_count as f32 * 1.0;
        score -= self.quality_metrics.stepwise_motion_count as f32 * 1.5;
        score += self.quality_metrics.leap_count as f32 * 1.0;

        // Bonus for common tone retention
        score -= self.quality_metrics.common_tone_retention * 4.0;

        score
    }

    /// Check if this follows good voice leading principles
    pub fn is_smooth(&self) -> bool {
        let has_serious_violations = self.violations.iter().any(|v| {
            matches!(
                v,
                VoiceLeadingViolation::ParallelFifths { .. }
                    | VoiceLeadingViolation::ParallelOctaves { .. }
                    | VoiceLeadingViolation::ParallelUnisons { .. }
            )
        });

        if has_serious_violations {
            return false;
        }

        let reasonable_movement = self.total_movement <= 8;
        let good_retention = self.quality_metrics.common_tone_retention >= 0.25
            || self.quality_metrics.stepwise_motion_count >= self.movements.len() / 2;

        reasonable_movement && good_retention
    }

    /// Get a detailed description of the voice leading quality
    pub fn voice_leading_type(&self) -> String {
        if !self.violations.is_empty() {
            let violation_names: Vec<String> = self
                .violations
                .iter()
                .map(|v| match v {
                    VoiceLeadingViolation::ParallelFifths { .. } => "parallel 5ths".to_string(),
                    VoiceLeadingViolation::ParallelOctaves { .. } => "parallel 8ves".to_string(),
                    VoiceLeadingViolation::ParallelUnisons { .. } => "parallel unisons".to_string(),
                    VoiceLeadingViolation::HiddenFifths { .. } => "hidden 5ths".to_string(),
                    VoiceLeadingViolation::HiddenOctaves { .. } => "hidden 8ves".to_string(),
                    VoiceLeadingViolation::LargeLeap { semitones, .. } => {
                        format!("large leap ({})", semitones)
                    }
                    VoiceLeadingViolation::VoiceCrossing { .. } => "voice crossing".to_string(),
                    VoiceLeadingViolation::WideSpacing { .. } => "wide spacing".to_string(),
                })
                .collect();

            return format!("Poor voice leading: {}", violation_names.join(", "));
        }

        match (self.common_tones.len(), self.total_movement) {
            (n, _) if n >= 2 => "Excellent: multiple common tones".to_string(),
            (1, m) if m <= 3 => "Very good: common tone + stepwise motion".to_string(),
            (1, m) if m <= 6 => "Good: common tone connection".to_string(),
            (0, m) if m <= 4 => "Good: smooth stepwise motion".to_string(),
            (0, m) if m <= 8 => "Fair: moderate voice leading".to_string(),
            _ => "Weak: wide voice leading".to_string(),
        }
    }
}

impl fmt::Display for VoiceLeading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Common tones section
        if !self.common_tones.is_empty() {
            let common_notes: Vec<String> = self
                .common_tones
                .iter()
                .map(|n| n.to_string().green().bold().to_string())
                .collect();
            write!(f, "Common: [{}] ", common_notes.join(", "))?;
        } else {
            write!(f, "No common tones ")?;
        }

        // Movements section
        if !self.movements.is_empty() {
            write!(f, "Movements: ")?;
            for (i, movement) in self.movements.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }

                let movement_str = if movement.semitones == 0 {
                    format!("{}={}", movement.from_note, movement.to_note).bright_blue()
                } else {
                    let color_fn = match movement.semitones.abs() {
                        1..=2 => |s: &String| s.green(),
                        3..=4 => |s: &String| s.yellow(),
                        _ => |s: &String| s.red(),
                    };

                    if movement.semitones > 0 {
                        color_fn(&format!(
                            "{}→{}(+{})",
                            movement.from_note, movement.to_note, movement.semitones
                        ))
                    } else {
                        color_fn(&format!(
                            "{}→{}({})",
                            movement.from_note, movement.to_note, movement.semitones
                        ))
                    }
                };

                write!(f, "{}", movement_str)?;
            }
        }

        // Violations section
        if !self.violations.is_empty() {
            write!(f, " ")?;
            let violation_indicators: Vec<String> = self
                .violations
                .iter()
                .map(|v| match v {
                    VoiceLeadingViolation::ParallelFifths { .. } => "‖5".red().bold().to_string(),
                    VoiceLeadingViolation::ParallelOctaves { .. } => "‖8".red().bold().to_string(),
                    VoiceLeadingViolation::ParallelUnisons { .. } => "‖1".red().bold().to_string(),
                    VoiceLeadingViolation::HiddenFifths { .. } => "h5".yellow().to_string(),
                    VoiceLeadingViolation::HiddenOctaves { .. } => "h8".yellow().to_string(),
                    VoiceLeadingViolation::LargeLeap { .. } => "leap".yellow().to_string(),
                    _ => "⚠".yellow().to_string(),
                })
                .collect();
            write!(f, "[{}]", violation_indicators.join(" "))?;
        }

        // Quality summary
        let quality_indicator = if self.violations.is_empty() && self.is_smooth() {
            "✓".green().bold()
        } else if self.violations.is_empty() {
            "~".yellow().bold()
        } else {
            "✗".red().bold()
        };

        write!(
            f,
            " [Total: {}, Score: {:.1}] {}",
            self.total_movement,
            self.smoothness_score(),
            quality_indicator
        )
    }
}

// ============================================================================
// Helper functions for voice leading optimization
// ============================================================================

/// Generate all permutations of a slice
pub fn permutations(items: &[usize]) -> Vec<Vec<usize>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }

    let mut result = Vec::new();
    for (i, &item) in items.iter().enumerate() {
        let mut rest: Vec<usize> = items.to_vec();
        rest.remove(i);
        for mut perm in permutations(&rest) {
            perm.insert(0, item);
            result.push(perm);
        }
    }
    result
}

/// Generate permutations of specified length
pub fn permutations_limited(items: &[usize], len: usize) -> Vec<Vec<usize>> {
    if len == 0 {
        return vec![vec![]];
    }
    if items.is_empty() {
        return vec![];
    }

    let mut result = Vec::new();
    for (i, &item) in items.iter().enumerate() {
        let mut rest: Vec<usize> = items.to_vec();
        rest.remove(i);
        for mut perm in permutations_limited(&rest, len - 1) {
            perm.insert(0, item);
            result.push(perm);
        }
    }
    result
}

/// Calculate pitch distance including octave (not just pitch class)
pub fn pitch_distance(from: Note, to: Note) -> i8 {
    let from_midi = from.pitch_class() as i16 + (from.octave() as i16 * 12);
    let to_midi = to.pitch_class() as i16 + (to.octave() as i16 * 12);

    let direct = to_midi - from_midi;
    let up_octave = direct + 12;
    let down_octave = direct - 12;

    if direct.abs() <= up_octave.abs() && direct.abs() <= down_octave.abs() {
        direct as i8
    } else if up_octave.abs() <= down_octave.abs() {
        up_octave as i8
    } else {
        down_octave as i8
    }
}

/// Adjust target note's octave to be closest to source note while staying in range
pub fn adjust_octave_to_voice(
    from_note: Note,
    to_note: Note,
    min_octave: i8,
    max_octave: i8,
) -> Note {
    let from_midi = from_note.pitch_class() as i16 + (from_note.octave() as i16 * 12);

    let mut best_octave = to_note.octave();
    let mut best_distance = i16::MAX;

    for octave_offset in -2..=2 {
        let test_octave = to_note.octave() + octave_offset;
        if test_octave < min_octave || test_octave > max_octave {
            continue;
        }

        let to_midi = to_note.pitch_class() as i16 + (test_octave as i16 * 12);
        let distance = (to_midi - from_midi).abs();

        if distance < best_distance {
            best_distance = distance;
            best_octave = test_octave;
        }
    }

    Note::new_with_octave(to_note.pitch_class(), best_octave).unwrap_or(to_note)
}

/// Find the best voicing of target_chord to follow from_chord with smooth voice leading.
/// This reorders the target chord's notes so each voice moves minimally.
pub fn find_best_voicing(from_chord: &Chord, target_chord: &Chord) -> Chord {
    let from_notes = from_chord.notes_vec();
    let to_notes = target_chord.notes_vec();

    if from_notes.is_empty() || to_notes.is_empty() {
        return target_chord.clone();
    }

    // Get the octave range from source chord for normalization
    let source_min_octave = from_notes.iter().map(|n| n.octave()).min().unwrap_or(4);
    let source_max_octave = from_notes.iter().map(|n| n.octave()).max().unwrap_or(4);

    let voice_count = from_notes.len().min(to_notes.len());
    let to_indices: Vec<usize> = (0..to_notes.len()).collect();
    let mut best_assignment: Vec<usize> = (0..voice_count).collect();
    let mut best_total = i32::MAX;

    for perm in permutations_limited(&to_indices, voice_count) {
        let mut total = 0i32;
        for (i, &target_idx) in perm.iter().enumerate() {
            if i >= from_notes.len() || target_idx >= to_notes.len() {
                continue;
            }
            let from_note = from_notes[i];
            let to_note = to_notes[target_idx];
            let distance = pitch_distance(from_note, to_note);
            total += distance.abs() as i32;
        }

        if total < best_total {
            best_total = total;
            best_assignment = perm;
        }
    }

    // Build the reordered chord with notes in voice-leading order
    let mut reordered_notes = Vec::new();
    for (voice_idx, &target_idx) in best_assignment.iter().enumerate() {
        if voice_idx >= from_notes.len() || target_idx >= to_notes.len() {
            continue;
        }

        let from_note = from_notes[voice_idx];
        let to_note = to_notes[target_idx];

        let adjusted_note =
            adjust_octave_to_voice(from_note, to_note, source_min_octave, source_max_octave);
        reordered_notes.push(adjusted_note);
    }

    // Add any remaining notes from the target that weren't assigned
    if to_notes.len() > from_notes.len() {
        for (i, note) in to_notes.iter().enumerate() {
            if !best_assignment.contains(&i) {
                reordered_notes.push(*note);
            }
        }
    }

    Chord::from_notes(reordered_notes)
}

/// Analyze voice leading for a sequence of chords
pub fn analyze_chord_sequence(chords: &[Chord]) -> Vec<VoiceLeading> {
    if chords.len() < 2 {
        return Vec::new();
    }

    let mut voice_leadings = Vec::new();
    for i in 0..chords.len() - 1 {
        voice_leadings.push(VoiceLeading::analyze(&chords[i], &chords[i + 1]));
    }
    voice_leadings
}

/// Get detailed voice leading analysis for a sequence of chords
pub fn detailed_analysis(chords: &[Chord]) -> Vec<VoiceLeadingAnalysis> {
    let voice_leadings = analyze_chord_sequence(chords);

    voice_leadings
        .into_iter()
        .enumerate()
        .map(|(i, vl)| {
            let quality = vl.voice_leading_type();
            let smoothness_score = vl.smoothness_score();
            let is_smooth = vl.is_smooth();

            VoiceLeadingAnalysis {
                from_chord_index: i,
                to_chord_index: i + 1,
                voice_leading: vl,
                quality,
                smoothness_score,
                is_smooth,
            }
        })
        .collect()
}

/// Calculate average voice leading quality for a sequence of chords
pub fn average_quality(chords: &[Chord]) -> f32 {
    let voice_leadings = analyze_chord_sequence(chords);
    if voice_leadings.is_empty() {
        return 0.0;
    }

    let total_score: f32 = voice_leadings.iter().map(|vl| vl.smoothness_score()).sum();
    total_score / voice_leadings.len() as f32
}

/// Check if the entire chord sequence has good voice leading
pub fn has_good_voice_leading(chords: &[Chord]) -> bool {
    let voice_leadings = analyze_chord_sequence(chords);

    let smooth_count = voice_leadings.iter().filter(|vl| vl.is_smooth()).count();
    let total_transitions = voice_leadings.len();

    if total_transitions == 0 {
        return true;
    }

    let smooth_ratio = smooth_count as f32 / total_transitions as f32;
    let has_major_violations = voice_leadings.iter().any(|vl| {
        vl.violations.iter().any(|v| {
            matches!(
                v,
                VoiceLeadingViolation::ParallelFifths { .. }
                    | VoiceLeadingViolation::ParallelOctaves { .. }
            )
        })
    });

    smooth_ratio >= 0.75 && !has_major_violations
}

/// Optimize voice leading for a sequence of chords
pub fn optimize_chord_sequence(chords: Vec<Chord>) -> Vec<Chord> {
    if chords.len() < 2 {
        return chords;
    }

    println!("=== Voice Leading Optimization ===");
    let original_quality = average_quality(&chords);
    println!("Original quality: {:.1}", original_quality);

    let mut optimized = vec![chords[0].clone()];
    println!("Starting with: {}", chords[0]);

    for i in 1..chords.len() {
        let previous = &optimized[i - 1];
        let current = &chords[i];

        println!("\n--- Transition {} → {} ---", i - 1, i);
        println!("From: {} | To: {}", previous, current);

        let best_inversion = find_best_voicing(previous, current);
        optimized.push(best_inversion);
    }

    let new_quality = average_quality(&optimized);

    println!("\n=== Optimization Complete ===");
    println!(
        "Quality improvement: {:.1} → {:.1} ({:+.1})",
        original_quality,
        new_quality,
        new_quality - original_quality
    );

    if new_quality < original_quality {
        println!("✓ Voice leading improved!");
    } else if (new_quality - original_quality).abs() < 0.01 {
        println!("✓ Voicings optimized for smooth voice leading");
    } else {
        println!("⚠ Voice leading got worse - this shouldn't happen!");
    }

    optimized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_leading_analysis() {
        let c_major = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let f_major = Chord::from_note_strings(vec!["F", "A", "C"]).unwrap();

        let vl = VoiceLeading::analyze(&c_major, &f_major);

        // C is common to both chords
        assert!(!vl.common_tones.is_empty());
        assert!(vl.total_movement > 0);
    }

    #[test]
    fn test_permutations() {
        let items = vec![0, 1, 2];
        let perms = permutations(&items);
        assert_eq!(perms.len(), 6); // 3! = 6
    }

    #[test]
    fn test_optimize_chord_sequence() {
        let c_major = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let f_major = Chord::from_note_strings(vec!["F", "A", "C"]).unwrap();

        let chords = vec![c_major, f_major];
        let optimized = optimize_chord_sequence(chords);

        assert_eq!(optimized.len(), 2);
    }
}
