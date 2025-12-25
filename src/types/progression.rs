use crate::types::{chord::Chord, note::Note};
use anyhow::Result;
use colored::*;
use std::fmt;
use std::ops::{Add, Index, Sub};

/// Represents a sequence of chords (a chord progression)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progression {
    chords: Vec<Chord>,
}

impl Progression {
    /// Create a new empty progression
    pub fn new() -> Self {
        Progression { chords: Vec::new() }
    }

    /// Create a progression from a vector of chords
    pub fn from_chords(chords: Vec<Chord>) -> Self {
        Progression { chords }
    }

    /// Create a progression from chord strings (e.g., vec![vec!["C", "E", "G"], vec!["F", "A", "C"]])
    pub fn from_chord_strings(chord_strings: Vec<Vec<&str>>) -> Result<Self> {
        let mut chords = Vec::new();
        for chord_notes in chord_strings {
            let chord = Chord::from_note_strings(chord_notes)?;
            chords.push(chord);
        }
        Ok(Self::from_chords(chords))
    }

    /// Add a chord to the end of the progression
    pub fn push(&mut self, chord: Chord) {
        self.chords.push(chord);
    }

    /// Remove and return the last chord in the progression
    pub fn pop(&mut self) -> Option<Chord> {
        self.chords.pop()
    }

    /// Insert a chord at the specified position
    pub fn insert(&mut self, index: usize, chord: Chord) {
        self.chords.insert(index, chord);
    }

    /// Remove a chord at the specified position
    pub fn remove(&mut self, index: usize) -> Chord {
        self.chords.remove(index)
    }

    /// Get the number of chords in the progression
    pub fn len(&self) -> usize {
        self.chords.len()
    }

    /// Check if the progression is empty
    pub fn is_empty(&self) -> bool {
        self.chords.is_empty()
    }

    /// Get an iterator over the chords in the progression
    pub fn chords(&self) -> impl Iterator<Item = &Chord> {
        self.chords.iter()
    }

    /// Get the chords as a vector (useful for indexing)
    pub fn chords_vec(&self) -> Vec<Chord> {
        self.chords.clone()
    }

    /// Get a reference to a specific chord by index
    pub fn get(&self, index: usize) -> Option<&Chord> {
        self.chords.get(index)
    }

    /// Get a mutable reference to a specific chord by index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Chord> {
        self.chords.get_mut(index)
    }

    /// Transpose the entire progression by a number of semitones
    pub fn transpose(self, semitones: i8) -> Self {
        let transposed_chords: Vec<Chord> = self
            .chords
            .into_iter()
            .map(|chord| chord + semitones)
            .collect();

        Progression {
            chords: transposed_chords,
        }
    }

    /// Apply a function to all chords in the progression
    pub fn map<F>(self, f: F) -> Self
    where
        F: Fn(Chord) -> Chord,
    {
        let mapped_chords: Vec<Chord> = self.chords.into_iter().map(f).collect();

        Progression {
            chords: mapped_chords,
        }
    }

    /// Apply a function that might fail to all chords in the progression
    pub fn try_map<F, E>(self, f: F) -> Result<Self, E>
    where
        F: Fn(Chord) -> Result<Chord, E>,
    {
        let mapped_chords: Result<Vec<Chord>, E> = self.chords.into_iter().map(f).collect();

        Ok(Progression {
            chords: mapped_chords?,
        })
    }

    /// Reverse the order of chords in the progression (retrograde)
    pub fn retrograde(mut self) -> Self {
        self.chords.reverse();
        self
    }

    /// Get the key signature that best fits this progression (basic analysis)
    pub fn analyze_key(&self) -> Option<Note> {
        if self.is_empty() {
            return None;
        }

        // Simple heuristic: the most common root note in the progression
        let mut root_counts = std::collections::HashMap::new();

        for chord in &self.chords {
            if let Some(root) = chord.root() {
                *root_counts.entry(root).or_insert(0) += 1;
            }
        }

        root_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(note, _)| note)
    }

    /// Get all unique notes used in the progression
    pub fn get_all_notes(&self) -> Vec<Note> {
        let mut all_notes = std::collections::BTreeSet::new();

        for chord in &self.chords {
            for note in chord.notes() {
                all_notes.insert(*note);
            }
        }

        all_notes.into_iter().collect()
    }

    /// Analyze voice leading between adjacent chords
    pub fn analyze_voice_leading(&self) -> Vec<VoiceLeading> {
        if self.len() < 2 {
            return Vec::new();
        }

        let mut voice_leadings = Vec::new();

        for i in 0..self.len() - 1 {
            let current = &self.chords[i];
            let next = &self.chords[i + 1];
            voice_leadings.push(VoiceLeading::analyze(current, next));
        }

        voice_leadings
    }
}

