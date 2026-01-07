use crate::types::note::Note;
use anyhow::Result;
#[cfg(feature = "colored")]
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
    /// When no octave is specified, notes are placed in ascending order starting from octave 4.
    /// For example, [F, A, C] becomes [F4, A4, C5] not [F4, A4, C4].
    pub fn from_note_strings(note_strings: Vec<&str>) -> Result<Self> {
        let mut notes = Vec::new();
        let mut last_pitch: Option<i16> = None; // Track last absolute pitch for ascending order
        let mut current_octave: i8 = 4;

        for note_str in note_strings {
            let parsed_note: Note = note_str.parse()?;

            // Check if an explicit octave was provided (look for digit in string)
            let has_explicit_octave = note_str.chars().any(|c| c.is_ascii_digit() || c == '-');

            if has_explicit_octave {
                // Use the explicit octave as-is
                notes.push(parsed_note);
                last_pitch =
                    Some(parsed_note.pitch_class() as i16 + (parsed_note.octave() as i16 * 12));
                current_octave = parsed_note.octave();
            } else {
                // No explicit octave - place in ascending order
                let this_pitch_class = parsed_note.pitch_class() as i16;

                if let Some(last) = last_pitch {
                    // Calculate what the absolute pitch would be at current octave
                    let this_pitch_at_current_octave =
                        this_pitch_class + (current_octave as i16 * 12);

                    // If this note would be at or below the last note, bump it up an octave
                    if this_pitch_at_current_octave <= last {
                        current_octave += 1;
                    }
                }

                // Create note with adjusted octave
                let adjusted_note =
                    Note::new_with_octave(parsed_note.pitch_class(), current_octave)?;
                let absolute_pitch = this_pitch_class + (current_octave as i16 * 12);
                notes.push(adjusted_note);
                last_pitch = Some(absolute_pitch);
            }
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
        self.input_order.to_vec()
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

    /// Normalize the chord to a target octave (default: 4)
    ///
    /// This shifts all notes so the bass note is in the target octave,
    /// preserving the relative positions of all voices.
    /// Useful after inversions to prevent octave drift.
    pub fn normalize_octave(self, target_octave: i8) -> Self {
        if let Some(bass) = self.bass_note {
            let current_octave = bass.octave();
            let shift = (target_octave - current_octave) * 12;
            self.transpose(shift)
        } else if let Some(first) = self.input_order.first() {
            let current_octave = first.octave();
            let shift = (target_octave - current_octave) * 12;
            self.transpose(shift)
        } else {
            self
        }
    }

    /// Create the first inversion of the chord
    pub fn invert(self) -> Self {
        self.invert_n(1)
    }

    /// Create the nth inversion of the chord
    pub fn invert_n(mut self, n: usize) -> Self {
        if self.notes.len() < 2 {
            return self;
        }

        // Use modulo to handle cases where n > chord length
        let steps = n % self.notes.len();

        if steps == 0 {
            // No inversion needed - return as is
            return self;
        }

        for _ in 0..steps {
            if !self.input_order.is_empty() {
                let note_to_move = self.input_order.remove(0);
                self.notes.remove(&note_to_move);

                let new_note = note_to_move + 12; // Transpose up 1 octave

                self.input_order.push(new_note);
                self.notes.insert(new_note);
            }
        }

        // Update bass note to the new first note
        self.bass_note = self.input_order.first().copied();

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

            format!("{} {}", root.name(), chord_quality)
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

            format!("{} {}{}", root.name(), chord_quality, inversion_info)
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

            format!("{} {}{}", root.name(), chord_quality, inversion_info)
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
#[cfg(feature = "colored")]
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
                "{}{}: [{}]",
                colored_analysis,
                bass_info,
                notes_str.join(", "),
            )
        } else {
            write!(f, "[{}]{}", notes_str.join(", "), bass_info)
        }
    }
}

