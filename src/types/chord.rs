use crate::types::note::Note;
use anyhow::Result;
use colored::*;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::{Add, BitAnd, BitOr, BitXor, Sub};

/// Represents a musical chord as a collection of notes with bass note tracking for inversions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    notes: BTreeSet<Note>,
    bass_note: Option<Note>, // The note that should be in the bass (for inversions)
    input_order: Vec<Note>,  // Preserve original input order for display
}
impl Chord {
    /// Create a new empty chord
    pub fn new() -> Self {
        Chord {
            notes: BTreeSet::new(),
            bass_note: None,
            input_order: Vec::new(),
        }
    }

    /// Create a chord from a vector of notes
    pub fn from_notes(notes: Vec<Note>) -> Self {
        let mut chord = Chord::new();
        chord.input_order = notes.clone(); // Preserve input order
        for note in notes {
            chord.add_note(note);
        }
        chord
    }

    /// Create a chord from note strings (e.g., vec!["C", "E", "G"])
    pub fn from_note_strings(note_strings: Vec<&str>) -> Result<Self> {
        let mut notes = Vec::new();
        for note_str in note_strings {
            let note: Note = note_str.parse()?;
            notes.push(note);
        }
        Ok(Self::from_notes(notes))
    }

    /// Create a chord with a specific bass note
    pub fn with_bass(notes: Vec<Note>, bass: Note) -> Self {
        let mut chord = Self::from_notes(notes);
        if chord.notes.contains(&bass) {
            chord.bass_note = Some(bass);
        }
        chord
    }

    /// Add a note to the chord
    pub fn add_note(&mut self, note: Note) {
        let was_empty = self.notes.is_empty();
        self.notes.insert(note);

        // If this is the first note, make it the bass
        if was_empty {
            self.bass_note = Some(note);
        }

        // Add to input order if not already present
        if !self.input_order.contains(&note) {
            self.input_order.push(note);
        }
    }

    /// Remove a note from the chord
    pub fn remove_note(&mut self, note: &Note) -> bool {
        let removed = self.notes.remove(note);

        if removed {
            // Remove from input order
            self.input_order.retain(|n| n != note);

            // If we removed the bass note, update bass to the lowest remaining note
            if self.bass_note == Some(*note) {
                self.bass_note = self.notes.iter().next().copied();
            }
        }

        removed
    }

    /// Check if the chord contains a specific note
    pub fn contains(&self, note: &Note) -> bool {
        self.notes.contains(note)
    }

    /// Get the number of notes in the chord
    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Check if the chord is empty
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Get an iterator over the notes in the chord
    pub fn notes(&self) -> impl Iterator<Item = &Note> {
        self.notes.iter()
    }

    // /// Get the notes as a vector (useful for indexing)
    // pub fn notes_vec(&self) -> Vec<Note> {
    //     self.notes.iter().copied().collect()
    // }

    /// Get the notes as a vector in input order (respects inversions)
    pub fn notes_vec(&self) -> Vec<Note> {
        self.input_order.iter().copied().collect()
        // self.input_order.clone()
    }

    /// Get the bass note (the note that should be played in the bass)
    pub fn bass(&self) -> Option<Note> {
        self.bass_note.or_else(|| self.notes.iter().next().copied())
    }

    /// Get the root note (the fundamental root of the chord, regardless of inversion)
    /// This attempts to find the actual harmonic root through analysis
    pub fn root(&self) -> Option<Note> {
        if self.is_empty() {
            return None;
        }

        // For triads, try to find the root by analyzing intervals
        if self.notes.len() == 3 {
            if let Some(analyzed_root) = self.find_triad_root() {
                return Some(analyzed_root);
            }
        }

        // Fall back to bass note or lowest note
        self.bass()
    }