// Updated Progression methods - Replace the existing methods in progression.rs
// These methods work with the improved VoiceLeading analysis

impl Progression {
    /// Find the best inversion of target_chord to follow from_chord
    fn find_best_inversion(from_chord: &Chord, target_chord: &Chord) -> Chord {
        let mut best_chord = target_chord.clone();
        let mut best_score = f32::INFINITY;

        // Get the target octave from the from_chord's bass (to keep progression in same range)
        let target_octave = from_chord.bass().map(|b| b.octave()).unwrap_or(4);

        // Try root position and all inversions
        for inversion in 0..target_chord.len() {
            // Invert and normalize to the target octave to prevent drift
            let test_chord = target_chord
                .clone()
                .invert_n(inversion)
                .normalize_octave(target_octave);

            let voice_leading = VoiceLeading::analyze(from_chord, &test_chord);
            let score = voice_leading.smoothness_score();

            if score < best_score {
                best_score = score;
                best_chord = test_chord;
            }
        }

        best_chord
    }

    /// Debug version of find_best_inversion
    fn _find_best_inversion_(from_chord: &Chord, target_chord: &Chord) -> Chord {
        let mut best_chord = target_chord.clone();
        let mut best_score = f32::INFINITY;
        let mut best_analysis = None;

        println!("  Analyzing inversions for: {}", target_chord);
        println!("  From chord notes: {:?}", from_chord.notes_vec());

        // Try root position and all inversions
        for inversion in 0..target_chord.len() {
            let test_chord = target_chord.clone().invert_n(inversion);
            println!(
                "    Test chord {} notes: {:?}",
                inversion,
                test_chord.notes_vec()
            );

            let voice_leading = VoiceLeading::analyze(from_chord, &test_chord);
            let score = voice_leading.smoothness_score();

            println!(
                "    Inversion {}: {} -> {}",
                inversion,
                test_chord,
                voice_leading.voice_leading_type()
            );
            println!(
                "      Score: {:.1} | Violations: {} | Common tones: {}",
                score,
                voice_leading.violations.len(),
                voice_leading.common_tones.len()
            );

            if score < best_score {
                best_score = score;
                best_chord = test_chord.clone();
                best_analysis = Some(voice_leading);
                println!("      ^ NEW BEST");
            }
        }

        if let Some(analysis) = best_analysis {
            println!(
                "  Selected: {} ({})",
                best_chord,
                analysis.voice_leading_type()
            );
            if !analysis.violations.is_empty() {
                println!("    ⚠ Violations present - may need manual review");
            }
        }

        best_chord
    }

