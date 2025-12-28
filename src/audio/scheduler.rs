//! Scheduler for virtual time-based event dispatch
//!
//! Receives ScheduledEvents from the Interpreter (produced during execution with `wait`)
//! and dispatches them to PlaybackEngines at the correct real-time beats, synchronized
//! with the MasterClock.
//!
//! This enables non-blocking sequential playback: the interpreter runs to completion
//! immediately, scheduling all events, and the Scheduler dispatches them over time.

use crate::audio::clock::ClockTick;
use cadence_core::types::ScheduledEvent;
use std::collections::BinaryHeap;

/// Scheduler that dispatches scheduled events at the correct beat times
pub struct Scheduler {
    /// Priority queue of scheduled events (min-heap by beat)
    event_queue: BinaryHeap<ScheduledEvent>,
    /// Base beat from when scheduling started (for relative timing)
    base_beat: f64,
    /// Whether the scheduler is active
    active: bool,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        Scheduler {
            event_queue: BinaryHeap::new(),
            base_beat: 0.0,
            active: false,
        }
    }

    /// Add scheduled events and start dispatching from the current beat
    ///
    /// The events have virtual times (beats) relative to when the script started.
    /// We convert these to absolute beat positions by adding the base_beat.
    pub fn schedule_events(&mut self, events: Vec<ScheduledEvent>, current_beat: f64) {
        self.base_beat = current_beat;
        self.active = !events.is_empty();

        for mut event in events {
            // Convert virtual time to absolute clock time
            event.scheduled_beat += current_beat;
            self.event_queue.push(event);
        }
    }

    /// Process a clock tick and dispatch any due events
    ///
    /// Returns the number of events dispatched
    pub fn process_tick<F>(&mut self, tick: &ClockTick, mut dispatch: F) -> usize
    where
        F: FnMut(&ScheduledEvent),
    {
        if !self.active || self.event_queue.is_empty() {
            return 0;
        }

        let current_beat = tick.beat;
        let mut dispatched = 0;

        // Dispatch all events that are due (scheduled_beat <= current_beat)
        // BinaryHeap is a max-heap, but our Ord implementation makes it a min-heap
        while let Some(event) = self.event_queue.peek() {
            if event.scheduled_beat <= current_beat {
                let event = self.event_queue.pop().unwrap();
                dispatch(&event);
                dispatched += 1;
            } else {
                // No more events due at this beat
                break;
            }
        }

        // Deactivate if queue is empty
        if self.event_queue.is_empty() {
            self.active = false;
        }

        dispatched
    }

    /// Check if there are pending events
    pub fn has_pending_events(&self) -> bool {
        self.active && !self.event_queue.is_empty()
    }

    /// Get the number of pending events
    pub fn pending_count(&self) -> usize {
        self.event_queue.len()
    }

    /// Clear all pending events
    pub fn clear(&mut self) {
        self.event_queue.clear();
        self.active = false;
    }

    /// Check if the scheduler is active
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadence_core::types::ScheduledAction;
    use std::time::Instant;

    fn make_tick(beat: f64) -> ClockTick {
        ClockTick {
            beat,
            beat_number: beat as u64,
            tick_in_beat: ((beat.fract() * 24.0) as u8) % 24,
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn test_scheduler_dispatches_at_correct_beat() {
        let mut scheduler = Scheduler::new();

        // Schedule events at virtual beats 0, 1, 2
        let events = vec![
            ScheduledEvent {
                scheduled_beat: 0.0,
                track_id: 1,
                action: ScheduledAction::SetTempo(120.0),
            },
            ScheduledEvent {
                scheduled_beat: 1.0,
                track_id: 1,
                action: ScheduledAction::SetTempo(130.0),
            },
            ScheduledEvent {
                scheduled_beat: 2.0,
                track_id: 1,
                action: ScheduledAction::SetTempo(140.0),
            },
        ];

        // Start scheduling at beat 10 (current clock position)
        scheduler.schedule_events(events, 10.0);

        assert!(scheduler.is_active());
        assert_eq!(scheduler.pending_count(), 3);

        // At beat 10, first event should dispatch
        let mut dispatched = Vec::new();
        let tick = make_tick(10.0);
        scheduler.process_tick(&tick, |e| dispatched.push(e.clone()));
        assert_eq!(dispatched.len(), 1);
        assert_eq!(scheduler.pending_count(), 2);

        // At beat 10.5, nothing should dispatch
        dispatched.clear();
        let tick = make_tick(10.5);
        scheduler.process_tick(&tick, |e| dispatched.push(e.clone()));
        assert_eq!(dispatched.len(), 0);

        // At beat 11, second event should dispatch
        dispatched.clear();
        let tick = make_tick(11.0);
        scheduler.process_tick(&tick, |e| dispatched.push(e.clone()));
        assert_eq!(dispatched.len(), 1);

        // At beat 12, third event should dispatch
        dispatched.clear();
        let tick = make_tick(12.0);
        scheduler.process_tick(&tick, |e| dispatched.push(e.clone()));
        assert_eq!(dispatched.len(), 1);

        // Scheduler should be inactive now
        assert!(!scheduler.is_active());
    }

    #[test]
    fn test_scheduler_dispatches_multiple_at_same_beat() {
        let mut scheduler = Scheduler::new();

        // Three events at the same beat
        let events = vec![
            ScheduledEvent {
                scheduled_beat: 0.0,
                track_id: 1,
                action: ScheduledAction::SetTempo(120.0),
            },
            ScheduledEvent {
                scheduled_beat: 0.0,
                track_id: 2,
                action: ScheduledAction::SetVolume(0.5),
            },
            ScheduledEvent {
                scheduled_beat: 0.0,
                track_id: 3,
                action: ScheduledAction::Stop,
            },
        ];

        scheduler.schedule_events(events, 0.0);

        let mut dispatched = Vec::new();
        let tick = make_tick(0.0);
        scheduler.process_tick(&tick, |e| dispatched.push(e.clone()));

        assert_eq!(dispatched.len(), 3);
        assert!(!scheduler.is_active());
    }

    #[test]
    fn test_scheduler_clear() {
        let mut scheduler = Scheduler::new();

        let events = vec![ScheduledEvent {
            scheduled_beat: 5.0,
            track_id: 1,
            action: ScheduledAction::Stop,
        }];

        scheduler.schedule_events(events, 0.0);
        assert!(scheduler.is_active());

        scheduler.clear();
        assert!(!scheduler.is_active());
        assert_eq!(scheduler.pending_count(), 0);
    }
}
