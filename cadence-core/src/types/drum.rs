//! Drum sound types and General MIDI mappings
//!
//! Provides `DrumSound` enum for percussion with TidalCycles-style naming
//! and GM MIDI note number mappings.

use std::fmt;

/// Percussion sound type with General MIDI mappings
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DrumSound {
    /// Bass drum / Kick (GM 36)
    Kick,
    /// Acoustic snare (GM 38)
    Snare,
    /// Closed hi-hat (GM 42)
    HiHat,
    /// Open hi-hat (GM 46)
    OpenHiHat,
    /// Hand clap (GM 39)
    Clap,
    /// Low tom (GM 45)
    Tom,
    /// Crash cymbal (GM 49)
    Crash,
    /// Ride cymbal (GM 51)
    Ride,
    /// Rimshot / Side stick (GM 37)
    Rim,
    /// Cowbell (GM 56)
    Cowbell,
}

impl DrumSound {
    /// Parse drum sound from string (TidalCycles-style names)
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            // Kick / Bass drum
            "kick" | "k" | "bd" | "bass" => Some(DrumSound::Kick),
            // Snare
            "snare" | "s" | "sn" | "sd" => Some(DrumSound::Snare),
            // Hi-hat (closed)
            "hihat" | "hh" | "h" | "ch" => Some(DrumSound::HiHat),
            // Hi-hat (open)
            "openhat" | "oh" | "ho" => Some(DrumSound::OpenHiHat),
            // Clap
            "clap" | "cp" | "cl" => Some(DrumSound::Clap),
            // Tom
            "tom" | "t" | "lt" => Some(DrumSound::Tom),
            // Crash
            "crash" | "cr" | "cc" => Some(DrumSound::Crash),
            // Ride
            "ride" | "rd" | "ri" => Some(DrumSound::Ride),
            // Rim / Side stick
            "rim" | "rm" | "rs" => Some(DrumSound::Rim),
            // Cowbell
            "cowbell" | "cb" | "cow" => Some(DrumSound::Cowbell),
            _ => None,
        }
    }

    /// Get the General MIDI percussion note number (channel 10)
    pub fn midi_note(&self) -> u8 {
        match self {
            DrumSound::Kick => 36,      // Bass Drum 1
            DrumSound::Snare => 38,     // Acoustic Snare
            DrumSound::HiHat => 42,     // Closed Hi-Hat
            DrumSound::OpenHiHat => 46, // Open Hi-Hat
            DrumSound::Clap => 39,      // Hand Clap
            DrumSound::Tom => 45,       // Low Tom
            DrumSound::Crash => 49,     // Crash Cymbal 1
            DrumSound::Ride => 51,      // Ride Cymbal 1
            DrumSound::Rim => 37,       // Side Stick
            DrumSound::Cowbell => 56,   // Cowbell
        }
    }

    /// Get short display name for the drum
    pub fn short_name(&self) -> &'static str {
        match self {
            DrumSound::Kick => "kick",
            DrumSound::Snare => "snare",
            DrumSound::HiHat => "hh",
            DrumSound::OpenHiHat => "oh",
            DrumSound::Clap => "clap",
            DrumSound::Tom => "tom",
            DrumSound::Crash => "crash",
            DrumSound::Ride => "ride",
            DrumSound::Rim => "rim",
            DrumSound::Cowbell => "cowbell",
        }
    }

    /// Get a "frequency" for visualization purposes (not actual pitch)
    /// Returns a pseudo-frequency that spreads drums across the piano roll
    pub fn display_frequency(&self) -> f32 {
        match self {
            DrumSound::Kick => 65.41,       // C2 - low
            DrumSound::Snare => 130.81,     // C3
            DrumSound::Clap => 146.83,      // D3
            DrumSound::Rim => 164.81,       // E3
            DrumSound::Tom => 174.61,       // F3
            DrumSound::HiHat => 261.63,     // C4 - middle
            DrumSound::OpenHiHat => 293.66, // D4
            DrumSound::Cowbell => 329.63,   // E4
            DrumSound::Crash => 392.00,     // G4
            DrumSound::Ride => 440.00,      // A4 - higher
        }
    }
}

impl fmt::Display for DrumSound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drum_parsing() {
        assert_eq!(DrumSound::from_name("kick"), Some(DrumSound::Kick));
        assert_eq!(DrumSound::from_name("k"), Some(DrumSound::Kick));
        assert_eq!(DrumSound::from_name("bd"), Some(DrumSound::Kick));
        assert_eq!(DrumSound::from_name("snare"), Some(DrumSound::Snare));
        assert_eq!(DrumSound::from_name("hh"), Some(DrumSound::HiHat));
        assert_eq!(DrumSound::from_name("KICK"), Some(DrumSound::Kick)); // case insensitive
        assert_eq!(DrumSound::from_name("invalid"), None);
    }

    #[test]
    fn test_midi_notes() {
        assert_eq!(DrumSound::Kick.midi_note(), 36);
        assert_eq!(DrumSound::Snare.midi_note(), 38);
        assert_eq!(DrumSound::HiHat.midi_note(), 42);
        assert_eq!(DrumSound::Clap.midi_note(), 39);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", DrumSound::Kick), "kick");
        assert_eq!(format!("{}", DrumSound::Snare), "snare");
    }
}
