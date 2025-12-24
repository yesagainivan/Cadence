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
use crate::parser::{Evaluator, Expression, SharedEnvironment, Value};
use anyhow::Result;
use crossbeam_channel::{Receiver as CrossbeamReceiver, Sender as CrossbeamSender, select};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};

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
            Value::String(_) => Err(anyhow::anyhow!("Cannot play a string string")),
            Value::Chord(chord) => Ok(vec![(chord.notes().map(|n| n.frequency()).collect(), 1.0)]),
            Value::Progression(prog) => Ok(prog
                .chords()
                .map(|c| (c.notes().map(|n| n.frequency()).collect(), 1.0))
                .collect()),
            Value::Boolean(_) => Err(anyhow::anyhow!("Cannot play a boolean value")),
            Value::Number(_) => Err(anyhow::anyhow!("Cannot play a raw number")),
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
            let _ = env
                .write()
                .map_err(|e| anyhow::anyhow!("Environment lock poisoned: {}", e))?
                .set(name, value);
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

/// Commands that can be sent to the playback engine
#[derive(Debug)]
pub enum PlaybackCommand {
    /// Start playing a progression immediately (interrupts current)
    PlayProgression(ProgressionConfig),
    /// Queue a progression to start at the next beat boundary
    QueueProgression(ProgressionConfig),
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
        let (tx, rx) = channel();
        let is_playing = Arc::new(AtomicBool::new(false));
        let is_playing_clone = is_playing.clone();

        let thread = thread::spawn(move || {
            PlaybackLoop::new(audio_handle, tick_rx, bpm, rx, is_playing_clone, track_id).run();
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
        self.command_tx
            .send(PlaybackCommand::QueueProgression(config))
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

/// Internal playback loop that runs in a dedicated thread
///
/// Uses crossbeam select! to wait on both clock ticks and commands simultaneously.
struct PlaybackLoop {
    audio_handle: Arc<AudioPlayerHandle>,
    tick_rx: CrossbeamReceiver<ClockTick>,
    bpm: Arc<AtomicU64>,
    command_rx: Receiver<PlaybackCommand>,
    is_playing: Arc<AtomicBool>,

    // Playback state
    current_progression: Option<ProgressionConfig>,
    /// Queue of progressions waiting to play (FIFO)
    pending_queue: VecDeque<ProgressionConfig>,
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
}

impl PlaybackLoop {
    fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        tick_rx: CrossbeamReceiver<ClockTick>,
        bpm: Arc<AtomicU64>,
        command_rx: Receiver<PlaybackCommand>,
        is_playing: Arc<AtomicBool>,
        track_id: usize,
    ) -> Self {
        Self {
            audio_handle,
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
        // Only process on beat boundaries for now (can be more granular later)
        if !tick.is_beat_boundary() {
            return;
        }

        // Handle gap ending
        if self.in_gap && tick.beat >= self.gap_end_beat {
            self.in_gap = false;
        }

        // Check if we should advance to next chord
        if self.current_progression.is_some() && tick.beat >= self.next_chord_beat && !self.in_gap {
            self.play_next_beat(tick.beat);
        } else if self.current_progression.is_none() && !self.pending_queue.is_empty() {
            // No current progression but items in queue - start on this beat
            if let Some(next) = self.pending_queue.pop_front() {
                self.current_progression = Some(next);
                self.chord_index = 0;
                self.iteration = 0;
                self.next_chord_beat = tick.beat;
                self.is_playing.store(true, Ordering::Relaxed);
                let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
                self.play_next_beat(tick.beat);
            }
        }
    }

    fn handle_command(&mut self, cmd: PlaybackCommand) -> bool {
        match cmd {
            PlaybackCommand::PlayProgression(config) => {
                // Immediate switch - reset position and start new progression
                self.current_progression = Some(config);
                self.pending_queue.clear();
                self.chord_index = 0;
                self.iteration = 0;
                self.next_chord_beat = 0.0; // Start immediately on next beat
                self.is_playing.store(true, Ordering::Relaxed);
                self.audio_started = false;
                let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
            }
            PlaybackCommand::QueueProgression(config) => {
                self.pending_queue.push_back(config);
                self.is_playing.store(true, Ordering::Relaxed);
            }
            PlaybackCommand::Stop => {
                self.current_progression = None;
                self.pending_queue.clear();
                self.is_playing.store(false, Ordering::Relaxed);
                let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
                self.audio_started = false;
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
        if let Some(next) = self.pending_queue.pop_front() {
            self.current_progression = Some(next);
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
        let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
        let _ = self.audio_handle.set_track_notes(self.track_id, vec![]);
        self.audio_started = false;
    }

    fn play_next_beat(&mut self, current_beat: f64) {
        // Quantized Interrupt Logic:
        // If current progression is an infinite loop, try to switch to queued item
        let is_infinite = self
            .current_progression
            .as_ref()
            .map_or(false, |p| p.loop_count.is_none());

        if is_infinite && !self.pending_queue.is_empty() {
            self.try_start_next_queued();
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

        // Update environment with current cycle index (for `every` operator)
        // We use self.iteration as the cycle count
        if let Err(e) = config
            .source
            .update_environment("_cycle", Value::Number(self.iteration as i32))
        {
            eprintln!("Failed to update environment: {}", e);
        }

        // Evaluate the source to get current frequencies
        let frequencies = match config.source.evaluate() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to evaluate playback source: {}", e);
                return;
            }
        };

        // Check if we've reached end of progression
        if self.chord_index >= frequencies.len() {
            self.chord_index = 0;
            self.iteration += 1;

            // Re-check loop count
            if let Some(max_loops) = config.loop_count {
                if self.iteration >= max_loops {
                    if !self.try_start_next_queued() {
                        self.stop_playback();
                    }
                    return;
                }
            }
        }

        if frequencies.is_empty() {
            return;
        }

        let (chord_frequencies, event_duration) = &frequencies[self.chord_index];

        // Set the notes for this track
        if let Err(e) = self
            .audio_handle
            .set_track_notes(self.track_id, chord_frequencies.clone())
        {
            eprintln!("Failed to set notes: {}", e);
        }

        // Start audio if not already playing
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
