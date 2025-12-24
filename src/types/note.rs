use anyhow::{Result, anyhow};
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

/// Represents a musical note using chromatic representation (0-11)
/// 0=C, 1=C#/Db, 2=D, 3=D#/Eb, 4=E, 5=F, 6=F#/Gb, 7=G, 8=G#/Ab, 9=A, 10=A#/Bb, 11=B
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Note {
    pitch_class: u8, // 0-11 chromatic representation
    accidental_preference: AccidentalPreference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccidentalPreference {
    Sharp,
    Flat,
    Natural,
}

/// Standard 12-tone equal temperament frequencies for the 4th octave (C4-B4)
/// Based on A4 = 440Hz standard tuning
const BASE_OCTAVE_FREQUENCIES: [f32; 12] = [
    261.63, // C4
    277.18, // C#4/Db4
    293.66, // D4
    311.13, // D#4/Eb4
    329.63, // E4
    349.23, // F4
    369.99, // F#4/Gb4
    392.00, // G4
    415.30, // G#4/Ab4
    440.00, // A4
    466.16, // A#4/Bb4
    493.88, // B4
];

impl Note {
    /// Get the frequency for this note in the 4th octave (C4-B4 range)
    ///
    /// # Returns
    /// The frequency in Hz for this note's pitch class in the 4th octave
    pub fn frequency(&self) -> f32 {
        BASE_OCTAVE_FREQUENCIES[self.pitch_class() as usize]
    }

    /// Get the frequency for this note in a specific octave
    ///
    /// # Arguments
    /// * `octave` - The octave number (4 is middle octave, each octave doubles/halves frequency)
    ///
    /// # Returns
    /// The frequency in Hz for this note in the specified octave
    pub fn frequency_in_octave(&self, octave: i8) -> f32 {
        let base_frequency = self.frequency();
        let octave_multiplier = 2.0_f32.powi((octave - 4) as i32);
        base_frequency * octave_multiplier
    }
}

impl Note {
    /// Create a new note from chromatic pitch class (0-11)
    pub fn new(pitch_class: u8) -> Result<Self> {
        if pitch_class > 11 {
            return Err(anyhow!("Pitch class must be 0-11, got {}", pitch_class));
        }

        Ok(Note {
            pitch_class,
            accidental_preference: AccidentalPreference::Natural,
        })
    }

    /// Create a note with specific accidental preference
    pub fn with_accidental_preference(pitch_class: u8, sharp: bool) -> Result<Self> {
        if pitch_class > 11 {
            return Err(anyhow!("Pitch class must be 0-11, got {}", pitch_class));
        }

        let preference = if Self::is_natural_note(pitch_class) {
            AccidentalPreference::Natural
        } else if sharp {
            AccidentalPreference::Sharp
        } else {
            AccidentalPreference::Flat
        };

        Ok(Note {
            pitch_class,
            accidental_preference: preference,
        })
    }

    /// Get the chromatic pitch class (0-11)
    pub fn pitch_class(&self) -> u8 {
        self.pitch_class
    }

    /// Check if a pitch class corresponds to a natural note (white key)
    fn is_natural_note(pitch_class: u8) -> bool {
        matches!(pitch_class, 0 | 2 | 4 | 5 | 7 | 9 | 11) // C, D, E, F, G, A, B
    }

    /// Get the base note name for display purposes
    fn base_note_name(pitch_class: u8) -> &'static str {
        match pitch_class {
            0 => "C",
            2 => "D",
            4 => "E",
            5 => "F",
            7 => "G",
            9 => "A",
            11 => "B",
            _ => "", // Will be handled by accidental logic
        }
    }

    /// Get sharp representation for accidental notes
    fn sharp_name(pitch_class: u8) -> &'static str {
        match pitch_class {
            1 => "C#",
            3 => "D#",
            6 => "F#",
            8 => "G#",
            10 => "A#",
            _ => "",
        }
    }

    /// Get flat representation for accidental notes
    fn flat_name(pitch_class: u8) -> &'static str {
        match pitch_class {
            1 => "Db",
            3 => "Eb",
            6 => "Gb",
            8 => "Ab",
            10 => "Bb",
            _ => "",
        }
    }

    /// Transpose the note by a number of semitones
    pub fn transpose(self, semitones: i8) -> Note {
        let new_pitch_class = ((self.pitch_class as i8 + semitones).rem_euclid(12)) as u8;

        // When transposing, reset accidental preference appropriately
        let new_preference = if Self::is_natural_note(new_pitch_class) {
            AccidentalPreference::Natural
        } else {
            // Default to sharp for non-natural notes unless original was flat
            match self.accidental_preference {
                AccidentalPreference::Flat => AccidentalPreference::Sharp, // Could be smarter here
                _ => AccidentalPreference::Sharp,
            }
        };

        Note {
            pitch_class: new_pitch_class,
            accidental_preference: new_preference,
        }
    }

    // /// Transpose the note by a number of semitones
    // pub fn transpose(self, semitones: i8) -> Note {
    //     let new_pitch_class = ((self.pitch_class as i8 + semitones).rem_euclid(12)) as u8;
    //     Note {
    //         pitch_class: new_pitch_class,
    //         accidental_preference: self.accidental_preference,
    //     }
    // }
}

