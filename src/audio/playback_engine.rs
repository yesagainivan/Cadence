//! Beat-quantized progression playback engine
//!
//! This module provides a production-ready audio progression system that enables
//! seamless, beat-synchronized switching between progressions—inspired by live
//! coding environments like Sonic Pi and TidalCycles.
//!
//! ## Synchronization
//! All tracks receive tick events from a shared MasterClock, ensuring they stay
//! perfectly in sync regardless of how many tracks are playing.

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::clock::{ClockTick, Duration};
use crate::audio::midi::{frequency_to_midi, MidiOutputHandle};
use crate::parser::{Evaluator, Expression, SharedEnvironment, Value};
use crate::types::Waveform;
use anyhow::Result;
use crossbeam_channel::{select, Receiver as CrossbeamReceiver, Sender as CrossbeamSender};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Epsilon for floating-point beat comparisons (half a tick at 24 PPQN)
const TICK_EPSILON: f64 = 1.0 / 48.0;

/// Source of playback content - supports both static and reactive (live-coding) modes
#[derive(Clone, Debug)]
pub enum PlaybackSource {
    /// Static frequencies - already evaluated, won't change during playback
    Static(Vec<Vec<f32>>),
    /// Reactive expression - re-evaluated each beat for live-coding reactivity
    /// When the variable is reassigned, the playing audio will update on the next beat
    Reactive {
        expression: Expression,
        env: SharedEnvironment,
    },
}

impl PlaybackSource {
    /// Get the current frequencies and durations by evaluating the source
    /// For Static sources, returns the stored frequencies with default duration 1.0
    /// For Reactive sources, re-evaluates the expression against the current environment
    /// Returns Vec<(frequencies, duration_in_beats)>
    pub fn evaluate(&self) -> Result<Vec<(Vec<f32>, f32)>> {
        match self {
            PlaybackSource::Static(freqs) => {
                // Static sources get default duration of 1.0 beat each
                Ok(freqs.iter().map(|f| (f.clone(), 1.0)).collect())
            }
            PlaybackSource::Reactive { expression, env } => {
                let evaluator = Evaluator::new();
                let env_guard = env
                    .read()
                    .map_err(|e| anyhow::anyhow!("Environment lock poisoned: {}", e))?;
                let value = evaluator.eval_with_env(expression.clone(), Some(&env_guard))?;
                Self::value_to_frequencies(&value)
            }
        }
    }

    /// Convert a Value to a vector of (frequency vectors, duration) tuples
    fn value_to_frequencies(value: &Value) -> Result<Vec<(Vec<f32>, f32)>> {
        match value {
            Value::Note(note) => Ok(vec![(vec![note.frequency()], 1.0)]),
            Value::String(s) => {
                // Try to parse string as a pattern
                if let Ok(pattern) = crate::types::Pattern::parse(s) {
                    Self::value_to_frequencies(&Value::Pattern(pattern))
                } else {
                    Err(anyhow::anyhow!("Cannot play a string \"{}\"", s))
                }
            }
            Value::Chord(chord) => Ok(vec![(
                chord.notes_vec().iter().map(|n| n.frequency()).collect(),
                1.0,
            )]),
            Value::Pattern(pattern) => {
                // Convert pattern to frequencies with per-event durations
                // Groups subdivide their time slot, so [C D] E gives C 0.5 beats, D 0.5 beats, E 1 beat
                Ok(pattern
                    .to_events()
                    .into_iter()
                    .map(|(freqs, duration, is_rest)| {
                        if is_rest {
                            (vec![], duration) // Empty = silence for this step, but keep duration
                        } else {
                            (freqs, duration)
                        }
                    })
                    .collect())
            }
            Value::Boolean(_) => Err(anyhow::anyhow!("Cannot play a boolean value")),
            Value::Number(_) => Err(anyhow::anyhow!("Cannot play a raw number")),
            Value::Function { name, .. } => Err(anyhow::anyhow!(
                "Cannot play a function '{}' - call it first",
                name
            )),
        }
    }