    /// Find the root of a triad by analyzing intervals
    fn find_triad_root(&self) -> Option<Note> {
        let notes_vec = self.notes_vec();
        if notes_vec.len() != 3 {
            return None;
        }

        // Try each note as a potential root
        for &potential_root in &notes_vec {
            let mut intervals = Vec::new();

            for &note in &notes_vec {
                if note != potential_root {
                    let interval =
                        (note.pitch_class() as i8 - potential_root.pitch_class() as i8 + 12) % 12;
                    intervals.push(interval);
                }
            }

            intervals.sort();

            // Check for common triad patterns (including suspended chords)
            match intervals.as_slice() {
                [2, 7] => return Some(potential_root), // sus2 chord
                [3, 7] => return Some(potential_root), // minor triad
                [4, 7] => return Some(potential_root), // major triad
                [3, 6] => return Some(potential_root), // diminished triad
                [4, 8] => return Some(potential_root), // augmented triad
                [5, 7] => return Some(potential_root), // sus4 chord
                _ => continue,
            }
        }

        None
    }

    /// Transpose the entire chord by a number of semitones
    pub fn transpose(self, semitones: i8) -> Self {
        let transposed_notes: BTreeSet<Note> = self
            .notes
            .into_iter()
            .map(|note| note + semitones)
            .collect();

        let transposed_bass = self.bass_note.map(|bass| bass + semitones);

        // Transpose the input order as well
        let transposed_input_order: Vec<Note> = self
            .input_order
            .into_iter()
            .map(|note| note + semitones)
            .collect();

        Chord {
            notes: transposed_notes,
            bass_note: transposed_bass,
            input_order: transposed_input_order,
        }
    }

    /// Create the first inversion of the chord
    pub fn invert(mut self) -> Self {
        if self.notes.len() < 2 {
            return self;
        }

        let notes_vec = self.notes_vec();
        let current_bass = self.bass().unwrap();

        // Find the next note in pitch-class order to become the bass
        if let Some(current_index) = notes_vec.iter().position(|&n| n == current_bass) {
            let next_bass_index = (current_index + 1) % notes_vec.len();
            self.bass_note = Some(notes_vec[next_bass_index]);

            // Update input order to reflect the inversion
            // Move the bass note to the front
            let new_bass = notes_vec[next_bass_index];
            self.input_order.retain(|n| *n != new_bass);
            self.input_order.insert(0, new_bass);
        }

        self
    }

    /// Create the nth inversion of the chord
    pub fn invert_n(mut self, n: usize) -> Self {
        if self.notes.len() < 2 {
            return self;
        }

        // Use modulo to handle cases where n > chord length
        let effective_n = n % self.notes.len();

        if effective_n == 0 {
            // No inversion needed - return as is
            return self;
        }

        // Create new input order by rotating the notes
        let mut new_input_order = self.input_order.clone();

        // Rotate left by effective_n positions
        // This moves the first effective_n notes to the end
        new_input_order.rotate_left(effective_n);

        // Update bass note to the new first note
        if let Some(new_bass) = new_input_order.first() {
            self.bass_note = Some(*new_bass);
        }

        // Update input order
        self.input_order = new_input_order;

        self
    }

    /// Get the inversion number (0 = root position, 1 = first inversion, etc.)
    pub fn inversion(&self) -> usize {
        if let (Some(root), Some(bass)) = (self.root(), self.bass()) {
            if root == bass {
                return 0; // Root position
            }

            let notes_vec = self.notes_vec();
            if let Some(root_index) = notes_vec.iter().position(|&n| n == root) {
                if let Some(bass_index) = notes_vec.iter().position(|&n| n == bass) {
                    return (bass_index + notes_vec.len() - root_index) % notes_vec.len();
                }
            }
        }
        0
    }

    /// Analyze the chord and try to identify it
    pub fn analyze(&self) -> String {
        if self.is_empty() {
            return "Empty".to_string();
        }

        let notes_vec = self.notes_vec();

        match notes_vec.len() {
            1 => format!("{}", notes_vec[0]),
            2 => self.analyze_interval(),
            3 => self.analyze_triad(),
            4 => self.analyze_seventh(), // Now handles both 7th and 6th chords
            5 => self.analyze_extended(), // For 9th, 11th chords etc.
            _ => format!("{}-note chord", self.len()),
        }
    }

