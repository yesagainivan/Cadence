//! Scheduled event types for virtual time based playback
//!
//! This module provides types for scheduling musical events at specific
//! virtual time points, inspired by Sonic Pi's non-blocking sleep model.

use crate::types::DrumSound;

/// An event scheduled for a specific virtual time (in beats)
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledEvent {
    /// When to execute (in beats from schedule start)
    pub scheduled_beat: f64,
    /// The action to perform
    pub action: ScheduledAction,
    /// Track ID (for routing)
    pub track_id: usize,
}

impl ScheduledEvent {
    /// Create a new scheduled event
    pub fn new(scheduled_beat: f64, action: ScheduledAction, track_id: usize) -> Self {
        Self {
            scheduled_beat,
            action,
            track_id,
        }
    }
}

impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse order for min-heap behavior (earliest first)
        other
            .scheduled_beat
            .partial_cmp(&self.scheduled_beat)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Eq for ScheduledEvent {}

/// Actions that can be scheduled for future execution
#[derive(Debug, Clone, PartialEq)]
pub enum ScheduledAction {
    /// Play notes at this moment
    PlayNotes {
        /// Frequencies to play (Hz)
        frequencies: Vec<f32>,
        /// Duration of this event in beats  
        duration_beats: f32,
        /// Optional drum sounds to trigger
        drums: Vec<DrumSound>,
    },
    /// Set tempo at this moment
    SetTempo(f32),
    /// Set volume at this moment
    SetVolume(f32),
    /// Stop playback
    Stop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BinaryHeap;

    #[test]
    fn test_scheduled_event_ordering() {
        let mut heap = BinaryHeap::new();

        // Add events in random order
        heap.push(ScheduledEvent::new(
            2.0,
            ScheduledAction::SetTempo(120.0),
            1,
        ));
        heap.push(ScheduledEvent::new(
            0.0,
            ScheduledAction::PlayNotes {
                frequencies: vec![440.0],
                duration_beats: 1.0,
                drums: vec![],
            },
            1,
        ));
        heap.push(ScheduledEvent::new(1.0, ScheduledAction::Stop, 1));

        // Should pop in ascending order (earliest first)
        assert_eq!(heap.pop().unwrap().scheduled_beat, 0.0);
        assert_eq!(heap.pop().unwrap().scheduled_beat, 1.0);
        assert_eq!(heap.pop().unwrap().scheduled_beat, 2.0);
    }
}