    /// Get the envelope parameters from the source (if it's a pattern with envelope)
    pub fn get_envelope(&self) -> Result<Option<(f32, f32, f32, f32)>> {
        match self {
            PlaybackSource::Static(_) => Ok(None),
            PlaybackSource::Reactive { expression, env } => {
                let evaluator = Evaluator::new();
                let env_guard = env
                    .read()
                    .map_err(|e| anyhow::anyhow!("Environment lock poisoned: {}", e))?;
                let value = evaluator.eval_with_env(expression.clone(), Some(&env_guard))?;
                match value {
                    Value::Pattern(pattern) => Ok(pattern.envelope),
                    _ => Ok(None),
                }
            }
        }
    }

    /// Get the waveform from the source (if it's a pattern with waveform)
    pub fn get_waveform(&self) -> Result<Option<Waveform>> {
        match self {
            PlaybackSource::Static(_) => Ok(None),
            PlaybackSource::Reactive { expression, env } => {
                let evaluator = Evaluator::new();
                let env_guard = env
                    .read()
                    .map_err(|e| anyhow::anyhow!("Environment lock poisoned: {}", e))?;
                let value = evaluator.eval_with_env(expression.clone(), Some(&env_guard))?;
                match value {
                    Value::Pattern(pattern) => Ok(pattern.waveform),
                    _ => Ok(None),
                }
            }
        }
    }

    /// Get the number of events in this source (evaluates if reactive)
    pub fn len(&self) -> Result<usize> {
        Ok(self.evaluate()?.len())
    }

    /// Check if the source is empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.evaluate()?.is_empty())
    }

    /// Update a variable in the reactive environment (no-op for Static sources)
    /// Used for injecting time/state like `_cycle` into the evaluator
    pub fn update_environment(&self, name: &str, value: Value) -> Result<()> {
        if let PlaybackSource::Reactive { env, .. } = self {
            let mut guard = env
                .write()
                .map_err(|e| anyhow::anyhow!("Environment lock poisoned: {}", e))?;

            if guard.set(name, value.clone()).is_err() {
                guard.define(name.to_string(), value);
            }
        }
        Ok(())
    }
}

/// Configuration for progression playback
#[derive(Clone, Debug)]
pub struct ProgressionConfig {
    /// Source of the progression (static frequencies or reactive expression)
    pub source: PlaybackSource,
    /// How long each chord plays (in beats)
    pub note_duration: Duration,
    /// Gap between chords (default: 0, seamless transition)
    pub gap_duration: Duration,
    /// Number of times to loop (None = infinite loop)
    pub loop_count: Option<usize>,
}

impl ProgressionConfig {
    /// Create a new progression config with static frequencies (legacy API)
    pub fn new(progression: Vec<Vec<f32>>) -> Self {
        Self {
            source: PlaybackSource::Static(progression),
            note_duration: Duration::Beats(1.0),
            gap_duration: Duration::Beats(0.0),
            loop_count: Some(1),
        }
    }

    /// Create a new progression config with a reactive expression
    /// The expression will be re-evaluated on each beat, enabling live variable updates
    pub fn new_reactive(expression: Expression, env: SharedEnvironment) -> Self {
        Self {
            source: PlaybackSource::Reactive { expression, env },
            note_duration: Duration::Beats(1.0),
            gap_duration: Duration::Beats(0.0),
            loop_count: Some(1),
        }
    }

    /// Set the note duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.note_duration = duration;
        self
    }

    /// Set gap between chords
    pub fn with_gap(mut self, gap: Duration) -> Self {
        self.gap_duration = gap;
        self
    }

    /// Enable looping (infinite)
    pub fn with_looping(mut self) -> Self {
        self.loop_count = None;
        self
    }

    /// Set specific loop count
    pub fn with_loop_count(mut self, count: usize) -> Self {
        self.loop_count = Some(count);
        self
    }
}

// QueueMode is now defined in types::audio_config for WASM compatibility
pub use crate::types::QueueMode;