    /// Analyze extended chords (5+ notes)
    fn analyze_extended(&self) -> String {
        if let Some(root) = self.root() {
            let notes_vec = self.notes_vec();
            let mut intervals = Vec::new();

            // Calculate intervals from the root
            for &note in &notes_vec {
                if note != root {
                    let interval = (note.pitch_class() as i8 - root.pitch_class() as i8 + 12) % 12;
                    intervals.push(interval);
                }
            }

            intervals.sort();

            // For now, just identify some common extended chords
            let chord_quality = match intervals.as_slice() {
                [2, 4, 7, 10] => "9th",             // major triad + 7th + 9th
                [2, 3, 7, 10] => "minor 9th",       // minor triad + 7th + 9th
                [2, 4, 7, 11] => "Major 9th",       // major triad + maj7 + 9th
                [2, 3, 7, 11] => "minor Major 9th", // minor triad + maj7 + 9th
                _ => &format!("{}-note", self.len()),
            };

            format!("{} {}", root, chord_quality)
        } else {
            format!("{}-note chord", self.len())
        }
    }

    fn analyze_interval(&self) -> String {
        let notes_vec = self.notes_vec();
        if notes_vec.len() != 2 {
            return "Unknown".to_string();
        }

        let bass = self.bass().unwrap();
        let other = if notes_vec[0] == bass {
            notes_vec[1]
        } else {
            notes_vec[0]
        };

        let interval = (other.pitch_class() as i8 - bass.pitch_class() as i8 + 12) % 12;
        let interval_name = match interval {
            1 => "minor 2nd",
            2 => "major 2nd",
            3 => "minor 3rd",
            4 => "major 3rd",
            5 => "perfect 4th",
            6 => "tritone",
            7 => "perfect 5th",
            8 => "minor 6th",
            9 => "major 6th",
            10 => "minor 7th",
            11 => "major 7th",
            0 => "unison",
            _ => "unknown interval",
        };

        format!("{}-{} ({})", bass, other, interval_name)
    }

    fn analyze_triad(&self) -> String {
        if let Some(root) = self.root() {
            let notes_vec = self.notes_vec();
            let mut intervals = Vec::new();

            // Calculate intervals from the root
            for &note in &notes_vec {
                if note != root {
                    let interval = (note.pitch_class() as i8 - root.pitch_class() as i8 + 12) % 12;
                    intervals.push(interval);
                }
            }

            intervals.sort();

            let chord_quality = match intervals.as_slice() {
                [2, 7] => "sus2",       // suspended 2nd: root, 2nd, 5th
                [3, 7] => "minor",      // minor triad
                [4, 7] => "Major",      // major triad
                [3, 6] => "diminished", // diminished triad
                [4, 8] => "Augmented",  // augmented triad
                [5, 7] => "sus4",       // suspended 4th: root, 4th, 5th
                _ => "Unknown",
            };

            // Add inversion information
            let inversion_info = match self.inversion() {
                0 => "",
                1 => " (1st inv)",
                2 => " (2nd inv)",
                n => &format!(" ({}th inv)", n),
            };

            format!("{} {}{}", root, chord_quality, inversion_info)
        } else {
            "Unknown triad".to_string()
        }
    }

