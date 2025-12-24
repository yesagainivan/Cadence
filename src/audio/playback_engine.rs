//! Beat-quantized progression playback engine
//!
//! This module provides a production-ready audio progression system that enables
//! seamless, beat-synchronized switching between progressions—inspired by live
//! coding environments like Sonic Pi and TidalCycles.

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::scheduler::{Duration, Scheduler};
use crate::parser::{Evaluator, Expression, SharedEnvironment, Value};
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::{self, JoinHandle};
use std::time::Instant;

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
    /// Get the current frequencies by evaluating the source
    /// For Static sources, returns the stored frequencies
    /// For Reactive sources, re-evaluates the expression against the current environment
    pub fn evaluate(&self) -> Result<Vec<Vec<f32>>> {
        match self {
            PlaybackSource::Static(freqs) => Ok(freqs.clone()),
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

    /// Convert a Value to a vector of frequency vectors
    fn value_to_frequencies(value: &Value) -> Result<Vec<Vec<f32>>> {
        match value {
            Value::Note(note) => Ok(vec![vec![note.frequency()]]),
            Value::Chord(chord) => Ok(vec![chord.notes().map(|n| n.frequency()).collect()]),
            Value::Progression(prog) => Ok(prog
                .chords()
                .map(|c| c.notes().map(|n| n.frequency()).collect())
                .collect()),
            Value::Boolean(_) => Err(anyhow::anyhow!("Cannot play a boolean value")),
            Value::Pattern(pattern) => {
                // Convert pattern to frequencies, preserving rests as empty vectors
                // Each event becomes one "chord" in the playback
                Ok(pattern
                    .to_events()
                    .into_iter()
                    .map(|(freqs, _, is_rest)| {
                        if is_rest {
                            vec![] // Empty = silence for this step
                        } else {
                            freqs
                        }
                    })
                    .collect())
            }
        }
    }

    /// Get the number of chords in this source (evaluates if reactive)
    pub fn len(&self) -> Result<usize> {
        Ok(self.evaluate()?.len())
    }

    /// Check if the source is empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.evaluate()?.is_empty())
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
pub struct PlaybackEngine {
    command_tx: Sender<PlaybackCommand>,
    is_playing: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
    pub track_id: usize,
}

impl PlaybackEngine {
    /// Create a new playback engine with a persistent playback thread
    pub fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        scheduler: Arc<Scheduler>,
        track_id: usize,
    ) -> Self {
        let (tx, rx) = channel();
        let is_playing = Arc::new(AtomicBool::new(false));
        let is_playing_clone = is_playing.clone();

        let thread = thread::spawn(move || {
            PlaybackLoop::new(audio_handle, scheduler, rx, is_playing_clone, track_id).run();
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
struct PlaybackLoop {
    audio_handle: Arc<AudioPlayerHandle>,
    scheduler: Arc<Scheduler>,
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
}

impl PlaybackLoop {
    fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        scheduler: Arc<Scheduler>,
        command_rx: Receiver<PlaybackCommand>,
        is_playing: Arc<AtomicBool>,
        track_id: usize,
    ) -> Self {
        Self {
            audio_handle,
            scheduler,
            command_rx,
            is_playing,
            current_progression: None,
            pending_queue: VecDeque::new(),
            chord_index: 0,
            iteration: 0,
            audio_started: false,
            track_id,
        }
    }

    fn run(&mut self) {
        loop {
            // Process all pending commands
            match self.process_commands() {
                LoopAction::Continue => {}
                LoopAction::Shutdown => break,
            }

            // If we have a progression to play, advance it
            if self.current_progression.is_some() {
                self.play_next_beat();
            } else if !self.pending_queue.is_empty() {
                // Queued content waiting to start at next beat
                self.is_playing.store(true, Ordering::Relaxed);

                // Ensure scheduler is running
                self.scheduler.start();

                // Wait for next beat boundary
                let beat_time = self.scheduler.next_beat_time();
                self.wait_until_with_command_check(beat_time);

                // Check if we were stopped or interrupted during wait
                if !self.is_playing.load(Ordering::Relaxed) {
                    continue;
                }

                // Now start the queued item if still pending
                // Note: pending_queue might have been cleared if Stop was received,
                // or modified if new Queue commands arrived.
                if let Some(next) = self.pending_queue.pop_front() {
                    self.current_progression = Some(next);
                    self.chord_index = 0;
                    self.iteration = 0;
                    self.audio_started = false;
                    let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
                }
            } else {
                // No progression - wait for commands
                match self.command_rx.recv() {
                    Ok(cmd) => {
                        if let LoopAction::Shutdown = self.handle_command(cmd) {
                            break;
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }
        }

        // Clean up - pause specific track instead of master
        let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
        self.is_playing.store(false, Ordering::Relaxed);
    }

    fn process_commands(&mut self) -> LoopAction {
        loop {
            match self.command_rx.try_recv() {
                Ok(cmd) => {
                    if let LoopAction::Shutdown = self.handle_command(cmd) {
                        return LoopAction::Shutdown;
                    }
                }
                Err(TryRecvError::Empty) => return LoopAction::Continue,
                Err(TryRecvError::Disconnected) => return LoopAction::Shutdown,
            }
        }
    }

    fn handle_command(&mut self, cmd: PlaybackCommand) -> LoopAction {
        match cmd {
            PlaybackCommand::PlayProgression(config) => {
                // Immediate switch - reset position and start new progression
                self.current_progression = Some(config);
                self.pending_queue.clear(); // Clear queue on immediate play
                self.chord_index = 0;
                self.iteration = 0;
                self.is_playing.store(true, Ordering::Relaxed);

                // Ensure scheduler is tracking time, but don't reset if already running
                // to maintain sync with other tracks
                self.scheduler.start();

                // Don't start audio yet - it will start on first beat
                self.audio_started = false;
                // Ensure track volume is up (in case it was stopped)
                let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
            }
            PlaybackCommand::QueueProgression(config) => {
                // Queue for next beat boundary - ALWAYS add to FIFO queue
                self.pending_queue.push_back(config);

                // Ensure we are marked as playing so the loop picks it up
                self.is_playing.store(true, Ordering::Relaxed);

                // Ensure scheduler is running
                self.scheduler.start();
            }
            PlaybackCommand::Stop => {
                self.current_progression = None;
                self.pending_queue.clear();
                self.is_playing.store(false, Ordering::Relaxed);
                // Mute track instead of pausing master
                let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
                self.audio_started = false;
            }
            PlaybackCommand::SetVolume(vol) => {
                let _ = self.audio_handle.set_track_volume(self.track_id, vol);
            }
            PlaybackCommand::Shutdown => {
                return LoopAction::Shutdown;
            }
        }
        LoopAction::Continue
    }

    /// Helper: Try to start the next queued progression.
    /// Returns true if a new progression was started, false if queue was empty.
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

    /// Helper: Stop playback and fade out.
    fn stop_playback(&mut self) {
        self.current_progression = None;
        self.is_playing.store(false, Ordering::Relaxed);
        let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
        self.audio_started = false;
    }

    fn play_next_beat(&mut self) {
        // Quantized Interrupt Logic:
        // If current progression is an infinite loop OR we have no current progression,
        // try to switch to a queued item at the beat boundary.
        let is_infinite = self
            .current_progression
            .as_ref()
            .map_or(false, |p| p.loop_count.is_none());

        if (is_infinite || self.current_progression.is_none()) && !self.pending_queue.is_empty() {
            self.try_start_next_queued();
        }

        let config = match &self.current_progression {
            Some(c) => c.clone(),
            None => return,
        };

        // Check if we've completed all iterations
        if let Some(max_loops) = config.loop_count {
            if self.iteration >= max_loops {
                // Current progression done - try to get next from queue
                if !self.try_start_next_queued() {
                    self.stop_playback();
                }
                return;
            }
        }

        // Evaluate the source to get current frequencies
        // For reactive sources, this re-evaluates the expression each beat
        let frequencies = match config.source.evaluate() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to evaluate playback source: {}", e);
                return;
            }
        };

        // Get current chord - check bounds after evaluation (reactive sources may change length)
        if self.chord_index >= frequencies.len() {
            // Move to next iteration
            self.chord_index = 0;
            self.iteration += 1;

            // Re-check loop count
            if let Some(max_loops) = config.loop_count {
                if self.iteration >= max_loops {
                    // Current progression done - try to get next from queue
                    if !self.try_start_next_queued() {
                        self.stop_playback();
                    }
                    return;
                }
            }
        }

        // Handle case where frequencies are empty after re-evaluation
        if frequencies.is_empty() {
            return;
        }

        let chord_frequencies = &frequencies[self.chord_index];

        // Set the notes for this track (seamless - no pause needed)
        if let Err(e) = self
            .audio_handle
            .set_track_notes(self.track_id, chord_frequencies.clone())
        {
            eprintln!("Failed to set notes: {}", e);
        }

        // Start audio if not already playing (via master play)
        if !self.audio_started {
            if let Err(e) = self.audio_handle.play() {
                eprintln!("Failed to start audio: {}", e);
            }
            self.audio_started = true;
        }

        // Advance chord index
        self.chord_index += 1;

        // Wait for beat duration, checking for commands periodically
        let beat_end = self.scheduler.time_from_now(config.note_duration);
        self.wait_until_with_command_check(beat_end);

        // Handle gap if specified
        let gap_ms = config.gap_duration.to_millis(self.scheduler.get_bpm());
        if gap_ms > 0 {
            // Mute track for gap
            let _ = self.audio_handle.set_track_notes(self.track_id, vec![]);
            self.scheduler.sleep(config.gap_duration);
            // Notes will be set again at start of next beat
        }
    }

    /// Wait until a specific time, but check for commands periodically
    fn wait_until_with_command_check(&mut self, target: Instant) {
        while Instant::now() < target {
            // Check for high-priority commands (Stop/Shutdown) more frequently
            match self.command_rx.try_recv() {
                Ok(PlaybackCommand::Stop) => {
                    self.current_progression = None;
                    self.pending_queue.clear();
                    self.is_playing.store(false, Ordering::Relaxed);
                    let _ = self.audio_handle.set_track_volume(self.track_id, 0.0);
                    self.audio_started = false;
                    return;
                }
                Ok(PlaybackCommand::SetVolume(vol)) => {
                    let _ = self.audio_handle.set_track_volume(self.track_id, vol);
                }
                Ok(PlaybackCommand::Shutdown) => {
                    return;
                }
                Ok(PlaybackCommand::QueueProgression(config)) => {
                    // Add to queue
                    self.pending_queue.push_back(config);
                }
                Ok(PlaybackCommand::PlayProgression(config)) => {
                    // Immediate switch even mid-beat
                    self.current_progression = Some(config);
                    self.pending_queue.clear();
                    self.chord_index = 0;
                    self.iteration = 0;
                    self.scheduler.start(); // Ensure started (was reset)
                    // Ensure volume is up
                    let _ = self.audio_handle.set_track_volume(self.track_id, 1.0);
                    return; // Exit early to start new progression
                }
                Err(_) => {}
            }

            // Small sleep for responsiveness
            thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

enum LoopAction {
    Continue,
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

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
                let scheduler = Scheduler::new(120.0);
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler), 1);
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
                let scheduler = Scheduler::new(120.0);
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler), 1);

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

    // ========== Queue Behavior Unit Tests ==========
    // These tests verify the queue logic without requiring audio hardware

    /// Test that try_start_next_queued returns false on empty queue
    #[test]
    fn test_try_start_next_queued_empty() {
        // We can't create a PlaybackLoop directly without audio, but we can test
        // the VecDeque behavior it depends on
        let mut queue: VecDeque<ProgressionConfig> = VecDeque::new();
        assert!(queue.pop_front().is_none());
    }

    /// Test that VecDeque maintains FIFO order (core queue guarantee)
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

        // FIFO: first in, first out
        let first = queue.pop_front().unwrap();
        assert_eq!(first.source.evaluate().unwrap()[0][0], 440.0);

        let second = queue.pop_front().unwrap();
        assert_eq!(second.source.evaluate().unwrap()[0][0], 880.0);

        let third = queue.pop_front().unwrap();
        assert_eq!(third.source.evaluate().unwrap()[0][0], 220.0);

        assert!(queue.is_empty());
    }

    /// Test that Stop command clears queue via engine
    #[test]
    fn test_stop_clears_queue() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                let scheduler = Scheduler::new(120.0);
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler), 1);

                let config = ProgressionConfig::new(vec![vec![440.0]]);

                // Queue multiple items
                assert!(engine.queue_progression(config.clone()).is_ok());
                assert!(engine.queue_progression(config.clone()).is_ok());
                assert!(engine.queue_progression(config.clone()).is_ok());

                // Stop should clear everything
                assert!(engine.stop().is_ok());

                // Give time for command to be processed
                std::thread::sleep(std::time::Duration::from_millis(50));

                // After stop, is_playing should be false
                assert!(!engine.is_playing());
            }
            Err(_) => {
                println!("Skipping test_stop_clears_queue - no audio device");
            }
        }
    }

    /// Test that PlayProgression clears pending queue
    #[test]
    fn test_play_clears_queue() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                let scheduler = Scheduler::new(120.0);
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler), 1);

                let config1 = ProgressionConfig::new(vec![vec![440.0]]);
                let config2 = ProgressionConfig::new(vec![vec![880.0]]);

                // Queue several items
                assert!(engine.queue_progression(config1.clone()).is_ok());
                assert!(engine.queue_progression(config1.clone()).is_ok());

                // Play immediately should clear queue and play config2
                assert!(engine.play_progression(config2).is_ok());

                // Give time for command to be processed
                std::thread::sleep(std::time::Duration::from_millis(50));

                // Engine should be playing
                assert!(engine.is_playing());
            }
            Err(_) => {
                println!("Skipping test_play_clears_queue - no audio device");
            }
        }
    }

    /// Test ProgressionConfig loop settings
    #[test]
    fn test_progression_config_loop_settings() {
        let progression = vec![vec![440.0]];

        // Default: 1 loop
        let config = ProgressionConfig::new(progression.clone());
        assert_eq!(config.loop_count, Some(1));

        // Infinite loop
        let config_infinite = ProgressionConfig::new(progression.clone()).with_looping();
        assert_eq!(config_infinite.loop_count, None);

        // Specific count
        let config_count = ProgressionConfig::new(progression.clone()).with_loop_count(5);
        assert_eq!(config_count.loop_count, Some(5));
    }

    /// Test multiple rapid queue additions
    #[test]
    fn test_rapid_queue_additions() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                let scheduler = Scheduler::new(120.0);
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler), 1);

                // Rapidly queue many items (simulates repeat 32 { play E queue })
                for i in 0..32 {
                    let freq = 440.0 + (i as f32 * 10.0);
                    let config = ProgressionConfig::new(vec![vec![freq]]);
                    assert!(engine.queue_progression(config).is_ok());
                }

                // All queues should succeed without panic or error
                // Stop to clean up
                assert!(engine.stop().is_ok());
            }
            Err(_) => {
                println!("Skipping test_rapid_queue_additions - no audio device");
            }
        }
    }
}
