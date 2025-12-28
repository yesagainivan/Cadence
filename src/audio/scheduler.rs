//! Scheduler for virtual time-based event dispatch
//!
//! Receives ScheduledEvents from the Interpreter (produced during execution with `wait`)
//! and dispatches them to AudioHandle at the correct real-time beats, synchronized
//! with the MasterClock.
//!
//! This enables non-blocking sequential playback: the interpreter runs to completion
//! immediately, scheduling all events, and the Scheduler dispatches them over time.

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::clock::ClockTick;
use cadence_core::types::{ScheduledAction, ScheduledEvent};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::thread;

/// Commands that can be sent to the scheduler
#[derive(Debug)]
pub enum SchedulerCommand {
    /// Add new scheduled events (with base beat for timing)
    Schedule(Vec<ScheduledEvent>, f64),
    /// Clear all pending events
    Clear,
    /// Shutdown the scheduler
    Shutdown,
}

/// Handle for sending commands to the scheduler thread
#[derive(Clone)]
pub struct SchedulerHandle {
    command_tx: Sender<SchedulerCommand>,
}

impl SchedulerHandle {
    /// Schedule events to be played starting at the given base beat
    pub fn schedule(&self, events: Vec<ScheduledEvent>, base_beat: f64) {
        let _ = self
            .command_tx
            .send(SchedulerCommand::Schedule(events, base_beat));
    }

    /// Clear all pending scheduled events
    pub fn clear(&self) {
        let _ = self.command_tx.send(SchedulerCommand::Clear);
    }

    /// Shutdown the scheduler thread
    pub fn shutdown(&self) {
        let _ = self.command_tx.send(SchedulerCommand::Shutdown);
    }
}

/// Scheduler that dispatches scheduled events at the correct beat times
pub struct Scheduler {
    /// Priority queue of scheduled events (min-heap by beat)
    event_queue: BinaryHeap<ScheduledEvent>,
    /// Audio handle for dispatching play commands
    audio_handle: Arc<AudioPlayerHandle>,
    /// Command receiver
    command_rx: Receiver<SchedulerCommand>,
    /// Clock tick receiver
    tick_rx: Receiver<ClockTick>,
}

impl Scheduler {
    /// Create a new scheduler that runs in its own thread
    /// Returns a handle for sending commands
    pub fn spawn(
        audio_handle: Arc<AudioPlayerHandle>,
        tick_rx: Receiver<ClockTick>,
    ) -> SchedulerHandle {
        let (command_tx, command_rx) = unbounded();

        let scheduler = Scheduler {
            event_queue: BinaryHeap::new(),
            audio_handle,
            command_rx,
            tick_rx,
        };

        thread::spawn(move || scheduler.run_loop());

        SchedulerHandle { command_tx }
    }

    /// Main scheduler loop
    fn run_loop(mut self) {
        loop {
            crossbeam_channel::select! {
                // Handle commands
                recv(self.command_rx) -> msg => match msg {
                    Ok(SchedulerCommand::Schedule(events, base_beat)) => {
                        self.schedule_events(events, base_beat);
                    }
                    Ok(SchedulerCommand::Clear) => {
                        self.event_queue.clear();
                    }
                    Ok(SchedulerCommand::Shutdown) => {
                        return;
                    }
                    Err(_) => {
                        // Channel closed, shutdown
                        return;
                    }
                },
                // Handle clock ticks
                recv(self.tick_rx) -> msg => match msg {
                    Ok(tick) => {
                        self.process_tick(&tick);
                    }
                    Err(_) => {
                        // Clock channel closed, shutdown
                        return;
                    }
                },
            }
        }
    }

    /// Add scheduled events, adjusting their beats by base_beat
    fn schedule_events(&mut self, events: Vec<ScheduledEvent>, base_beat: f64) {
        for mut event in events {
            // Convert virtual time to absolute clock time
            event.scheduled_beat += base_beat;
            self.event_queue.push(event);
        }
    }

    /// Process a clock tick and dispatch any due events
    fn process_tick(&mut self, tick: &ClockTick) {
        let current_beat = tick.beat;

        // Dispatch all events that are due (scheduled_beat <= current_beat)
        while let Some(event) = self.event_queue.peek() {
            if event.scheduled_beat <= current_beat {
                let event = self.event_queue.pop().unwrap();
                self.dispatch_event(&event);
            } else {
                break;
            }
        }
    }

    /// Dispatch a scheduled event to the audio system
    fn dispatch_event(&self, event: &ScheduledEvent) {
        match &event.action {
            ScheduledAction::PlayNotes {
                frequencies,
                duration_beats: _,
                drums,
            } => {
                // Filter out empty frequencies (rests)
                if !frequencies.is_empty() {
                    // Use trigger_note instead of set_track_notes for proper envelope attack
                    if let Err(e) = self
                        .audio_handle
                        .trigger_note(event.track_id, frequencies.clone())
                    {
                        eprintln!("Scheduler dispatch error: {}", e);
                    }
                }

                // Trigger any drum sounds
                for drum in drums {
                    if let Err(e) = self.audio_handle.play_drum(event.track_id, *drum) {
                        eprintln!("Scheduler drum error: {}", e);
                    }
                }
            }
            ScheduledAction::SetTempo(_bpm) => {
                // TODO: Send tempo change to clock
            }
            ScheduledAction::SetVolume(volume) => {
                let _ = self.audio_handle.set_track_volume(event.track_id, *volume);
            }
            ScheduledAction::Stop => {
                let _ = self.audio_handle.set_track_notes(event.track_id, vec![]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_scheduler_event_ordering() {
        // Test that BinaryHeap ordering works correctly for ScheduledEvents
        let mut heap = BinaryHeap::new();

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

        // Should pop in ascending order (earliest first due to reverse Ord impl)
        assert_eq!(heap.pop().unwrap().scheduled_beat, 0.0);
        assert_eq!(heap.pop().unwrap().scheduled_beat, 1.0);
        assert_eq!(heap.pop().unwrap().scheduled_beat, 2.0);
    }
}
