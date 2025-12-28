//! Unified Event Dispatcher
//!
//! Consolidates Scheduler (for one-shot events) and PlaybackEngine (for loops)
//! into a single system that handles all audio playback.
//!
//! Architecture inspired by Sonic Pi and TidalCycles:
//! - Single scheduler handles ALL events with timestamps
//! - Looping patterns are tracked and stepped on beat boundaries
//! - No "fighting" between systems - one source of truth per track

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::clock::ClockTick;
use crate::parser::{Evaluator, Expression, SharedEnvironment, Value};
use crate::types::{DrumSound, Waveform};
use cadence_core::types::{ScheduledAction, ScheduledEvent};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Result from evaluating a pattern step - includes audio properties
#[derive(Clone, Debug)]
pub struct PlaybackStep {
    pub frequencies: Vec<f32>,
    pub drums: Vec<DrumSound>,
    pub envelope: Option<(f32, f32, f32, f32)>,
    pub waveform: Option<Waveform>,
    /// Duration of this step in beats (for fast/slow support)
    pub duration_beats: f32,
}

/// Unique identifier for a looping pattern
pub type PatternId = u64;

/// Configuration for a looping pattern
#[derive(Clone, Debug)]
pub struct LoopingPattern {
    /// Expression to evaluate each step (for reactive updates)
    pub expression: Expression,
    /// Environment for evaluation
    pub env: SharedEnvironment,
    /// Track ID
    pub track_id: usize,
    /// Current step index within the pattern
    pub step_index: usize,
    /// Total number of steps (cached from last evaluation)
    pub total_steps: usize,
    /// Current cycle number
    pub cycle: usize,
    /// Accumulated beat time since last step
    pub beat_accumulator: f32,
    /// Duration of current step in beats (cached)
    pub current_step_duration: f32,
}

impl LoopingPattern {
    pub fn new(expression: Expression, env: SharedEnvironment, track_id: usize) -> Self {
        Self {
            expression,
            env,
            track_id,
            step_index: 0,
            total_steps: 1,
            cycle: 0,
            // Start with 1.0 so first step triggers immediately on first tick
            beat_accumulator: 1.0,
            current_step_duration: 1.0, // Default: 1 beat per step
        }
    }

    /// Check if it's time to advance to the next step
    /// Returns true if enough beats have accumulated
    pub fn should_advance(&self) -> bool {
        self.beat_accumulator >= self.current_step_duration
    }

    /// Accumulate beat time (call this on each tick)
    pub fn accumulate(&mut self, beats: f32) {
        self.beat_accumulator += beats;
    }

    /// Reset accumulator after advancing
    pub fn reset_accumulator(&mut self) {
        // Keep fractional beats for accurate timing
        self.beat_accumulator -= self.current_step_duration;
        if self.beat_accumulator < 0.0 {
            self.beat_accumulator = 0.0;
        }
    }