    /// Enhanced voice leading optimization with better analysis
    pub fn optimize_voice_leading(self) -> Self {
        if self.len() < 2 {
            println!("Cannot optimize voice leading: progression too short");
            return self;
        }

        println!("=== Voice Leading Optimization ===");
        let original_quality = self.average_voice_leading_quality();
        println!("Original quality: {:.1}", original_quality);

        let mut optimized_chords = vec![self.chords[0].clone()];
        println!("Starting with: {}", self.chords[0]);

        for i in 1..self.chords.len() {
            let previous_chord = &optimized_chords[i - 1];
            let current_chord = &self.chords[i];

            println!("\n--- Transition {} → {} ---", i - 1, i);
            println!("From: {} | To: {}", previous_chord, current_chord);

            let best_inversion = Self::find_best_inversion(previous_chord, current_chord);
            optimized_chords.push(best_inversion);
        }

        let optimized = Progression::from_chords(optimized_chords);
        let new_quality = optimized.average_voice_leading_quality();

        println!("\n=== Optimization Complete ===");
        println!(
            "Quality improvement: {:.1} → {:.1} ({:+.1})",
            original_quality,
            new_quality,
            new_quality - original_quality
        );

        if new_quality < original_quality {
            println!("✓ Voice leading improved!");
        } else if new_quality == original_quality {
            println!("~ Voice leading unchanged (already optimal)");
        } else {
            println!("⚠ Voice leading got worse - this shouldn't happen!");
        }

        optimized
    }