/// Commands that can be sent to the playback engine
#[derive(Debug)]
pub enum PlaybackCommand {
    /// Start playing a progression immediately (interrupts current)
    PlayProgression(ProgressionConfig),
    /// Queue a progression to start at the specified sync point
    QueueProgression(ProgressionConfig, QueueMode),
    /// Stop playback immediately
    Stop,
    /// Set volume for this track
    SetVolume(f32),
    /// Shutdown the playback engine
    Shutdown,
}

/// Engine for managing sequential progression playback with beat-quantized switching
///
/// Receives tick events from a shared MasterClock to stay synchronized with other tracks.
pub struct PlaybackEngine {
    command_tx: Sender<PlaybackCommand>,
    is_playing: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
    pub track_id: usize,
}

impl PlaybackEngine {
    /// Create a new playback engine that receives ticks from the master clock
    pub fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        tick_rx: CrossbeamReceiver<ClockTick>,
        bpm: Arc<AtomicU64>,
        track_id: usize,
    ) -> Self {
        Self::new_with_midi(audio_handle, tick_rx, bpm, track_id, None)
    }

    /// Create a new playback engine with optional MIDI output
    pub fn new_with_midi(
        audio_handle: Arc<AudioPlayerHandle>,
        tick_rx: CrossbeamReceiver<ClockTick>,
        bpm: Arc<AtomicU64>,
        track_id: usize,
        midi_handle: Option<Arc<MidiOutputHandle>>,
    ) -> Self {
        let (tx, rx) = channel();
        let is_playing = Arc::new(AtomicBool::new(false));
        let is_playing_clone = is_playing.clone();

        let thread = thread::spawn(move || {
            PlaybackLoop::new(
                audio_handle,
                midi_handle,
                tick_rx,
                bpm,
                rx,
                is_playing_clone,
                track_id,
            )
            .run();
        });

        PlaybackEngine {
            command_tx: tx,
            is_playing,
            _thread: thread,
            track_id,
        }
    }

    /// Play a progression immediately (interrupts any current playback)
    pub fn play_progression(&self, config: ProgressionConfig) -> Result<()> {
        self.command_tx
            .send(PlaybackCommand::PlayProgression(config))
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }

    /// Queue a progression to start at the next beat boundary (seamless transition)
    pub fn queue_progression(&self, config: ProgressionConfig) -> Result<()> {
        self.queue_progression_with_mode(config, QueueMode::Beat)
    }

    /// Queue a progression to start at the specified sync point
    pub fn queue_progression_with_mode(
        &self,
        config: ProgressionConfig,
        mode: QueueMode,
    ) -> Result<()> {
        self.command_tx
            .send(PlaybackCommand::QueueProgression(config, mode))
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }

    /// Stop any currently playing progression
    pub fn stop(&self) -> Result<()> {
        self.command_tx
            .send(PlaybackCommand::Stop)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }

    /// Set volume for this track
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.command_tx
            .send(PlaybackCommand::SetVolume(volume))
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))
    }

    /// Check if a progression is currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        let _ = self.command_tx.send(PlaybackCommand::Shutdown);
    }
}

/// Cursor for tracking position within a pattern using beat-based timing
/// This enables phase-preserving variable updates (like the browser editor)
#[derive(Clone, Debug)]
struct PlaybackCursor {
    /// When this pattern started (global clock beat)
    pattern_start_beat: f64,
    /// Total duration of the pattern in beats (cached from evaluation)
    pattern_duration_beats: f32,
    /// Whether this pattern loops
    looping: bool,
}

impl PlaybackCursor {
    fn new(start_beat: f64, duration_beats: f32, looping: bool) -> Self {
        Self {
            pattern_start_beat: start_beat,
            pattern_duration_beats: duration_beats,
            looping,
        }
    }

    /// Calculate the effective position within the pattern at the given beat
    /// Returns (local_beat, wrapped_for_loop)
    fn get_position(&self, current_beat: f64) -> (f32, bool) {
        let local_beat = (current_beat - self.pattern_start_beat) as f32;

        if self.looping && self.pattern_duration_beats > 0.0 {
            let wrapped = local_beat % self.pattern_duration_beats;
            let did_wrap = local_beat >= self.pattern_duration_beats;
            (wrapped, did_wrap)
        } else {
            (local_beat, false)
        }
    }