    /// Evaluate the pattern and get the current step's audio data
    pub fn get_current_step(&mut self) -> Result<PlaybackStep, anyhow::Error> {
        let evaluator = Evaluator::new();
        let env_guard = self.env.read().map_err(|e| anyhow::anyhow!("{}", e))?;
        let value = evaluator.eval_with_env(self.expression.clone(), Some(&env_guard))?;

        match value {
            Value::Note(note) => {
                self.total_steps = 1;
                self.current_step_duration = 1.0;
                Ok(PlaybackStep {
                    frequencies: vec![note.frequency()],
                    drums: vec![],
                    envelope: None,
                    waveform: None,
                    duration_beats: 1.0,
                })
            }
            Value::Chord(chord) => {
                self.total_steps = 1;
                self.current_step_duration = 1.0;
                Ok(PlaybackStep {
                    frequencies: chord.notes_vec().iter().map(|n| n.frequency()).collect(),
                    drums: vec![],
                    envelope: None,
                    waveform: None,
                    duration_beats: 1.0,
                })
            }
            Value::Pattern(pattern) => {
                let events = pattern.to_rich_events();
                self.total_steps = events.len().max(1);
                let idx = self.step_index % self.total_steps;
                if idx < events.len() {
                    let event = &events[idx];
                    let freqs: Vec<f32> = event.notes.iter().map(|n| n.frequency).collect();
                    let duration = event.duration;
                    self.current_step_duration = duration;
                    Ok(PlaybackStep {
                        frequencies: freqs,
                        drums: event.drums.clone(),
                        envelope: pattern.envelope,
                        waveform: pattern.waveform,
                        duration_beats: duration,
                    })
                } else {
                    let step_duration = pattern.step_beats();
                    self.current_step_duration = step_duration;
                    Ok(PlaybackStep {
                        frequencies: vec![],
                        drums: vec![],
                        envelope: pattern.envelope,
                        waveform: pattern.waveform,
                        duration_beats: step_duration,
                    })
                }
            }
            Value::String(s) => {
                // Try parsing as pattern
                if let Ok(pattern) = crate::types::Pattern::parse(&s) {
                    let events = pattern.to_rich_events();
                    self.total_steps = events.len().max(1);
                    let idx = self.step_index % self.total_steps;
                    if idx < events.len() {
                        let event = &events[idx];
                        let freqs: Vec<f32> = event.notes.iter().map(|n| n.frequency).collect();
                        let duration = event.duration;
                        self.current_step_duration = duration;
                        Ok(PlaybackStep {
                            frequencies: freqs,
                            drums: event.drums.clone(),
                            envelope: pattern.envelope,
                            waveform: pattern.waveform,
                            duration_beats: duration,
                        })
                    } else {
                        let step_duration = pattern.step_beats();
                        self.current_step_duration = step_duration;
                        Ok(PlaybackStep {
                            frequencies: vec![],
                            drums: vec![],
                            envelope: pattern.envelope,
                            waveform: pattern.waveform,
                            duration_beats: step_duration,
                        })
                    }
                } else {
                    Err(anyhow::anyhow!("Cannot play string"))
                }
            }
            _ => Err(anyhow::anyhow!("Cannot play this type")),
        }
    }

    /// Advance to the next step
    pub fn advance(&mut self) {
        self.step_index += 1;
        if self.step_index >= self.total_steps && self.total_steps > 0 {
            self.step_index = 0;
            self.cycle += 1;
        }
    }
}

/// Commands that can be sent to the dispatcher
#[derive(Debug)]
pub enum DispatcherCommand {
    /// Schedule one-shot events (with base beat for timing)
    Schedule(Vec<ScheduledEvent>, f64),
    /// Start a looping pattern
    StartLoop {
        id: PatternId,
        expression: Expression,
        env: SharedEnvironment,
        track_id: usize,
    },
    /// Stop a looping pattern
    StopLoop(PatternId),
    /// Stop all patterns on a track
    StopTrack(usize),
    /// Stop all playback
    StopAll,
    /// Set track volume
    SetTrackVolume(usize, f32),
    /// Set track waveform
    SetTrackWaveform(usize, Waveform),
    /// Set track envelope (ADSR)
    SetTrackEnvelope(usize, Option<(f32, f32, f32, f32)>),
    /// Play a one-shot note immediately (no scheduling)
    TriggerImmediate {
        track_id: usize,
        frequencies: Vec<f32>,
        drums: Vec<DrumSound>,
    },
    /// Shutdown
    Shutdown,
}

/// Handle for sending commands to the dispatcher thread
#[derive(Clone)]
pub struct DispatcherHandle {
    command_tx: Sender<DispatcherCommand>,
    next_pattern_id: Arc<std::sync::atomic::AtomicU64>,
    is_running: Arc<AtomicBool>,
}

impl DispatcherHandle {
    /// Schedule events to be played starting at the given base beat
    pub fn schedule(&self, events: Vec<ScheduledEvent>, base_beat: f64) {
        let _ = self
            .command_tx
            .send(DispatcherCommand::Schedule(events, base_beat));
    }

    /// Start a new looping pattern, returns its ID
    pub fn start_loop(
        &self,
        expression: Expression,
        env: SharedEnvironment,
        track_id: usize,
    ) -> PatternId {
        let id = self.next_pattern_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.command_tx.send(DispatcherCommand::StartLoop {
            id,
            expression,
            env,
            track_id,
        });
        id
    }

    /// Stop a specific looping pattern
    pub fn stop_loop(&self, id: PatternId) {
        let _ = self.command_tx.send(DispatcherCommand::StopLoop(id));
    }

    /// Stop all patterns on a track
    pub fn stop_track(&self, track_id: usize) {
        let _ = self.command_tx.send(DispatcherCommand::StopTrack(track_id));
    }

    /// Stop all playback
    pub fn stop_all(&self) {
        let _ = self.command_tx.send(DispatcherCommand::StopAll);
    }

