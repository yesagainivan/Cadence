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
use crate::audio::midi::{frequency_to_midi, MidiOutputHandle};
use crate::parser::{Evaluator, Expression, SharedEnvironment, Value};
use crate::types::{DrumSound, QueueMode, Waveform};
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
    /// Stereo pan position (0.0 = left, 0.5 = center, 1.0 = right)
    pub pan: Option<f32>,
    /// Duration of this step in beats (for fast/slow support)
    pub duration_beats: f32,
}

/// Unique identifier for a looping pattern
pub type PatternId = u64;

/// Configuration for a looping pattern (TidalCycles-style cycle tracking)
#[derive(Clone, Debug)]
pub struct LoopingPattern {
    /// Expression to evaluate each step (for reactive updates)
    pub expression: Expression,
    /// Environment for evaluation
    pub env: SharedEnvironment,
    /// Track ID
    pub track_id: usize,
    /// Beat when this pattern started (for calculating cycle position)
    pub start_beat: f64,
    /// Last step index we triggered (to detect transitions)
    pub last_triggered_step: Option<usize>,
    /// Cached pattern data: (total_steps, beats_per_cycle, envelope, waveform)
    pub cached_pattern_info: Option<(usize, f32, Option<(f32, f32, f32, f32)>, Option<Waveform>)>,
    /// Current cycle count (for EveryPattern alternation)
    pub current_cycle: usize,
    /// Cached beats per cycle for the current pattern (for Cycle queue mode)
    pub last_known_beats_per_cycle: f32,
}

impl LoopingPattern {
    pub fn new(
        expression: Expression,
        env: SharedEnvironment,
        track_id: usize,
        start_beat: f64,
    ) -> Self {
        Self {
            expression,
            env,
            track_id,
            start_beat,
            last_triggered_step: None,
            cached_pattern_info: None,
            current_cycle: 0,
            last_known_beats_per_cycle: 0.0,
        }
    }