    /// Get detailed voice leading analysis with enhanced output
    pub fn detailed_voice_leading_analysis(&self) -> Vec<VoiceLeadingAnalysis> {
        let voice_leadings = self.analyze_voice_leading();

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

    /// Check if the entire progression has good voice leading (stricter criteria)
    pub fn has_good_voice_leading(&self) -> bool {
        let voice_leadings = self.analyze_voice_leading();

        // Check that most transitions are smooth and no major violations exist
        let smooth_count = voice_leadings.iter().filter(|vl| vl.is_smooth()).count();
        let total_transitions = voice_leadings.len();

        if total_transitions == 0 {
            return true; // Single chord or empty progression
        }

        // At least 75% of transitions should be smooth
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

    /// Get average voice leading quality with improved calculation
    pub fn average_voice_leading_quality(&self) -> f32 {
        let voice_leadings = self.analyze_voice_leading();
        if voice_leadings.is_empty() {
            return 0.0;
        }

        let total_score: f32 = voice_leadings.iter().map(|vl| vl.smoothness_score()).sum();
        total_score / voice_leadings.len() as f32
    }

    /// Get a comprehensive voice leading report
    pub fn voice_leading_report(&self) -> String {
        if self.len() < 2 {
            return "Progression too short for voice leading analysis".to_string();
        }

        let mut report = String::new();
        let analysis = self.detailed_voice_leading_analysis();
        let avg_quality = self.average_voice_leading_quality();
        let has_good_vl = self.has_good_voice_leading();

        report.push_str(&format!("=== Voice Leading Report ===\n"));
        report.push_str(&format!("Progression: {}\n\n", self));

        report.push_str("Transitions:\n");
        for item in &analysis {
            report.push_str(&format!("  {}\n", item));

            // Add violation details if any
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

        report.push_str(&format!("\nSummary:\n"));
        report.push_str(&format!("  Average quality score: {:.1}\n", avg_quality));
        report.push_str(&format!(
            "  Overall assessment: {}\n",
            if has_good_vl {
                "✓ Good voice leading"
            } else {
                "⚠ Needs improvement"
            }
        ));

        // Provide suggestions if voice leading could be improved
        if !has_good_vl {
            report.push_str(&format!("\nSuggestions:\n"));
            report.push_str(&format!(
                "  - Try running smooth_voice_leading() to optimize inversions\n"
            ));
            report.push_str(&format!("  - Look for common tones between chords\n"));
            report.push_str(&format!("  - Minimize large leaps in individual voices\n"));
        }

        report
    }

    /// Convert this progression to a Pattern for unified playback
    /// This enables envelope handling and cycle-based timing for progressions
    pub fn to_pattern(&self) -> crate::types::Pattern {
        crate::types::Pattern::from_progression(self)
    }
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

// Represents the voice leading analysis between two chords
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceLeading {
    pub common_tones: Vec<Note>,
    pub movements: Vec<VoiceMovement>, // Enhanced movement tracking
    pub total_movement: i8,
    pub violations: Vec<VoiceLeadingViolation>, // Track specific violations
    pub quality_metrics: VoiceLeadingMetrics,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceMovement {
    pub from_note: Note,
    pub to_note: Note,
    pub semitones: i8,
    pub voice_index: usize, // Track which voice this is (0=bass, 1=tenor, 2=alto, 3=soprano)
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceLeadingMetrics {
    pub parallel_motion_count: usize,
    pub contrary_motion_count: usize,
    pub oblique_motion_count: usize,
    pub stepwise_motion_count: usize,
    pub leap_count: usize,
    pub common_tone_retention: f32,
}

impl VoiceLeading {
    /// Analyze voice leading between two chords using proper voice leading rules
    pub fn analyze(from_chord: &Chord, to_chord: &Chord) -> Self {
        let from_notes = from_chord.notes_vec();
        let to_notes = to_chord.notes_vec();

        // Ensure we have the same number of voices
        let voice_count = from_notes.len().min(to_notes.len());

        // Find common tones (notes that stay the same)
        let common_tones: Vec<Note> = from_notes
            .iter()
            .filter(|note| to_notes.contains(note))
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
        let mut movements = Vec::new();
        let voice_count = from_notes.len().min(to_notes.len());

        // Direct 1-to-1 assignment respecting chord order
        for i in 0..voice_count {
            let from_note = from_notes[i];
            let to_note = to_notes[i];
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

        // Choose the shortest path (considering octave wrapping)
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

                // Skip if no motion in either voice
                if move1.semitones == 0 && move2.semitones == 0 {
                    continue;
                }

                let interval1 = Self::calculate_interval(move1.from_note, move2.from_note);
                let interval2 = Self::calculate_interval(move1.to_note, move2.to_note);

                // Check for parallel motion (both voices move in same direction)
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

                // Check for hidden fifths/octaves (similar motion to perfect consonances)
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
                // Larger than a major third
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

        // Count motion types between voice pairs
        for i in 0..movements.len() {
            for j in (i + 1)..movements.len() {
                let move1 = &movements[i];
                let move2 = &movements[j];

                if move1.semitones == 0 && move2.semitones == 0 {
                    continue; // No motion
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

        // Count stepwise motion and leaps
        for movement in movements {
            if movement.semitones.abs() <= 2 {
                stepwise_motion += 1;
            } else if movement.semitones.abs() > 2 {
                leap_count += 1;
            }
        }

        let common_tone_retention = common_tones.len() as f32 / voice_count as f32;

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

        // Common tone bonus (very important)
        score -= self.common_tones.len() as f32 * 3.0;

        // Violation penalties (these are serious)
        for violation in &self.violations {
            match violation {
                VoiceLeadingViolation::ParallelFifths { .. } => score += 15.0,
                VoiceLeadingViolation::ParallelOctaves { .. } => score += 20.0,
                VoiceLeadingViolation::ParallelUnisons { .. } => score += 10.0,
                VoiceLeadingViolation::HiddenFifths { .. } => score += 5.0,
                VoiceLeadingViolation::HiddenOctaves { .. } => score += 5.0,
                VoiceLeadingViolation::LargeLeap { semitones, .. } => {
                    score += (*semitones as f32 - 4.0) * 2.0; // Penalty increases with leap size
                }
                VoiceLeadingViolation::VoiceCrossing { .. } => score += 8.0,
                VoiceLeadingViolation::WideSpacing { .. } => score += 3.0,
            }
        }

        // Motion type bonuses/penalties
        score -= self.quality_metrics.contrary_motion_count as f32 * 1.0; // Contrary motion is good
        score -= self.quality_metrics.stepwise_motion_count as f32 * 1.5; // Stepwise motion is good
        score += self.quality_metrics.leap_count as f32 * 1.0; // Leaps are less ideal

        // Bonus for common tone retention
        score -= self.quality_metrics.common_tone_retention * 4.0;

        score
    }

    /// Check if this follows good voice leading principles (stricter than before)
    pub fn is_smooth(&self) -> bool {
        // No major violations
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

        // Reasonable total movement
        let reasonable_movement = self.total_movement <= 8;

        // Good common tone retention or small movements
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

impl Default for Progression {
    fn default() -> Self {
        Self::new()
    }
}

// Replace the existing Display implementation for Progression
impl fmt::Display for Progression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "{}", "[]".bright_black());
        }

        write!(f, "{}", "[".bright_white().bold())?;
        for (i, chord) in self.chords.iter().enumerate() {
            if i > 0 {
                write!(f, "{} ", ",".bright_white())?;
            }
            write!(f, "{}", chord)?;
        }
        write!(f, "{}", "]".bright_white().bold())
    }
}

impl fmt::Display for VoiceLeading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Common tones section with better formatting
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

        // Movements section with enhanced display
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
                        1..=2 => |s: &String| s.green(),  // Stepwise
                        3..=4 => |s: &String| s.yellow(), // Small leap
                        _ => |s: &String| s.red(),        // Large leap
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

// Arithmetic operations for transposition
impl Add<i8> for Progression {
    type Output = Progression;

    fn add(self, semitones: i8) -> Self::Output {
        self.transpose(semitones)
    }
}

impl Sub<i8> for Progression {
    type Output = Progression;

    fn sub(self, semitones: i8) -> Self::Output {
        self.transpose(-semitones)
    }
}

// Index access to chords
impl Index<usize> for Progression {
    type Output = Chord;

    fn index(&self, index: usize) -> &Self::Output {
        &self.chords[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_major_progression() -> Progression {
        Progression::from_chord_strings(vec![
            vec!["C", "E", "G"],
            vec!["F", "A", "C"],
            vec!["G", "B", "D"],
            vec!["C", "E", "G"],
        ])
        .unwrap()
    }

    #[test]
    fn test_progression_creation() {
        let prog = c_major_progression();
        assert_eq!(prog.len(), 4);
        assert!(!prog.is_empty());

        // Test first chord is C major
        let first_chord = &prog[0];
        assert!(first_chord.contains(&"C".parse().unwrap()));
        assert!(first_chord.contains(&"E".parse().unwrap()));
        assert!(first_chord.contains(&"G".parse().unwrap()));
    }

    #[test]
    fn test_progression_from_invalid_chords() {
        let result = Progression::from_chord_strings(vec![vec!["C", "X", "G"]]);
        assert!(result.is_err());
    }

    #[test]
    fn test_progression_push_pop() {
        let mut prog = Progression::new();
        assert!(prog.is_empty());

        let c_major = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        prog.push(c_major.clone());

        assert_eq!(prog.len(), 1);
        assert_eq!(prog[0], c_major);

        let popped = prog.pop();
        assert_eq!(popped, Some(c_major));
        assert!(prog.is_empty());
    }

    #[test]
    fn test_progression_insert_remove() {
        let mut prog = c_major_progression();
        let original_len = prog.len();

        let dm = Chord::from_note_strings(vec!["D", "F", "A"]).unwrap();
        prog.insert(1, dm.clone());

        assert_eq!(prog.len(), original_len + 1);
        assert_eq!(prog[1], dm);

        let removed = prog.remove(1);
        assert_eq!(removed, dm);
        assert_eq!(prog.len(), original_len);
    }

    #[test]
    fn test_progression_transpose() {
        let prog = c_major_progression();
        let transposed = prog + 2; // Up 2 semitones

        // First chord should be D major (C major + 2)
        let first_chord = &transposed[0];
        let pitch_classes: Vec<u8> = first_chord.notes().map(|n| n.pitch_class()).collect();
        assert!(pitch_classes.contains(&2)); // D
        assert!(pitch_classes.contains(&6)); // F#
        assert!(pitch_classes.contains(&9)); // A
    }

    #[test]
    fn test_progression_map() {
        let prog = c_major_progression();
        let inverted_prog = prog.map(|chord| chord.invert());

        // First chord should be C major first inversion
        // Compare pitch_class because octave changes during inversion
        let first_chord = &inverted_prog[0];
        assert_eq!(first_chord.bass().unwrap().pitch_class(), 4); // E in bass
        assert_eq!(first_chord.root().unwrap().pitch_class(), 0); // C still root
    }

    #[test]
    fn test_progression_retrograde() {
        let prog = c_major_progression();
        let original_first = prog[0].clone();
        let original_last = prog[prog.len() - 1].clone();

        let retrograde = prog.retrograde();

        // First and last should be swapped
        assert_eq!(retrograde[0], original_last);
        assert_eq!(retrograde[retrograde.len() - 1], original_first);
    }

    #[test]
    fn test_progression_analyze_key() {
        let prog = c_major_progression();
        let key = prog.analyze_key();

        // Should identify C as the key (appears twice as root)
        assert_eq!(key, Some("C".parse().unwrap()));
    }

    #[test]
    fn test_progression_get_all_notes() {
        let prog = c_major_progression();
        let all_notes = prog.get_all_notes();

        // Should contain all notes from C major scale
        let note_classes: Vec<u8> = all_notes.iter().map(|n| n.pitch_class()).collect();
        assert!(note_classes.contains(&0)); // C
        assert!(note_classes.contains(&2)); // D
        assert!(note_classes.contains(&4)); // E
        assert!(note_classes.contains(&5)); // F
        assert!(note_classes.contains(&7)); // G
        assert!(note_classes.contains(&9)); // A
        assert!(note_classes.contains(&11)); // B
    }

    #[test]
    fn test_voice_leading_analysis() {
        let prog = c_major_progression();
        let voice_leadings = prog.analyze_voice_leading();

        assert_eq!(voice_leadings.len(), 3); // 4 chords = 3 transitions

        // First transition: C maj -> F maj (common tone: C)
        let first_transition = &voice_leadings[0];
        assert!(
            first_transition
                .common_tones
                .contains(&"C".parse().unwrap())
        );

        // Should have movements for all voices
        assert_eq!(first_transition.movements.len(), 3); // All 3 voices move
        assert!(first_transition.total_movement > 0);
    }

    #[test]
    fn test_voice_leading_common_tones() {
        let c_maj = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let a_min = Chord::from_note_strings(vec!["A", "C", "E"]).unwrap();

        let voice_leading = VoiceLeading::analyze(&c_maj, &a_min);

        // Common tones should be C and E
        assert_eq!(voice_leading.common_tones.len(), 2);
        assert!(voice_leading.common_tones.contains(&"C".parse().unwrap()));
        assert!(voice_leading.common_tones.contains(&"E".parse().unwrap()));

        // Should have movements for all three voices since the implementation creates 1-to-1 mappings
        assert_eq!(voice_leading.movements.len(), 3);

        // Check that we have the expected movements (C->A, E->C, G->E based on chord order)
        let movements = &voice_leading.movements;
        assert_eq!(movements[0].from_note, "C".parse().unwrap());
        assert_eq!(movements[0].to_note, "A".parse().unwrap());
        assert_eq!(movements[1].from_note, "E".parse().unwrap());
        assert_eq!(movements[1].to_note, "C".parse().unwrap());
        assert_eq!(movements[2].from_note, "G".parse().unwrap());
        assert_eq!(movements[2].to_note, "E".parse().unwrap());
    }

    #[test]
    fn test_progression_display() {
        let prog = c_major_progression();
        let display = format!("{}", prog);

        // Note: colored output may contain ANSI codes, check for content presence
        assert!(display.contains("["));
        assert!(display.contains("]"));
        assert!(display.contains("C"));
        assert!(display.contains("F"));

        let empty = Progression::new();
        let empty_display = format!("{}", empty);
        // Empty display contains [] but may have ANSI codes
        assert!(empty_display.len() >= 2);
    }

    #[test]
    fn test_progression_index_access() {
        let prog = c_major_progression();

        // Test index access
        let first_chord = &prog[0];
        assert!(first_chord.contains(&"C".parse().unwrap()));

        // Test get method
        assert!(prog.get(0).is_some());
        assert!(prog.get(100).is_none());
    }

    #[test]
    fn test_empty_progression() {
        let empty = Progression::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.analyze_key(), None);
        assert!(empty.get_all_notes().is_empty());
        assert!(empty.analyze_voice_leading().is_empty());
    }
}