    /// Trigger a note immediately (for simple one-shot plays)
    pub fn trigger_immediate(&self, track_id: usize, frequencies: Vec<f32>, drums: Vec<DrumSound>) {
        let _ = self.command_tx.send(DispatcherCommand::TriggerImmediate {
            track_id,
            frequencies,
            drums,
        });
    }

    /// Set track volume
    pub fn set_track_volume(&self, track_id: usize, volume: f32) {
        let _ = self
            .command_tx
            .send(DispatcherCommand::SetTrackVolume(track_id, volume));
    }

    /// Set track waveform
    pub fn set_track_waveform(&self, track_id: usize, waveform: Waveform) {
        let _ = self
            .command_tx
            .send(DispatcherCommand::SetTrackWaveform(track_id, waveform));
    }

    /// Set track envelope (ADSR)
    pub fn set_track_envelope(&self, track_id: usize, envelope: Option<(f32, f32, f32, f32)>) {
        let _ = self
            .command_tx
            .send(DispatcherCommand::SetTrackEnvelope(track_id, envelope));
    }

    /// Shutdown the dispatcher
    pub fn shutdown(&self) {
        let _ = self.command_tx.send(DispatcherCommand::Shutdown);
    }

    /// Check if the dispatcher is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
}

/// Unified event dispatcher
pub struct EventDispatcher {
    /// Priority queue of scheduled one-shot events (min-heap by beat)
    event_queue: BinaryHeap<ScheduledEvent>,
    /// Currently active looping patterns
    active_loops: HashMap<PatternId, LoopingPattern>,
    /// Audio handle
    audio_handle: Arc<AudioPlayerHandle>,
    /// Command receiver
    command_rx: Receiver<DispatcherCommand>,
    /// Clock tick receiver
    tick_rx: Receiver<ClockTick>,
    /// Current beat (for tracking)
    current_beat: f64,
    /// Is running flag
    is_running: Arc<AtomicBool>,
    /// Last beat when loops were stepped (to avoid double-stepping)
    last_loop_beat: u64,
}

impl EventDispatcher {
    /// Create a new dispatcher that runs in its own thread
    pub fn spawn(
        audio_handle: Arc<AudioPlayerHandle>,
        tick_rx: Receiver<ClockTick>,
    ) -> DispatcherHandle {
        let (command_tx, command_rx) = unbounded();
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();

        let dispatcher = EventDispatcher {
            event_queue: BinaryHeap::new(),
            active_loops: HashMap::new(),
            audio_handle,
            command_rx,
            tick_rx,
            current_beat: 0.0,
            is_running: is_running_clone,
            last_loop_beat: u64::MAX, // Start with MAX so first beat triggers
        };

        thread::spawn(move || dispatcher.run_loop());

        DispatcherHandle {
            command_tx,
            next_pattern_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            is_running,
        }
    }