// Plain Display impl for non-colored builds (WASM)
#[cfg(not(feature = "colored"))]
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "[]");
        }
        let notes_str: Vec<String> = self.input_order.iter().map(|n| n.to_string()).collect();
        let analysis = self.analyze();

        // Show bass note if different from root (slash chord notation)
        let bass_info = if let (Some(root), Some(bass)) = (self.root(), self.bass()) {
            if root != bass {
                format!("/{}", bass)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if analysis.contains("Major") || analysis.contains("minor") || analysis.contains("7th") {
            write!(f, "{}{}: [{}]", analysis, bass_info, notes_str.join(", "))
        } else {
            write!(f, "[{}]{}", notes_str.join(", "), bass_info)
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

    /// Intersection: common tones between chords (by pitch class, ignoring octave)
    fn bitand(self, other: Chord) -> Self::Output {
        // Find common pitch classes
        let self_pitch_classes: BTreeSet<u8> = self.notes.iter().map(|n| n.pitch_class()).collect();
        let other_pitch_classes: BTreeSet<u8> =
            other.notes.iter().map(|n| n.pitch_class()).collect();

        // Get notes from self that have matching pitch classes in other
        let common_notes: BTreeSet<Note> = self
            .notes
            .iter()
            .filter(|n| other_pitch_classes.contains(&n.pitch_class()))
            .copied()
            .collect();

        // Create input order from common notes, preserving left operand's order
        let mut input_order = Vec::new();
        for note in &self.input_order {
            if self_pitch_classes
                .intersection(&other_pitch_classes)
                .any(|&pc| pc == note.pitch_class())
            {
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

    /// Union: all pitch classes from both chords (uses left operand's octave for duplicates)
    fn bitor(self, other: Chord) -> Self::Output {
        // Collect pitch classes we've already seen (from self)
        let mut seen_pitch_classes: BTreeSet<u8> = BTreeSet::new();
        let mut result_notes: BTreeSet<Note> = BTreeSet::new();
        let mut input_order = Vec::new();

        // Add all notes from self
        for note in &self.input_order {
            if !seen_pitch_classes.contains(&note.pitch_class()) {
                seen_pitch_classes.insert(note.pitch_class());
                result_notes.insert(*note);
                input_order.push(*note);
            }
        }

        // Add notes from other that have new pitch classes
        for note in &other.input_order {
            if !seen_pitch_classes.contains(&note.pitch_class()) {
                seen_pitch_classes.insert(note.pitch_class());
                result_notes.insert(*note);
                input_order.push(*note);
            }
        }

        Chord {
            notes: result_notes,
            bass_note: self.bass_note.or(other.bass_note),
            input_order,
        }
    }
}

impl BitXor for Chord {
    type Output = Chord;

    /// Symmetric difference: pitch classes that are in one chord but not both
    fn bitxor(self, other: Chord) -> Self::Output {
        let self_pitch_classes: BTreeSet<u8> = self.notes.iter().map(|n| n.pitch_class()).collect();
        let other_pitch_classes: BTreeSet<u8> =
            other.notes.iter().map(|n| n.pitch_class()).collect();

        // Find pitch classes only in self
        let only_in_self: BTreeSet<u8> = self_pitch_classes
            .difference(&other_pitch_classes)
            .copied()
            .collect();
        // Find pitch classes only in other
        let only_in_other: BTreeSet<u8> = other_pitch_classes
            .difference(&self_pitch_classes)
            .copied()
            .collect();

        let mut result_notes: BTreeSet<Note> = BTreeSet::new();
        let mut input_order = Vec::new();

        // Add notes from self that are only in self
        for note in &self.input_order {
            if only_in_self.contains(&note.pitch_class()) {
                result_notes.insert(*note);
                input_order.push(*note);
            }
        }

        // Add notes from other that are only in other
        for note in &other.input_order {
            if only_in_other.contains(&note.pitch_class()) {
                result_notes.insert(*note);
                input_order.push(*note);
            }
        }

        Chord {
            notes: result_notes,
            bass_note: None,
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
        let original_bass = c_maj.bass();

        // Test that invert() changes the bass note
        let first_inv = c_maj.clone().invert();
        assert_ne!(first_inv.bass(), original_bass); // Bass should change
        assert_eq!(first_inv.root(), Some(Note::new_with_octave(0, 5).unwrap())); // Root should be C5

        // Test that inverting again changes bass again
        let second_inv = first_inv.clone().invert();
        assert_ne!(second_inv.bass(), first_inv.bass()); // Bass should change again

        // Should still contain the same pitch classes
        assert_eq!(first_inv.len(), 3);
        assert!(first_inv.contains(&"C5".parse().unwrap())); // C5 now
        assert!(first_inv.contains(&"E".parse().unwrap()));
        assert!(first_inv.contains(&"G".parse().unwrap()));

        assert_eq!(second_inv.len(), 3);
        assert!(second_inv.contains(&"C5".parse().unwrap()));
        assert!(second_inv.contains(&"E5".parse().unwrap()));
        assert!(second_inv.contains(&"G".parse().unwrap()));
    }

    #[test]
    fn test_chord_analysis() {
        let c_maj = c_major();
        let analysis = c_maj.analyze();
        assert!(analysis.contains("C Major"));

        let a_min = a_minor();
        let analysis = a_min.analyze();
        assert!(analysis.contains("A minor"));

        // Test that inverted chord still identifies as C Major
        let c_maj_first_inv = c_maj.invert();
        let analysis = c_maj_first_inv.analyze();
        assert!(analysis.contains("C Major"));
        // Note: inversion text may or may not appear depending on inversion() calculation
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
        // Note: exact bracket matching removed since Display uses ANSI colors
        assert!(display.contains("C"));
        assert!(display.contains("E"));
        assert!(display.contains("G"));

        // Test inversion display
        let c_maj_first_inv = c_maj.invert();
        let display = format!("{}", c_maj_first_inv);
        assert!(display.contains("C Major"));
        assert!(display.contains("/"));
        assert!(display.contains("E")); // Should show slash chord notation (ignoring ANSI)

        let empty = Chord::new();
        let empty_display = format!("{}", empty);
        // The display contains "[]" but may have ANSI color codes
        assert!(empty_display.len() >= 2); // At minimum contains []
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
        assert_eq!(first_inv.root(), Some(Note::new_with_octave(0, 5).unwrap())); // Root is C5
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
    fn test_physical_inversion() {
        // C Major in root position: C4, E4, G4
        let c_maj = Chord::from_note_strings(vec!["C4", "E4", "G4"]).unwrap();

        // Invert to first inversion: E4, G4, C5
        let inverted = c_maj.invert();
        let notes_vec = inverted.notes_vec();

        // Check ordering and octaves
        assert_eq!(notes_vec[0].pitch_class(), 4); // E
        assert_eq!(notes_vec[0].octave(), 4);

        assert_eq!(notes_vec[1].pitch_class(), 7); // G
        assert_eq!(notes_vec[1].octave(), 4);

        assert_eq!(notes_vec[2].pitch_class(), 0); // C
        assert_eq!(notes_vec[2].octave(), 5); // This should be C5, not C4

        assert_eq!(inverted.bass().unwrap().pitch_class(), 4); // E
        assert_eq!(inverted.bass().unwrap().octave(), 4);
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