    fn analyze_seventh(&self) -> String {
        if let Some(root) = self.root() {
            let notes_vec = self.notes_vec();
            let mut intervals = Vec::new();

            // Calculate intervals from the root
            for &note in &notes_vec {
                if note != root {
                    let interval = (note.pitch_class() as i8 - root.pitch_class() as i8 + 12) % 12;
                    intervals.push(interval);
                }
            }

            intervals.sort();

            let chord_quality = match intervals.as_slice() {
                // 6th chords (3 notes + 6th)
                [3, 7, 9] => "minor 6th", // minor triad + major 6th
                [4, 7, 9] => "Major 6th", // major triad + major 6th

                // 7th chords (3 notes + 7th)
                [3, 6, 9] => "diminished 7th", // diminished triad + diminished 7th
                [3, 6, 10] => "minor 7th♭5",   // minor triad♭5 + minor 7th
                [3, 7, 10] => "minor 7th",     // minor triad + minor 7th
                [3, 7, 11] => "minor Major 7th", // minor triad + major 7th
                [4, 7, 10] => "Dominant 7th",  // major triad + minor 7th
                [4, 7, 11] => "Major 7th",     // major triad + major 7th
                [4, 8, 10] => "Augmented 7th", // augmented triad + minor 7th
                [4, 8, 11] => "Augmented Major 7th", // augmented triad + major 7th

                // Suspended 7th chords
                [2, 7, 10] => "sus2 7th",       // sus2 + minor 7th
                [2, 7, 11] => "sus2 Major 7th", // sus2 + major 7th
                [5, 7, 10] => "sus4 7th",       // sus4 + minor 7th
                [5, 7, 11] => "sus4 Major 7th", // sus4 + major 7th

                // Add9 chords (without 7th)
                [2, 4, 7] => "add9",       // major triad + 9th (no 7th)
                [2, 3, 7] => "minor add9", // minor triad + 9th (no 7th)

                _ => "Unknown 4-note",
            };

            // Add inversion information for 4-note chords
            let inversion_info = match self.inversion() {
                0 => "",
                1 => " (1st inv)",
                2 => " (2nd inv)",
                3 => " (3rd inv)",
                n => &format!(" ({}th inv)", n),
            };

            format!("{} {}{}", root, chord_quality, inversion_info)
        } else {
            "Unknown 4-note chord".to_string()
        }
    }
}

impl Default for Chord {
    fn default() -> Self {
        Self::new()
    }
}

// Replace the existing Display implementation for Chord
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "{}", "[]".bright_black());
        }

        // Use input order for display with colored notes
        let notes_str: Vec<String> = self
            .input_order
            .iter()
            .map(|n| n.to_string().cyan().to_string())
            .collect();

        let analysis = self.analyze();

        // Show bass note if different from root (slash chord notation)
        let bass_info = if let (Some(root), Some(bass)) = (self.root(), self.bass()) {
            if root != bass {
                format!("/{}", bass.to_string().magenta().bold())
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Color-code different chord types
        let colored_analysis = if analysis.contains("Major") && !analysis.contains("minor") {
            analysis.blue().bold().to_string()
        } else if analysis.contains("minor") {
            analysis.red().bold().to_string()
        } else if analysis.contains("sus") {
            analysis.yellow().bold().to_string()
        } else if analysis.contains("7th") {
            analysis.green().bold().to_string()
        } else if analysis.contains("6th") {
            analysis.bright_green().bold().to_string()
        } else if analysis.contains("add") {
            analysis.bright_cyan().bold().to_string()
        } else if analysis.contains("diminished") {
            analysis.purple().bold().to_string()
        } else if analysis.contains("Augmented") {
            analysis.bright_red().bold().to_string()
        } else {
            analysis.white().to_string()
        };

        // Show both the note list and analysis when possible
        if analysis.contains("Major")
            || analysis.contains("minor")
            || analysis.contains("7th")
            || analysis.contains("sus")
            || analysis.contains("6th")
            || analysis.contains("add")
            || analysis.contains("diminished")
            || analysis.contains("Augmented")
        {
            write!(
                f,
                "{}{}: {}{}{}",
                colored_analysis,
                bass_info,
                "[".bright_white(),
                notes_str.join(&format!("{} ", ",".bright_white())),
                "]".bright_white()
            )
        } else {
            write!(
                f,
                "{}{}{}{}",
                "[".bright_white(),
                notes_str.join(&format!("{} ", ",".bright_white())),
                "]".bright_white(),
                bass_info
            )
        }
    }
}

// Arithmetic operations for transposition
impl Add<i8> for Chord {
    type Output = Chord;

    fn add(self, semitones: i8) -> Self::Output {
        self.transpose(semitones)
    }
}

impl Sub<i8> for Chord {
    type Output = Chord;

    fn sub(self, semitones: i8) -> Self::Output {
        self.transpose(-semitones)
    }
}

// Set operations for harmonic analysis
impl BitAnd for Chord {
    type Output = Chord;