    /// Check if the pattern has completed (for non-looping patterns)
    fn is_complete(&self, current_beat: f64) -> bool {
        if self.looping {
            false
        } else {
            let local_beat = (current_beat - self.pattern_start_beat) as f32;
            local_beat >= self.pattern_duration_beats
        }
    }
}

/// Internal playback loop that runs in a dedicated thread
///
/// Uses crossbeam select! to wait on both clock ticks and commands simultaneously.
struct PlaybackLoop {
    audio_handle: Arc<AudioPlayerHandle>,
    midi_handle: Option<Arc<MidiOutputHandle>>,
    tick_rx: CrossbeamReceiver<ClockTick>,
    bpm: Arc<AtomicU64>,
    command_rx: Receiver<PlaybackCommand>,
    is_playing: Arc<AtomicBool>,

    // Playback state
    current_progression: Option<ProgressionConfig>,
    /// Queue of progressions waiting to play (FIFO) with their sync modes
    pending_queue: VecDeque<(ProgressionConfig, QueueMode)>,
    chord_index: usize,
    iteration: usize,
    audio_started: bool,
    track_id: usize,

    // Timing state for sub-beat notes
    /// Next beat boundary to trigger chord change
    next_chord_beat: f64,
    /// Whether we're in a gap (silence between notes)
    in_gap: bool,
    gap_end_beat: f64,

    // MIDI state
    /// Currently active MIDI notes for this track (for proper Note Off)
    active_midi_notes: Vec<u8>,

    /// Last error message (for deduplication - only show unique errors)
    last_error: Option<String>,

    // Step-sequencer state (new)
    /// Cursor for tracking pattern position (enables phase preservation)
    cursor: Option<PlaybackCursor>,
    /// Cached evaluated events for current cycle (reduces lock contention)
    cached_events: Vec<(Vec<f32>, f32)>,
    /// Cycle number when cache was populated
    cache_cycle: usize,
}