    /// Calculate the current step index based on beat position
    /// Returns (step_index, is_new_step, playback_data) if we should trigger
    pub fn get_step_at_beat(
        &mut self,
        current_beat: f64,
    ) -> Result<Option<PlaybackStep>, anyhow::Error> {
        let evaluator = Evaluator::new();
        let env_guard = self.env.read().map_err(|e| anyhow::anyhow!("{}", e))?;
        let value = evaluator.eval_with_env(self.expression.clone(), Some(&env_guard))?;

        match value {
            Value::Note(note) => {
                // Single note: trigger once per beat
                let beats_elapsed = current_beat - self.start_beat;
                let current_step = beats_elapsed.floor() as usize;

                if self.last_triggered_step != Some(current_step) {
                    self.last_triggered_step = Some(current_step);
                    Ok(Some(PlaybackStep {
                        frequencies: vec![note.frequency()],
                        drums: vec![],
                        envelope: None,
                        waveform: None,
                        pan: None,
                        duration_beats: 1.0,
                    }))
                } else {
                    Ok(None)
                }
            }
            Value::Chord(chord) => {
                // Single chord: trigger once per beat
                let beats_elapsed = current_beat - self.start_beat;
                let current_step = beats_elapsed.floor() as usize;

                if self.last_triggered_step != Some(current_step) {
                    self.last_triggered_step = Some(current_step);
                    Ok(Some(PlaybackStep {
                        frequencies: chord.notes_vec().iter().map(|n| n.frequency()).collect(),
                        drums: vec![],
                        envelope: None,
                        waveform: None,
                        pan: None,
                        duration_beats: 1.0,
                    }))
                } else {
                    Ok(None)
                }
            }
            Value::Pattern(pattern) => {
                let events = pattern.to_rich_events();
                let beats_per_cycle = pattern.beats_per_cycle_f32();
                self.last_known_beats_per_cycle = beats_per_cycle;

                // Calculate position within the cycle
                let beats_elapsed = (current_beat - self.start_beat) as f32;
                let cycle_position = beats_elapsed % beats_per_cycle;

                // Find which step we're currently in
                let mut accumulated = 0.0f32;
                let mut current_step = 0;
                for (i, event) in events.iter().enumerate() {
                    let event_dur = event.duration_f32();
                    if cycle_position >= accumulated && cycle_position < accumulated + event_dur {
                        current_step = i;
                        break;
                    }
                    accumulated += event_dur;
                    // If we've gone past all events, we're in the last one
                    if i == events.len() - 1 {
                        current_step = i;
                    }
                }

                // Only trigger if this is a new step
                if self.last_triggered_step != Some(current_step) {
                    self.last_triggered_step = Some(current_step);

                    if current_step < events.len() {
                        let event = &events[current_step];
                        Ok(Some(PlaybackStep {
                            frequencies: event.notes.iter().map(|n| n.frequency).collect(),
                            drums: event.drums.clone(),
                            envelope: pattern.envelope,
                            waveform: pattern.waveform,
                            pan: pattern.pan,
                            duration_beats: event.duration_f32(),
                        }))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None) // Same step, don't re-trigger
                }
            }
            Value::String(s) => {
                if let Ok(pattern) = crate::types::Pattern::parse(&s) {
                    let events = pattern.to_rich_events();
                    let beats_per_cycle = pattern.beats_per_cycle_f32();
                    self.last_known_beats_per_cycle = beats_per_cycle;

                    let beats_elapsed = (current_beat - self.start_beat) as f32;
                    let cycle_position = beats_elapsed % beats_per_cycle;

                    let mut accumulated = 0.0f32;
                    let mut current_step = 0;
                    for (i, event) in events.iter().enumerate() {
                        let event_dur = event.duration_f32();
                        if cycle_position >= accumulated && cycle_position < accumulated + event_dur
                        {
                            current_step = i;
                            break;
                        }
                        accumulated += event_dur;
                        if i == events.len() - 1 {
                            current_step = i;
                        }
                    }

                    if self.last_triggered_step != Some(current_step) {
                        self.last_triggered_step = Some(current_step);

                        if current_step < events.len() {
                            let event = &events[current_step];
                            Ok(Some(PlaybackStep {
                                frequencies: event.notes.iter().map(|n| n.frequency).collect(),
                                drums: event.drums.clone(),
                                envelope: pattern.envelope,
                                waveform: pattern.waveform,
                                pan: pattern.pan,
                                duration_beats: event.duration_f32(),
                            }))
                        } else {
                            Ok(None)
                        }
                    } else {
                        Ok(None)
                    }
                } else {
                    Err(anyhow::anyhow!("Cannot play string"))
                }
            }
            Value::EveryPattern(every) => {
                // Get beats_per_cycle from the base pattern (both should have same duration)
                let beats_per_cycle = every.base.beats_per_cycle_f32();
                self.last_known_beats_per_cycle = beats_per_cycle;

                // Calculate position within the cycle FIRST
                let beats_elapsed = (current_beat - self.start_beat) as f32;
                let cycle_position = beats_elapsed % beats_per_cycle;

                // Calculate current cycle number BEFORE selecting pattern
                let new_cycle = (beats_elapsed / beats_per_cycle).floor() as usize;

                // Track cycle transitions - reset step tracking when cycle changes
                if new_cycle > self.current_cycle {
                    self.current_cycle = new_cycle;
                    self.last_triggered_step = None; // Reset to trigger first step of new cycle
                }

                // NOW select the appropriate pattern based on updated cycle
                let pattern = every.get_pattern_for_cycle(self.current_cycle);
                let events = pattern.to_rich_events();

                // Find which step we're currently in
                let mut accumulated = 0.0f32;
                let mut current_step = 0;
                for (i, event) in events.iter().enumerate() {
                    let event_dur = event.duration_f32();
                    if cycle_position >= accumulated && cycle_position < accumulated + event_dur {
                        current_step = i;
                        break;
                    }
                    accumulated += event_dur;
                    if i == events.len() - 1 {
                        current_step = i;
                    }
                }

                // Only trigger if this is a new step
                if self.last_triggered_step != Some(current_step) {
                    self.last_triggered_step = Some(current_step);

                    if current_step < events.len() {
                        let event = &events[current_step];
                        Ok(Some(PlaybackStep {
                            frequencies: event.notes.iter().map(|n| n.frequency).collect(),
                            drums: event.drums.clone(),
                            envelope: pattern.envelope,
                            waveform: pattern.waveform,
                            pan: pattern.pan,
                            duration_beats: event.duration_f32(),
                        }))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            _ => Err(anyhow::anyhow!("Cannot play this type")),
        }
    }
}

/// A pattern waiting to be activated on a track at a musically appropriate time
#[derive(Clone, Debug)]
pub struct PendingLoop {
    /// Unique identifier for this pending pattern
    pub id: PatternId,
    /// Expression to evaluate when activated
    pub expression: Expression,
    /// Environment for evaluation  
    pub env: SharedEnvironment,
    /// Queue synchronization mode
    pub queue_mode: QueueMode,
    /// Beat when this was queued (for timing calculations)
    pub queued_at_beat: f64,
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
    /// Queue a looping pattern to start at next musical boundary
    QueueLoop {
        id: PatternId,
        expression: Expression,
        env: SharedEnvironment,
        track_id: usize,
        queue_mode: QueueMode,
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

    /// Queue a looping pattern to start at the next musical boundary
    /// Returns the pattern ID (will be activated later based on queue_mode)
    pub fn queue_loop(
        &self,
        expression: Expression,
        env: SharedEnvironment,
        track_id: usize,
        queue_mode: QueueMode,
    ) -> PatternId {
        let id = self.next_pattern_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.command_tx.send(DispatcherCommand::QueueLoop {
            id,
            expression,
            env,
            track_id,
            queue_mode,
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
    /// Patterns waiting to be activated at a musical boundary (track_id -> pending)
    pending_loops: HashMap<usize, PendingLoop>,
    /// Audio handle
    audio_handle: Arc<AudioPlayerHandle>,
    /// Command receiver
    command_rx: Receiver<DispatcherCommand>,
    /// Clock tick receiver
    tick_rx: Receiver<ClockTick>,
    /// Current beat (for tracking)
    current_beat: f64,
    /// Last integer beat (for detecting beat boundaries)
    last_beat_floor: i64,
    /// Is running flag
    is_running: Arc<AtomicBool>,
    /// MIDI output handle (optional - for output mode checking and MIDI note sending)
    midi_handle: Option<Arc<MidiOutputHandle>>,
    /// Track active MIDI notes per track: track_id -> set of active note numbers
    /// Used to send note_off before note_on to prevent note stacking
    active_midi_notes: HashMap<usize, Vec<u8>>,
}

impl EventDispatcher {
    /// Create a new dispatcher that runs in its own thread
    pub fn spawn(
        audio_handle: Arc<AudioPlayerHandle>,
        tick_rx: Receiver<ClockTick>,
        midi_handle: Option<Arc<MidiOutputHandle>>,
    ) -> DispatcherHandle {
        let (command_tx, command_rx) = unbounded();
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();

        let dispatcher = EventDispatcher {
            event_queue: BinaryHeap::new(),
            active_loops: HashMap::new(),
            pending_loops: HashMap::new(),
            audio_handle,
            command_rx,
            tick_rx,
            current_beat: 0.0,
            last_beat_floor: -1,
            is_running: is_running_clone,
            midi_handle,
            active_midi_notes: HashMap::new(),
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

    /// Check if the active pattern on a track is at the start of a new cycle
    /// Used by QueueMode::Cycle to determine when to activate pending patterns
    fn active_pattern_at_cycle_start(&self, track_id: usize, current_beat: f64) -> bool {
        for pattern in self.active_loops.values() {
            if pattern.track_id == track_id && pattern.last_known_beats_per_cycle > 0.0 {
                let beats_elapsed = (current_beat - pattern.start_beat) as f32;
                let cycle_position = beats_elapsed % pattern.last_known_beats_per_cycle;
                // At cycle start if position is very small (within tolerance) and some time has passed
                if cycle_position < 0.05 && beats_elapsed > 0.0 {
                    return true;
                }
            }
        }
        false
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
                // Also cancel any pending loops on this track
                self.pending_loops.remove(&track_id);
                // Start new loop at current beat position
                self.active_loops.insert(
                    id,
                    LoopingPattern::new(expression, env, track_id, self.current_beat),
                );
            }
            DispatcherCommand::StopLoop(id) => {
                if let Some(pattern) = self.active_loops.remove(&id) {
                    // Clear the track's audio notes
                    let _ = self.audio_handle.set_track_notes(pattern.track_id, vec![]);
                    // Send MIDI note_off for any active notes on this track
                    if let Some(midi) = &self.midi_handle {
                        if let Some(notes) = self.active_midi_notes.remove(&pattern.track_id) {
                            for note in notes {
                                let _ = midi.note_off(pattern.track_id, note);
                            }
                        }
                    }
                }
            }
            DispatcherCommand::StopTrack(track_id) => {
                // Remove all loops on this track
                self.active_loops.retain(|_, p| p.track_id != track_id);
                // Remove any pending loops on this track
                self.pending_loops.remove(&track_id);
                // Clear scheduled events for this track
                let remaining: Vec<_> = self
                    .event_queue
                    .drain()
                    .filter(|e| e.track_id != track_id)
                    .collect();
                for event in remaining {
                    self.event_queue.push(event);
                }
                // Clear the track's audio notes
                let _ = self.audio_handle.set_track_notes(track_id, vec![]);
                // Send MIDI note_off for any active notes on this track
                if let Some(midi) = &self.midi_handle {
                    if let Some(notes) = self.active_midi_notes.remove(&track_id) {
                        for note in notes {
                            let _ = midi.note_off(track_id, note);
                        }
                    }
                }
            }
            DispatcherCommand::StopAll => {
                self.active_loops.clear();
                self.pending_loops.clear();
                self.event_queue.clear();
                // Send MIDI note_off for all active notes
                if let Some(midi) = &self.midi_handle {
                    for (track_id, notes) in self.active_midi_notes.drain() {
                        for note in notes {
                            let _ = midi.note_off(track_id, note);
                        }
                    }
                }
                // Clear all audio tracks (1-16)
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
                // Check output mode - only play internal audio if enabled
                let audio_enabled = self
                    .midi_handle
                    .as_ref()
                    .map_or(true, |h| h.audio_enabled());
                let midi_enabled = self
                    .midi_handle
                    .as_ref()
                    .map_or(false, |h| h.midi_enabled() && h.is_connected());

                if audio_enabled {
                    // Trigger internal synth
                    let _ = self.audio_handle.play();
                    if !frequencies.is_empty() {
                        let _ = self
                            .audio_handle
                            .trigger_note(track_id, frequencies.clone());
                    }
                    for drum in &drums {
                        let _ = self.audio_handle.play_drum(track_id, *drum);
                    }
                }

                if midi_enabled {
                    // Send MIDI notes
                    if let Some(midi) = &self.midi_handle {
                        for freq in &frequencies {
                            let midi_note = frequency_to_midi(*freq);
                            let _ = midi.note_on(track_id, midi_note, 100);
                        }
                    }
                }
            }
            DispatcherCommand::QueueLoop {
                id,
                expression,
                env,
                track_id,
                queue_mode,
            } => {
                // Store in pending_loops for this track (replaces any existing pending)
                self.pending_loops.insert(
                    track_id,
                    PendingLoop {
                        id,
                        expression,
                        env,
                        queue_mode,
                        queued_at_beat: self.current_beat,
                    },
                );
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

        // Track beat boundaries for queue mode calculations
        let current_beat_floor = tick.beat.floor() as i64;
        let is_beat_boundary = current_beat_floor > self.last_beat_floor;
        if is_beat_boundary {
            self.last_beat_floor = current_beat_floor;
        }

        // 1. Dispatch any due one-shot events
        while let Some(event) = self.event_queue.peek() {
            if event.scheduled_beat <= tick.beat {
                let event = self.event_queue.pop().unwrap();
                self.dispatch_event(&event);
            } else {
                break;
            }
        }

        // 2. Check pending loops for activation based on queue mode
        // Collect tracks that should activate their pending patterns
        let mut to_activate: Vec<usize> = Vec::new();

        for (track_id, pending) in &self.pending_loops {
            let should_activate = match pending.queue_mode {
                QueueMode::Beat => {
                    // Activate on next beat boundary after queuing
                    is_beat_boundary && tick.beat.floor() > pending.queued_at_beat.floor()
                }
                QueueMode::Bar => {
                    // Activate when beat is at bar boundary (0, 4, 8, 12...)
                    // Assuming 4/4 time signature
                    is_beat_boundary
                        && current_beat_floor % 4 == 0
                        && tick.beat > pending.queued_at_beat
                }
                QueueMode::Beats(n) => {
                    // Activate after exactly n beats from when it was queued
                    tick.beat >= pending.queued_at_beat + n as f64
                }
                QueueMode::Cycle => {
                    // Activate when the current pattern on this track completes a cycle
                    // If no active pattern, treat like Beat mode (activate on next beat)
                    if self.active_loops.values().any(|p| p.track_id == *track_id) {
                        self.active_pattern_at_cycle_start(*track_id, tick.beat)
                    } else {
                        is_beat_boundary && tick.beat.floor() > pending.queued_at_beat.floor()
                    }
                }
            };

            if should_activate {
                to_activate.push(*track_id);
            }
        }

        // Activate the pending patterns
        for track_id in to_activate {
            if let Some(pending) = self.pending_loops.remove(&track_id) {
                // Stop any existing loops on this track
                self.active_loops.retain(|_, p| p.track_id != track_id);
                // Start the new loop
                self.active_loops.insert(
                    pending.id,
                    LoopingPattern::new(
                        pending.expression,
                        pending.env,
                        track_id,
                        tick.beat, // Start at exactly this beat for precise timing
                    ),
                );
            }
        }

        // 2. Check looping patterns on EVERY tick (not just beat boundaries)
        // This enables fast() patterns to trigger at sub-beat intervals
        // The pattern tracks which step was last triggered and only fires when
        // the cycle position crosses into a new step.
        let mut updates: Vec<(usize, PlaybackStep)> = Vec::new();

        for pattern in self.active_loops.values_mut() {
            match pattern.get_step_at_beat(tick.beat) {
                Ok(Some(step)) => {
                    updates.push((pattern.track_id, step));
                }
                Ok(None) => {
                    // No new step to trigger (still in same step)
                }
                Err(e) => {
                    eprintln!("Loop evaluation error: {}", e);
                }
            }
        }

        // Apply updates
        for (track_id, step) in updates {
            // Check output mode - only play internal audio if enabled
            let audio_enabled = self
                .midi_handle
                .as_ref()
                .map_or(true, |h| h.audio_enabled());
            let midi_enabled = self
                .midi_handle
                .as_ref()
                .map_or(false, |h| h.midi_enabled() && h.is_connected());

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
            // Apply pan if present (enables reactive pan updates)
            if let Some(pan) = step.pan {
                let _ = self.audio_handle.set_track_pan(track_id, pan);
            }

            if audio_enabled {
                // Play internal synth
                let _ = self.audio_handle.play();
                if !step.frequencies.is_empty() {
                    let _ = self
                        .audio_handle
                        .trigger_note(track_id, step.frequencies.clone());
                }
                for drum in &step.drums {
                    let _ = self.audio_handle.play_drum(track_id, *drum);
                }
            }

            if midi_enabled {
                // Send note_off for previous notes on this track, then note_on for new notes
                if let Some(midi) = &self.midi_handle {
                    // First, send note_off for any previously active notes on this track
                    if let Some(prev_notes) = self.active_midi_notes.get(&track_id) {
                        for &note in prev_notes {
                            let _ = midi.note_off(track_id, note);
                        }
                    }

                    // Convert new frequencies to MIDI notes
                    let new_notes: Vec<u8> = step
                        .frequencies
                        .iter()
                        .map(|f| frequency_to_midi(*f))
                        .collect();

                    // Send note_on for new notes
                    for &note in &new_notes {
                        let _ = midi.note_on(track_id, note, 100);
                    }

                    // Store the new active notes
                    self.active_midi_notes.insert(track_id, new_notes);
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
                // Check output mode
                let audio_enabled = self
                    .midi_handle
                    .as_ref()
                    .map_or(true, |h| h.audio_enabled());
                let midi_enabled = self
                    .midi_handle
                    .as_ref()
                    .map_or(false, |h| h.midi_enabled() && h.is_connected());

                if audio_enabled {
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

                if midi_enabled {
                    if let Some(midi) = &self.midi_handle {
                        for freq in frequencies {
                            let midi_note = frequency_to_midi(*freq);
                            let _ = midi.note_on(event.track_id, midi_note, 100);
                        }
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

    /// Test Beat queue mode activates on next beat boundary
    #[test]
    fn test_queue_mode_beat_activation() {
        // Scenario: Pattern queued at beat 2.5, should activate at beat 3.0
        let queued_at_beat: f64 = 2.5;

        // At beat 2.7 (same beat floor as 2.5) - should NOT activate
        let current_beat: f64 = 2.7;
        let is_beat_boundary = false; // Still in beat 2
        let should_activate = is_beat_boundary && current_beat.floor() > queued_at_beat.floor();
        assert!(!should_activate, "Should not activate before beat boundary");

        // At beat 3.0 (new beat) - should activate
        let current_beat: f64 = 3.0;
        let is_beat_boundary = true; // Crossed to beat 3
        let should_activate = is_beat_boundary && current_beat.floor() > queued_at_beat.floor();
        assert!(should_activate, "Should activate at next beat boundary");
    }

    /// Test Bar queue mode activates on bar boundaries (beat 0, 4, 8, 12...)
    #[test]
    fn test_queue_mode_bar_activation() {
        // Scenario: Pattern queued at beat 1.5, should activate at beat 4.0
        let queued_at_beat: f64 = 1.5;

        // At beat 2.0 - is a beat boundary but not a bar
        let current_beat: f64 = 2.0;
        let current_beat_floor = 2i64;
        let is_beat_boundary = true;
        let should_activate =
            is_beat_boundary && current_beat_floor % 4 == 0 && current_beat > queued_at_beat;
        assert!(!should_activate, "Beat 2 is not a bar boundary");

        // At beat 3.0 - still not a bar
        let current_beat: f64 = 3.0;
        let current_beat_floor = 3i64;
        let should_activate =
            is_beat_boundary && current_beat_floor % 4 == 0 && current_beat > queued_at_beat;
        assert!(!should_activate, "Beat 3 is not a bar boundary");

        // At beat 4.0 - this IS a bar boundary (4 % 4 == 0)
        let current_beat: f64 = 4.0;
        let current_beat_floor = 4i64;
        let should_activate =
            is_beat_boundary && current_beat_floor % 4 == 0 && current_beat > queued_at_beat;
        assert!(should_activate, "Beat 4 is a bar boundary, should activate");

        // Test beat 0 is also a bar
        let current_beat: f64 = 0.0;
        let current_beat_floor = 0i64;
        let queued_at_beat: f64 = -0.5; // Queued before beat 0
        let should_activate =
            is_beat_boundary && current_beat_floor % 4 == 0 && current_beat > queued_at_beat;
        assert!(should_activate, "Beat 0 is a bar boundary");
    }

    /// Test Beats(n) queue mode activates after exactly n beats
    #[test]
    fn test_queue_mode_beats_n_activation() {
        // Scenario: Pattern queued at beat 2.0 with Beats(4), should activate at beat 6.0
        let queued_at_beat: f64 = 2.0;
        let n: u32 = 4;

        // At beat 5.9 - not yet 4 beats later
        let current_beat: f64 = 5.9;
        let should_activate = current_beat >= queued_at_beat + n as f64;
        assert!(
            !should_activate,
            "Only 3.9 beats elapsed, should not activate"
        );

        // At exactly beat 6.0 - exactly 4 beats later
        let current_beat: f64 = 6.0;
        let should_activate = current_beat >= queued_at_beat + n as f64;
        assert!(should_activate, "Exactly 4 beats elapsed, should activate");

        // At beat 6.5 - more than 4 beats later
        let current_beat: f64 = 6.5;
        let should_activate = current_beat >= queued_at_beat + n as f64;
        assert!(
            should_activate,
            "More than 4 beats elapsed, should activate"
        );
    }

    /// Test Beats(1) activates after exactly 1 beat
    #[test]
    fn test_queue_mode_beats_1_activation() {
        let queued_at_beat: f64 = 3.5;
        let n: u32 = 1;

        // At beat 4.4 - not yet 1 beat later
        let current_beat: f64 = 4.4;
        let should_activate = current_beat >= queued_at_beat + n as f64;
        assert!(!should_activate, "Only 0.9 beats elapsed");

        // At beat 4.5 - exactly 1 beat later
        let current_beat: f64 = 4.5;
        let should_activate = current_beat >= queued_at_beat + n as f64;
        assert!(should_activate, "Exactly 1 beat elapsed");
    }

    /// Test queue clearing on StopAll
    #[test]
    fn test_queue_modes_enum() {
        // Verify all queue modes are accessible
        let _ = QueueMode::Beat;
        let _ = QueueMode::Bar;
        let _ = QueueMode::Cycle;
        let _ = QueueMode::Beats(4);

        // Verify default is Beat
        assert!(matches!(QueueMode::default(), QueueMode::Beat));
    }

    /// Test Cycle mode boundary detection math
    #[test]
    fn test_queue_mode_cycle_boundary_detection() {
        // Scenario: Pattern "C D E F" (4 beats) playing on track 1
        // Queue another pattern with Cycle mode - should activate at cycle boundary
        let beats_per_cycle = 4.0f32;
        let start_beat = 0.0f64;

        // At beat 3.9 - NOT at cycle start
        let current_beat = 3.9f64;
        let beats_elapsed = (current_beat - start_beat) as f32;
        let cycle_position = beats_elapsed % beats_per_cycle;
        assert!(
            cycle_position > 0.1,
            "At beat 3.9, cycle_position {} should be > 0.1",
            cycle_position
        );

        // At beat 4.02 - IS at cycle start (within tolerance)
        let current_beat = 4.02f64;
        let beats_elapsed = (current_beat - start_beat) as f32;
        let cycle_position = beats_elapsed % beats_per_cycle;
        assert!(
            cycle_position < 0.05,
            "At beat 4.02, cycle_position {} should be < 0.05",
            cycle_position
        );

        // At beat 8.01 - IS at cycle start (second cycle)
        let current_beat = 8.01f64;
        let beats_elapsed = (current_beat - start_beat) as f32;
        let cycle_position = beats_elapsed % beats_per_cycle;
        assert!(
            cycle_position < 0.05,
            "At beat 8.01, cycle_position {} should be < 0.05",
            cycle_position
        );
    }

    /// Test cycle boundary with fast patterns
    #[test]
    fn test_queue_mode_cycle_with_fast_pattern() {
        // Pattern "C D".fast(2) has beats_per_cycle = 1.0 (not 2.0)
        let beats_per_cycle = 1.0f32;
        let start_beat = 0.5f64;

        // Cycles complete at beats 1.5, 2.5, 3.5...
        let current_beat = 1.51f64;
        let beats_elapsed = (current_beat - start_beat) as f32;
        let cycle_position = beats_elapsed % beats_per_cycle;
        assert!(
            cycle_position < 0.05,
            "At beat 1.51, should be at cycle start"
        );

        // At beat 1.8 - mid-cycle
        let current_beat = 1.8f64;
        let beats_elapsed = (current_beat - start_beat) as f32;
        let cycle_position = beats_elapsed % beats_per_cycle;
        assert!(cycle_position > 0.1, "At beat 1.8, should be mid-cycle");
    }
}
