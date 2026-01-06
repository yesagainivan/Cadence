//! Rich event types for visualization and playback.

use super::super::drum::DrumSound;
use super::super::note::Note;
use super::super::time::{to_f32, Time};

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