impl PlaybackLoop {
    fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        midi_handle: Option<Arc<MidiOutputHandle>>,
        tick_rx: CrossbeamReceiver<ClockTick>,
        bpm: Arc<AtomicU64>,
        command_rx: Receiver<PlaybackCommand>,
        is_playing: Arc<AtomicBool>,
        track_id: usize,
    ) -> Self {
        Self {
            audio_handle,
            midi_handle,
            tick_rx,
            bpm,
            command_rx,
            is_playing,
            current_progression: None,
            pending_queue: VecDeque::new(),
            chord_index: 0,
            iteration: 0,
            audio_started: false,
            track_id,
            next_chord_beat: 0.0,
            in_gap: false,
            gap_end_beat: 0.0,
            active_midi_notes: Vec::new(),
            last_error: None,
            cursor: None,
            cached_events: Vec::new(),
            cache_cycle: 0,
        }
    }

    fn get_bpm(&self) -> f32 {
        f32::from_bits(self.bpm.load(Ordering::Relaxed) as u32)
    }

    fn run(&mut self) {
        // Convert mpsc Receiver to crossbeam for use in select!
        let (cmd_bridge_tx, cmd_bridge_rx): (
            CrossbeamSender<PlaybackCommand>,
            CrossbeamReceiver<PlaybackCommand>,
        ) = crossbeam_channel::unbounded();

        // Spawn a bridge thread to forward mpsc commands to crossbeam
        let command_rx = std::mem::replace(&mut self.command_rx, channel().1);
        let bridge_tx = cmd_bridge_tx.clone();
        let _bridge_thread = thread::spawn(move || {
            while let Ok(cmd) = command_rx.recv() {
                if bridge_tx.send(cmd).is_err() {
                    break;
                }
            }
        });

        loop {
            select! {
                recv(self.tick_rx) -> tick_result => {
                    match tick_result {
                        Ok(tick) => {
                            self.handle_tick(tick);
                        }
                        Err(_) => {
                            // Clock channel closed, shutdown
                            break;
                        }
                    }
                }
                recv(cmd_bridge_rx) -> cmd_result => {
                    match cmd_result {
                        Ok(cmd) => {
                            if self.handle_command(cmd) {
                                break;
                            }
                        }
                        Err(_) => {
                            // Command channel closed, shutdown
                            break;
                        }
                    }
                }
            }
        }

        // Clean up - mute track
        let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
        self.is_playing.store(false, Ordering::Relaxed);
    }

    fn handle_tick(&mut self, tick: ClockTick) {
        // Process ALL ticks for sub-beat accuracy (24 PPQN gives us triplet precision)
        // This is critical for patterns like fast("C E", 2) which need 8th note timing
        let current_beat = tick.beat;

        // Handle gap ending
        if self.in_gap && current_beat >= self.gap_end_beat - TICK_EPSILON {
            self.in_gap = false;
        }

        // Check if we should advance to next chord (using epsilon for float comparison)
        if self.current_progression.is_some()
            && current_beat >= self.next_chord_beat - TICK_EPSILON
            && !self.in_gap
        {
            self.play_next_beat(current_beat);
        } else if self.current_progression.is_none() && !self.pending_queue.is_empty() {
            // No current progression but items in queue - check sync mode
            if let Some((_, mode)) = self.pending_queue.front() {
                let should_start = match mode {
                    QueueMode::Beat => tick.is_beat_boundary(),
                    QueueMode::Bar => tick.is_bar_boundary(),
                    QueueMode::Cycle => tick.is_beat_boundary(), // No cycle to wait for, use beat
                };
                if should_start {
                    if let Some((next_config, _)) = self.pending_queue.pop_front() {
                        self.current_progression = Some(next_config);
                        self.chord_index = 0;
                        self.iteration = 0;
                        self.next_chord_beat = current_beat;
                        self.is_playing.store(true, Ordering::Relaxed);
                        let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
                        self.play_next_beat(current_beat);
                    }
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: PlaybackCommand) -> bool {
        match cmd {
            PlaybackCommand::PlayProgression(config) => {
                // Immediate switch - reset position and start new progression
                // Initialize cursor and cache for step-sequencer behavior
                let looping = config.loop_count.is_none();

                // Pre-evaluate to get duration (cache it for timing consistency)
                if let Ok(events) = config.source.evaluate() {
                    let duration: f32 = events.iter().map(|(_, d)| d).sum();
                    self.cached_events = events;
                    self.cache_cycle = 0;
                    self.cursor = Some(PlaybackCursor::new(0.0, duration, looping));
                } else {
                    self.cached_events.clear();
                    self.cursor = None;
                }

                self.current_progression = Some(config);
                self.pending_queue.clear();
                self.chord_index = 0;
                self.iteration = 0;
                self.next_chord_beat = 0.0; // Start immediately on next beat
                self.is_playing.store(true, Ordering::Relaxed);
                self.audio_started = false;
                let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
            }
            PlaybackCommand::QueueProgression(config, mode) => {
                self.pending_queue.push_back((config, mode));
                self.is_playing.store(true, Ordering::Relaxed);
            }
            PlaybackCommand::Stop => {
                self.current_progression = None;
                self.pending_queue.clear();
                self.is_playing.store(false, Ordering::Relaxed);
                // Set notes to empty to trigger ADSR release - don't mute volume immediately
                let _ = self.audio_handle.set_track_notes(self.track_id, vec![]);
                self.audio_started = false;
                // Clear cursor and cache
                self.cursor = None;
                self.cached_events.clear();
            }
            PlaybackCommand::SetVolume(vol) => {
                let _ = self.audio_handle.set_track_volume(self.track_id, vol);
            }
            PlaybackCommand::Shutdown => {
                return true;
            }
        }
        false
    }

    fn duration_to_beats(&self, duration: &Duration) -> f32 {
        match duration {
            Duration::Beats(b) => *b,
            Duration::Seconds(s) => {
                let bpm = self.get_bpm();
                s * bpm / 60.0
            }
            Duration::Bars(bars) => bars * 4.0,
        }
    }

    /// Try to start the next queued progression
    fn try_start_next_queued(&mut self) -> bool {
        if let Some((next_config, _mode)) = self.pending_queue.pop_front() {
            self.current_progression = Some(next_config);
            self.chord_index = 0;
            self.iteration = 0;
            true
        } else {
            false
        }
    }

    /// Stop playback
    fn stop_playback(&mut self) {
        self.current_progression = None;
        self.is_playing.store(false, Ordering::Relaxed);
        // Set notes to empty - this triggers ADSR release phase for graceful fade-out
        // Don't set volume to 0 here; let the envelope handle the fade
        let _ = self.audio_handle.set_track_notes(self.track_id, vec![]);
        self.audio_started = false;

        // Send MIDI Note Off for any active notes
        self.send_midi_notes_off();
    }

    /// Send MIDI Note Off for all currently active notes on this track
    fn send_midi_notes_off(&mut self) {
        if let Some(ref midi_handle) = self.midi_handle {
            if let Err(e) = midi_handle.notes_off(self.track_id, &self.active_midi_notes) {
                eprintln!("Failed to send MIDI note off: {}", e);
            }
        }
        self.active_midi_notes.clear();
    }

    /// Send MIDI notes for the given frequencies (converts to MIDI note numbers)
    fn send_midi_notes(&mut self, frequencies: &[f32]) {
        if let Some(ref midi_handle) = self.midi_handle {
            // First, turn off any currently active notes
            if !self.active_midi_notes.is_empty() {
                if let Err(e) = midi_handle.notes_off(self.track_id, &self.active_midi_notes) {
                    eprintln!("Failed to send MIDI note off: {}", e);
                }
                self.active_midi_notes.clear();
            }

            // Convert frequencies to MIDI notes and send Note On
            if !frequencies.is_empty() {
                let midi_notes: Vec<u8> =
                    frequencies.iter().map(|&f| frequency_to_midi(f)).collect();

                // Send Note On with velocity 100 (solid feel)
                if let Err(e) = midi_handle.notes_on(self.track_id, &midi_notes, 100) {
                    eprintln!("Failed to send MIDI note on: {}", e);
                }

                // Track active notes for later Note Off
                self.active_midi_notes = midi_notes;
            }
        }
    }

    fn play_next_beat(&mut self, current_beat: f64) {
        // Quantized Interrupt Logic:
        // If current progression is an infinite loop, check if we should switch to queued item
        // based on the queued item's sync mode
        let is_infinite = self
            .current_progression
            .as_ref()
            .map_or(false, |p| p.loop_count.is_none());

        if is_infinite && !self.pending_queue.is_empty() {
            // Check the sync mode of the next queued item
            if let Some((_, mode)) = self.pending_queue.front() {
                let should_switch = match mode {
                    // Beat and Bar modes: switch immediately (already at a beat/bar from handle_tick)
                    QueueMode::Beat | QueueMode::Bar => true,
                    // Cycle mode: only switch at end of pattern cycle (chord_index about to wrap)
                    QueueMode::Cycle => false, // Will be handled in evaluate_with_cycle_check
                };
                if should_switch {
                    self.try_start_next_queued();
                }
            }
        }

        let config = match &self.current_progression {
            Some(c) => c.clone(),
            None => return,
        };

        // Check if we've completed all iterations
        if let Some(max_loops) = config.loop_count {
            if self.iteration >= max_loops {
                if !self.try_start_next_queued() {
                    self.stop_playback();
                }
                return;
            }
        }

        // CRITICAL FIX: Check if we need to advance cycle BEFORE evaluating
        // This ensures every() and other cycle-dependent operators see the correct cycle
        let frequencies = match self.evaluate_with_cycle_check(&config) {
            Ok(f) => f,
            Err(e) => {
                // Filter out control-flow signals (not real errors)
                let msg = e.to_string();
                if !msg.contains("Switched to queued") && !msg.contains("Progression complete") {
                    // Only show error if different from last error (deduplication)
                    if self.last_error.as_ref() != Some(&msg) {
                        eprintln!("Failed to evaluate playback source: {}", e);
                        self.last_error = Some(msg);
                    }
                }
                return;
            }
        };

        if frequencies.is_empty() {
            return;
        }

        let (chord_frequencies, event_duration) = &frequencies[self.chord_index];

        // Set the envelope for this track (from pattern if available)
        if let Ok(envelope) = config.source.get_envelope() {
            if let Err(e) = self
                .audio_handle
                .set_track_envelope(self.track_id, envelope)
            {
                eprintln!("Failed to set envelope: {}", e);
            }
        }

        // Set the waveform for this track (from pattern if available)
        if let Ok(Some(waveform)) = config.source.get_waveform() {
            if let Err(e) = self
                .audio_handle
                .set_track_waveform(self.track_id, waveform)
            {
                eprintln!("Failed to set waveform: {}", e);
            }
        }

        // Check output mode to determine what to send
        let audio_enabled = self
            .midi_handle
            .as_ref()
            .map_or(true, |h| h.audio_enabled());
        let midi_enabled = self
            .midi_handle
            .as_ref()
            .map_or(false, |h| h.midi_enabled() && h.is_connected());

        // Set the notes for this track (audio) - only if audio is enabled
        if audio_enabled {
            if let Err(e) = self
                .audio_handle
                .set_track_notes(self.track_id, chord_frequencies.clone())
            {
                eprintln!("Failed to set notes: {}", e);
            }
        } else {
            // Clear audio notes if audio is disabled (MIDI-only mode)
            if let Err(e) = self.audio_handle.set_track_notes(self.track_id, vec![]) {
                eprintln!("Failed to clear notes: {}", e);
            }
        }

        // Send MIDI notes - only if MIDI is enabled and connected
        if midi_enabled {
            self.send_midi_notes(chord_frequencies);
        }

        // Start audio if not already playing (even in MIDI-only mode, we need the audio thread running)
        if !self.audio_started {
            if let Err(e) = self.audio_handle.play() {
                eprintln!("Failed to start audio: {}", e);
            }
            self.audio_started = true;
        }

        // Advance chord index
        self.chord_index += 1;

        // Calculate next chord beat using per-event duration
        // Pattern events have their own duration from to_events()
        // Non-pattern events use the config's note_duration as fallback
        let base_duration = self.duration_to_beats(&config.note_duration);
        let duration_beats = if *event_duration > 0.0 {
            // Use the event's own duration, scaled by the base duration
            // Pattern events are relative (e.g., 0.5 = half of a step)
            // We multiply by base_duration to get actual beat timing
            event_duration * base_duration
        } else {
            base_duration
        };
        let gap_beats = self.duration_to_beats(&config.gap_duration);

        if gap_beats > 0.0 {
            // Schedule gap, then chord
            self.in_gap = true;
            self.gap_end_beat = current_beat + duration_beats as f64;
            self.next_chord_beat = self.gap_end_beat + gap_beats as f64;

            // Mute for gap will happen when gap starts (next beat check)
        } else {
            self.next_chord_beat = current_beat + duration_beats as f64;
        }
    }

    /// Evaluate the current pattern, handling cycle advancement properly.
    /// Uses cached events to reduce lock contention - only re-evaluates on cycle change.
    fn evaluate_with_cycle_check(
        &mut self,
        config: &ProgressionConfig,
    ) -> Result<Vec<(Vec<f32>, f32)>> {
        // Check if we've reached end of pattern and need to wrap
        if self.chord_index >= self.cached_events.len() {
            self.chord_index = 0;
            self.iteration += 1;

            // Check for QueueMode::Cycle - end of cycle is our switch point
            if config.loop_count.is_none() && !self.pending_queue.is_empty() {
                if let Some((_, QueueMode::Cycle)) = self.pending_queue.front() {
                    self.try_start_next_queued();
                    // Return error to signal we've switched progressions
                    return Err(anyhow::anyhow!("Switched to queued progression"));
                }
            }

            // Check loop count after increment
            if let Some(max_loops) = config.loop_count {
                if self.iteration >= max_loops {
                    if !self.try_start_next_queued() {
                        self.stop_playback();
                    }
                    return Err(anyhow::anyhow!("Progression complete"));
                }
            }
        }

        // Only re-evaluate if cycle changed (reduces RwLock contention significantly)
        if self.iteration != self.cache_cycle {
            // Update _cycle in environment BEFORE evaluation
            config
                .source
                .update_environment("_cycle", Value::Number(self.iteration as i32))?;

            // Re-evaluate and cache
            self.cached_events = config.source.evaluate()?;
            self.cache_cycle = self.iteration;

            // Update cursor duration if pattern length changed (for phase preservation)
            if let Some(ref mut cursor) = self.cursor {
                let new_duration: f32 = self.cached_events.iter().map(|(_, d)| d).sum();
                cursor.pattern_duration_beats = new_duration;
            }
        }

        Ok(self.cached_events.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::audio::AudioPlayerHandle;
    use crate::audio::clock::MasterClock;

    #[test]
    fn test_progression_config_builder() {
        let progression = vec![
            vec![261.63, 329.63, 392.00], // C major
            vec![349.23, 440.00, 523.25], // F major
        ];

        let config = ProgressionConfig::new(progression.clone())
            .with_duration(Duration::Beats(2.0))
            .with_gap(Duration::Beats(0.25))
            .with_looping();

        assert_eq!(config.source.evaluate().unwrap().len(), 2);
        assert!(config.loop_count.is_none());
    }

    #[test]
    fn test_playback_engine_creation() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                let clock = MasterClock::new(120.0);
                let tick_rx = clock.subscribe();
                let bpm = Arc::new(AtomicU64::new(120.0_f32.to_bits() as u64));
                let engine = PlaybackEngine::new(Arc::new(handle), tick_rx, bpm, 1);
                assert!(!engine.is_playing());
            }
            Err(_) => {
                println!("Skipping playback engine test - no audio device");
            }
        }
    }

    #[test]
    fn test_playback_engine_commands() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                let clock = MasterClock::new(120.0);
                let tick_rx = clock.subscribe();
                let bpm = Arc::new(AtomicU64::new(120.0_f32.to_bits() as u64));
                let engine = PlaybackEngine::new(Arc::new(handle), tick_rx, bpm, 1);

                let config = ProgressionConfig::new(vec![vec![440.0]]);

                // Test that commands don't panic
                assert!(engine.play_progression(config.clone()).is_ok());
                assert!(engine.queue_progression(config.clone()).is_ok());
                assert!(engine.stop().is_ok());
            }
            Err(_) => {
                println!("Skipping command test - no audio device");
            }
        }
    }

    #[test]
    fn test_queue_fifo_order() {
        let mut queue: VecDeque<ProgressionConfig> = VecDeque::new();

        let config1 = ProgressionConfig::new(vec![vec![440.0]]);
        let config2 = ProgressionConfig::new(vec![vec![880.0]]);
        let config3 = ProgressionConfig::new(vec![vec![220.0]]);

        queue.push_back(config1.clone());
        queue.push_back(config2.clone());
        queue.push_back(config3.clone());

        assert_eq!(queue.len(), 3);

        let first = queue.pop_front().unwrap();
        assert_eq!(first.source.evaluate().unwrap()[0].0[0], 440.0);

        let second = queue.pop_front().unwrap();
        assert_eq!(second.source.evaluate().unwrap()[0].0[0], 880.0);

        let third = queue.pop_front().unwrap();
        assert_eq!(third.source.evaluate().unwrap()[0].0[0], 220.0);

        assert!(queue.is_empty());
    }

    #[test]
    fn test_progression_config_loop_settings() {
        let progression = vec![vec![440.0]];

        let config = ProgressionConfig::new(progression.clone());
        assert_eq!(config.loop_count, Some(1));

        let config_infinite = ProgressionConfig::new(progression.clone()).with_looping();
        assert_eq!(config_infinite.loop_count, None);

        let config_count = ProgressionConfig::new(progression.clone()).with_loop_count(5);
        assert_eq!(config_count.loop_count, Some(5));
    }
}