    /// Intersection: common tones between chords
    fn bitand(self, other: Chord) -> Self::Output {
        let common_notes: BTreeSet<Note> = self.notes.intersection(&other.notes).copied().collect();

        // Create input order from common notes, preserving left operand's order
        let mut input_order = Vec::new();
        for note in &self.input_order {
            if common_notes.contains(note) {
                input_order.push(*note);
            }
        }
        // Add any remaining notes from right operand that weren't in left's order
        for note in &other.input_order {
            if common_notes.contains(note) && !input_order.contains(note) {
                input_order.push(*note);
            }
        }

        Chord {
            notes: common_notes,
            bass_note: None, // Reset bass for set operations
            input_order,
        }
    }
}

impl BitOr for Chord {
    type Output = Chord;

    /// Union: all notes from both chords
    fn bitor(self, other: Chord) -> Self::Output {
        let all_notes: BTreeSet<Note> = self.notes.union(&other.notes).copied().collect();

        // Create input order combining both operands, left first
        let mut input_order = self.input_order.clone();
        for note in &other.input_order {
            if all_notes.contains(note) && !input_order.contains(note) {
                input_order.push(*note);
            }
        }

        Chord {
            notes: all_notes,
            bass_note: self.bass_note.or(other.bass_note), // Prefer left operand's bass
            input_order,
        }
    }
}

impl BitXor for Chord {
    type Output = Chord;