impl FromStr for Note {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim().to_uppercase();

        let (pitch_class, accidental_preference) = match s.as_str() {
            // Natural notes
            "C" => (0, AccidentalPreference::Natural),
            "D" => (2, AccidentalPreference::Natural),
            "E" => (4, AccidentalPreference::Natural),
            "F" => (5, AccidentalPreference::Natural),
            "G" => (7, AccidentalPreference::Natural),
            "A" => (9, AccidentalPreference::Natural),
            "B" => (11, AccidentalPreference::Natural),

            // Sharp notes
            "C#" | "CS" => (1, AccidentalPreference::Sharp),
            "D#" | "DS" => (3, AccidentalPreference::Sharp),
            "F#" | "FS" => (6, AccidentalPreference::Sharp),
            "G#" | "GS" => (8, AccidentalPreference::Sharp),
            "A#" | "AS" => (10, AccidentalPreference::Sharp),

            // Flat notes
            "DB" => (1, AccidentalPreference::Flat),
            "EB" => (3, AccidentalPreference::Flat),
            "GB" => (6, AccidentalPreference::Flat),
            "AB" => (8, AccidentalPreference::Flat),
            "BB" => (10, AccidentalPreference::Flat),

            _ => return Err(anyhow!("Invalid note name: {}", s)),
        };

        Ok(Note {
            pitch_class,
            accidental_preference,
        })
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.accidental_preference {
            AccidentalPreference::Natural => {
                if Self::is_natural_note(self.pitch_class) {
                    Self::base_note_name(self.pitch_class)
                } else {
                    // For non-natural notes with Natural preference, default to sharp
                    Self::sharp_name(self.pitch_class)
                }
            }
            AccidentalPreference::Sharp => {
                if Self::is_natural_note(self.pitch_class) {
                    Self::base_note_name(self.pitch_class)
                } else {
                    Self::sharp_name(self.pitch_class)
                }
            }
            AccidentalPreference::Flat => {
                if Self::is_natural_note(self.pitch_class) {
                    Self::base_note_name(self.pitch_class)
                } else {
                    Self::flat_name(self.pitch_class)
                }
            }
        };

        // Fallback to pitch class number if name is empty (shouldn't happen with fix above)
        if name.is_empty() {
            write!(f, "PC{}", self.pitch_class)
        } else {
            write!(f, "{}", name)
        }
    }
}
// Arithmetic operations for transposition
impl Add<i8> for Note {
    type Output = Note;

    fn add(self, semitones: i8) -> Self::Output {
        self.transpose(semitones)
    }
}

impl Sub<i8> for Note {
    type Output = Note;

    fn sub(self, semitones: i8) -> Self::Output {
        self.transpose(-semitones)
    }
}

// Calculate interval between two notes
impl Sub<Note> for Note {
    type Output = i8;

    fn sub(self, other: Note) -> Self::Output {
        let diff = (self.pitch_class as i8) - (other.pitch_class as i8);
        if diff < 0 { diff + 12 } else { diff }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_creation() {
        let c = Note::new(0).unwrap();
        assert_eq!(c.pitch_class(), 0);

        let invalid = Note::new(12);
        assert!(invalid.is_err());
    }

    #[test]
    fn test_note_parsing() {
        let c: Note = "C".parse().unwrap();
        assert_eq!(c.pitch_class(), 0);

        let cs: Note = "C#".parse().unwrap();
        assert_eq!(cs.pitch_class(), 1);

        let db: Note = "Db".parse().unwrap();
        assert_eq!(db.pitch_class(), 1);

        let invalid: Result<Note> = "H".parse();
        assert!(invalid.is_err());
    }

    #[test]
    fn test_note_display() {
        let c: Note = "C".parse().unwrap();
        assert_eq!(format!("{}", c), "C");

        let cs: Note = "C#".parse().unwrap();
        assert_eq!(format!("{}", cs), "C#");

        let db: Note = "Db".parse().unwrap();
        assert_eq!(format!("{}", db), "Db");
    }

    #[test]
    fn test_transposition() {
        let c: Note = "C".parse().unwrap();
        let d = c + 2;
        assert_eq!(d.pitch_class(), 2);

        let bb = c - 2;
        assert_eq!(bb.pitch_class(), 10);

        // Test wrapping
        let b: Note = "B".parse().unwrap();
        let c2 = b + 1;
        assert_eq!(c2.pitch_class(), 0);
    }

    #[test]
    fn test_interval_calculation() {
        let c: Note = "C".parse().unwrap();
        let e: Note = "E".parse().unwrap();
        assert_eq!(e - c, 4); // Major third

        let g: Note = "G".parse().unwrap();
        assert_eq!(g - c, 7); // Perfect fifth

        // Test descending interval
        assert_eq!(c - g, 5); // Perfect fourth (12 - 7)
    }

    #[test]
    fn test_accidental_preferences() {
        let cs = Note::with_accidental_preference(1, true).unwrap();
        assert_eq!(format!("{}", cs), "C#");

        let db = Note::with_accidental_preference(1, false).unwrap();
        assert_eq!(format!("{}", db), "Db");

        let c = Note::with_accidental_preference(0, true).unwrap();
        assert_eq!(format!("{}", c), "C"); // Natural notes ignore preference
    }
}