    /// Main dispatcher loop
    fn run_loop(mut self) {
        loop {
            crossbeam_channel::select! {
                recv(self.command_rx) -> msg => match msg {
                    Ok(cmd) => {
                        if !self.handle_command(cmd) {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                recv(self.tick_rx) -> msg => match msg {
                    Ok(tick) => {
                        self.process_tick(&tick);
                    }
                    Err(_) => break,
                },
            }
        }

        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Handle a command, returns false if should shutdown
    fn handle_command(&mut self, cmd: DispatcherCommand) -> bool {
        match cmd {
            DispatcherCommand::Schedule(events, base_beat) => {
                for mut event in events {
                    event.scheduled_beat += base_beat;
                    self.event_queue.push(event);
                }
            }
            DispatcherCommand::StartLoop {
                id,
                expression,
                env,
                track_id,
            } => {
                // Stop any existing loops on this track first
                self.active_loops.retain(|_, p| p.track_id != track_id);
                self.active_loops
                    .insert(id, LoopingPattern::new(expression, env, track_id));
            }
            DispatcherCommand::StopLoop(id) => {
                if let Some(pattern) = self.active_loops.remove(&id) {
                    // Clear the track's notes
                    let _ = self.audio_handle.set_track_notes(pattern.track_id, vec![]);
                }
            }
            DispatcherCommand::StopTrack(track_id) => {
                // Remove all loops on this track
                self.active_loops.retain(|_, p| p.track_id != track_id);
                // Clear scheduled events for this track
                let remaining: Vec<_> = self
                    .event_queue
                    .drain()
                    .filter(|e| e.track_id != track_id)
                    .collect();
                for event in remaining {
                    self.event_queue.push(event);
                }
                // Clear the track's notes
                let _ = self.audio_handle.set_track_notes(track_id, vec![]);
            }
            DispatcherCommand::StopAll => {
                self.active_loops.clear();
                self.event_queue.clear();
                // Clear all tracks (1-16)
                for track_id in 1..=16 {
                    let _ = self.audio_handle.set_track_notes(track_id, vec![]);
                }
            }
            DispatcherCommand::SetTrackVolume(track_id, volume) => {
                let _ = self.audio_handle.set_track_volume(track_id, volume);
            }
            DispatcherCommand::SetTrackWaveform(track_id, waveform) => {
                let _ = self.audio_handle.set_track_waveform(track_id, waveform);
            }
            DispatcherCommand::SetTrackEnvelope(track_id, envelope) => {
                let _ = self.audio_handle.set_track_envelope(track_id, envelope);
            }
            DispatcherCommand::TriggerImmediate {
                track_id,
                frequencies,
                drums,
            } => {
                // Trigger immediately without scheduling
                // Ensure audio is playing
                let _ = self.audio_handle.play();
                if !frequencies.is_empty() {
                    let _ = self.audio_handle.trigger_note(track_id, frequencies);
                }
                for drum in drums {
                    let _ = self.audio_handle.play_drum(track_id, drum);
                }
            }
            DispatcherCommand::Shutdown => {
                return false;
            }
        }
        true
    }

    /// Process a clock tick
    fn process_tick(&mut self, tick: &ClockTick) {
        self.current_beat = tick.beat;

        // 1. Dispatch any due one-shot events
        while let Some(event) = self.event_queue.peek() {
            if event.scheduled_beat <= tick.beat {
                let event = self.event_queue.pop().unwrap();
                self.dispatch_event(&event);
            } else {
                break;
            }
        }

        // 2. Step looping patterns based on beat accumulation
        // This supports fast() and slow() by tracking accumulated beat time
        if tick.beat_number != self.last_loop_beat && tick.is_beat_boundary() {
            self.last_loop_beat = tick.beat_number;

            // Accumulate 1 beat for all active loops
            for pattern in self.active_loops.values_mut() {
                pattern.accumulate(1.0);
            }

            // Collect pattern updates - may have multiple per pattern for fast() patterns
            let mut updates: Vec<(usize, PlaybackStep)> = Vec::new();

            for pattern in self.active_loops.values_mut() {
                // Process as many steps as needed (for fast patterns that have
                // multiple steps per beat)
                while pattern.should_advance() {
                    match pattern.get_current_step() {
                        Ok(step) => {
                            updates.push((pattern.track_id, step));
                            pattern.advance();
                            pattern.reset_accumulator();
                        }
                        Err(e) => {
                            eprintln!("Loop evaluation error: {}", e);
                            break;
                        }
                    }
                }
            }

            // Apply updates
            for (track_id, step) in updates {
                // Apply envelope if present (enables reactive envelope updates)
                if let Some(envelope) = step.envelope {
                    let _ = self
                        .audio_handle
                        .set_track_envelope(track_id, Some(envelope));
                }
                // Apply waveform if present (enables reactive waveform updates)
                if let Some(waveform) = step.waveform {
                    let _ = self.audio_handle.set_track_waveform(track_id, waveform);
                }
                // Ensure audio is playing
                let _ = self.audio_handle.play();
                if !step.frequencies.is_empty() {
                    let _ = self.audio_handle.trigger_note(track_id, step.frequencies);
                }
                for drum in step.drums {
                    let _ = self.audio_handle.play_drum(track_id, drum);
                }
            }
        }
    }

    /// Dispatch a one-shot scheduled event
    fn dispatch_event(&self, event: &ScheduledEvent) {
        match &event.action {
            ScheduledAction::PlayNotes {
                frequencies, drums, ..
            } => {
                // Ensure audio is playing
                let _ = self.audio_handle.play();
                if !frequencies.is_empty() {
                    let _ = self
                        .audio_handle
                        .trigger_note(event.track_id, frequencies.clone());
                }
                for drum in drums {
                    if let Err(e) = self.audio_handle.play_drum(event.track_id, *drum) {
                        eprintln!("Drum error: {}", e);
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

    #[test]
    fn test_dispatcher_command_variants() {
        // Just ensure the command enum compiles with all variants
        let _ = DispatcherCommand::Shutdown;
        let _ = DispatcherCommand::StopAll;
        let _ = DispatcherCommand::StopTrack(1);
    }
}