    /// Symmetric difference: notes that are in one chord but not both
    fn bitxor(self, other: Chord) -> Self::Output {
        let diff_notes: BTreeSet<Note> = self
            .notes
            .symmetric_difference(&other.notes)
            .copied()
            .collect();

        // Create input order from diff notes, preserving original orders
        let mut input_order = Vec::new();
        for note in &self.input_order {
            if diff_notes.contains(note) {
                input_order.push(*note);
            }
        }
        for note in &other.input_order {
            if diff_notes.contains(note) && !input_order.contains(note) {
                input_order.push(*note);
            }
        }

        Chord {
            notes: diff_notes,
            bass_note: None, // Reset bass for set operations
            input_order,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_major() -> Chord {
        Chord::from_note_strings(vec!["C", "E", "G"]).unwrap()
    }

    // fn f_major() -> Chord {
    //     Chord::from_note_strings(vec!["F", "A", "C"]).unwrap()
    // }

    fn a_minor() -> Chord {
        Chord::from_note_strings(vec!["A", "C", "E"]).unwrap()
    }

    #[test]
    fn test_chord_creation() {
        let chord = c_major();
        assert_eq!(chord.len(), 3);
        assert!(chord.contains(&"C".parse().unwrap()));
        assert!(chord.contains(&"E".parse().unwrap()));
        assert!(chord.contains(&"G".parse().unwrap()));
        assert_eq!(chord.bass(), Some("C".parse().unwrap()));
    }

    #[test]
    fn test_chord_from_invalid_notes() {
        let result = Chord::from_note_strings(vec!["C", "X", "G"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_chord_transpose() {
        let c_maj = c_major();
        let d_maj = c_maj + 2;

        // Check that transposed chord contains the expected notes
        let d_maj_notes: Vec<u8> = d_maj.notes().map(|n| n.pitch_class()).collect();
        assert!(d_maj_notes.contains(&2)); // D
        assert!(d_maj_notes.contains(&6)); // F#
        assert!(d_maj_notes.contains(&9)); // A

        // Bass should also be transposed
        assert_eq!(d_maj.bass().unwrap().pitch_class(), 2); // D
    }

    #[test]
    fn test_chord_inversion() {
        let c_maj = c_major();

        // Test first inversion
        let first_inv = c_maj.clone().invert();
        assert_eq!(first_inv.bass(), Some("E".parse().unwrap())); // E should be in bass
        assert_eq!(first_inv.root(), Some("C".parse().unwrap())); // C should still be the root
        assert_eq!(first_inv.inversion(), 1); // Should be first inversion

        // Test second inversion
        let second_inv = first_inv.clone().invert();
        assert_eq!(second_inv.bass(), Some("G".parse().unwrap())); // G should be in bass
        assert_eq!(second_inv.root(), Some("C".parse().unwrap())); // C should still be the root
        assert_eq!(second_inv.inversion(), 2); // Should be second inversion

        // Test cycling back to root position
        let back_to_root = second_inv.invert();
        assert_eq!(back_to_root.bass(), Some("C".parse().unwrap())); // Back to C in bass
        assert_eq!(back_to_root.inversion(), 0); // Should be root position

        // Should still contain the same pitch classes
        assert_eq!(first_inv.len(), 3);
        assert!(first_inv.contains(&"C".parse().unwrap()));
        assert!(first_inv.contains(&"E".parse().unwrap()));
        assert!(first_inv.contains(&"G".parse().unwrap()));
    }

    #[test]
    fn test_chord_analysis() {
        let c_maj = c_major();
        let analysis = c_maj.analyze();
        assert!(analysis.contains("C Major"));

        let a_min = a_minor();
        let analysis = a_min.analyze();
        assert!(analysis.contains("A minor"));

        // Test inversion analysis
        let c_maj_first_inv = c_maj.invert();
        let analysis = c_maj_first_inv.analyze();
        assert!(analysis.contains("C Major"));
        assert!(analysis.contains("1st inv"));
    }

    #[test]
    fn test_set_operations() {
        let c_maj = c_major();
        let a_min = a_minor();

        // Common tones (intersection)
        let common = c_maj.clone() & a_min.clone();
        assert_eq!(common.len(), 2); // C and E
        assert!(common.contains(&"C".parse().unwrap()));
        assert!(common.contains(&"E".parse().unwrap()));

        // Union
        let union = c_maj.clone() | a_min.clone();
        assert_eq!(union.len(), 4); // C, E, G, A

        // Symmetric difference
        let diff = c_maj ^ a_min;
        assert_eq!(diff.len(), 2); // G and A
        assert!(diff.contains(&"G".parse().unwrap()));
        assert!(diff.contains(&"A".parse().unwrap()));
    }

    #[test]
    fn test_chord_display() {
        let c_maj = c_major();
        let display = format!("{}", c_maj);
        assert!(display.contains("C Major"));
        assert!(display.contains("[C, E, G]"));

        // Test inversion display
        let c_maj_first_inv = c_maj.invert();
        let display = format!("{}", c_maj_first_inv);
        assert!(display.contains("C Major"));
        assert!(display.contains("/E")); // Should show slash chord notation

        let empty = Chord::new();
        assert_eq!(format!("{}", empty), "[]");
    }

    #[test]
    fn test_root_and_bass() {
        let c_maj = c_major();
        assert_eq!(c_maj.root(), Some("C".parse().unwrap()));
        assert_eq!(c_maj.bass(), Some("C".parse().unwrap()));

        let notes_vec = c_maj.notes_vec();
        assert_eq!(notes_vec.len(), 3);
        assert_eq!(notes_vec[0], "C".parse().unwrap());
        assert_eq!(notes_vec[1], "E".parse().unwrap());
        assert_eq!(notes_vec[2], "G".parse().unwrap());

        // Test inversion
        let first_inv = c_maj.invert();
        assert_eq!(first_inv.root(), Some("C".parse().unwrap())); // Root stays the same
        assert_eq!(first_inv.bass(), Some("E".parse().unwrap())); // Bass changes
    }

    #[test]
    fn test_empty_chord() {
        let empty = Chord::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.root(), None);
        assert_eq!(empty.bass(), None);
    }

    #[test]
    fn test_with_bass() {
        let c_maj_over_e = Chord::with_bass(
            vec![
                "C".parse().unwrap(),
                "E".parse().unwrap(),
                "G".parse().unwrap(),
            ],
            "E".parse().unwrap(),
        );

        assert_eq!(c_maj_over_e.bass(), Some("E".parse().unwrap()));
        assert_eq!(c_maj_over_e.root(), Some("C".parse().unwrap()));
        assert_eq!(c_maj_over_e.inversion(), 1);
    }
}
