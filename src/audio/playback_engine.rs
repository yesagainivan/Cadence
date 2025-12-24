//! Beat-quantized progression playback engine
//!
//! This module provides a production-ready audio progression system that enables
//! seamless, beat-synchronized switching between progressions—inspired by live
//! coding environments like Sonic Pi and TidalCycles.

use crate::audio::audio::AudioPlayerHandle;
use crate::audio::scheduler::{Duration, Scheduler};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::{self, JoinHandle};
use std::time::Instant;

/// Configuration for progression playback
#[derive(Clone, Debug)]
pub struct ProgressionConfig {
    /// Each chord as a vector of frequencies (Hz)
    pub progression: Vec<Vec<f32>>,
    /// How long each chord plays (in beats)
    pub note_duration: Duration,
    /// Gap between chords (default: 0, seamless transition)
    pub gap_duration: Duration,
    /// Number of times to loop (None = infinite loop)
    pub loop_count: Option<usize>,
}

impl ProgressionConfig {
    /// Create a new progression config with default values
    pub fn new(progression: Vec<Vec<f32>>) -> Self {
        Self {
            progression,
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
    /// Shutdown the playback engine
    Shutdown,
}

/// Engine for managing sequential progression playback with beat-quantized switching
pub struct PlaybackEngine {
    command_tx: Sender<PlaybackCommand>,
    is_playing: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
}

impl PlaybackEngine {
    /// Create a new playback engine with a persistent playback thread
    pub fn new(audio_handle: Arc<AudioPlayerHandle>, scheduler: Arc<Scheduler>) -> Self {
        let (tx, rx) = channel();
        let is_playing = Arc::new(AtomicBool::new(false));
        let is_playing_clone = is_playing.clone();

        let thread = thread::spawn(move || {
            PlaybackLoop::new(audio_handle, scheduler, rx, is_playing_clone).run();
        });

        PlaybackEngine {
            command_tx: tx,
            is_playing,
            _thread: thread,
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
    pending_progression: Option<ProgressionConfig>,
    chord_index: usize,
    iteration: usize,
    audio_started: bool,
}

impl PlaybackLoop {
    fn new(
        audio_handle: Arc<AudioPlayerHandle>,
        scheduler: Arc<Scheduler>,
        command_rx: Receiver<PlaybackCommand>,
        is_playing: Arc<AtomicBool>,
    ) -> Self {
        Self {
            audio_handle,
            scheduler,
            command_rx,
            is_playing,
            current_progression: None,
            pending_progression: None,
            chord_index: 0,
            iteration: 0,
            audio_started: false,
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

        // Clean up
        let _ = self.audio_handle.pause();
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
                self.pending_progression = None;
                self.chord_index = 0;
                self.iteration = 0;
                self.is_playing.store(true, Ordering::Relaxed);

                // Reset scheduler timing
                self.scheduler.reset();

                // Don't start audio yet - it will start on first beat
                self.audio_started = false;
            }
            PlaybackCommand::QueueProgression(config) => {
                // Queue for next beat boundary - seamless transition
                if self.current_progression.is_some() {
                    self.pending_progression = Some(config);
                } else {
                    // No current progression - start immediately
                    self.current_progression = Some(config);
                    self.chord_index = 0;
                    self.iteration = 0;
                    self.is_playing.store(true, Ordering::Relaxed);
                    self.scheduler.reset();
                    self.audio_started = false;
                }
            }
            PlaybackCommand::Stop => {
                self.current_progression = None;
                self.pending_progression = None;
                self.is_playing.store(false, Ordering::Relaxed);
                let _ = self.audio_handle.pause();
                self.audio_started = false;
            }
            PlaybackCommand::Shutdown => {
                return LoopAction::Shutdown;
            }
        }
        LoopAction::Continue
    }

    fn play_next_beat(&mut self) {
        // Check for pending progression at beat boundary (before playing current chord)
        if let Some(pending) = self.pending_progression.take() {
            // Seamless switch - don't pause, just change progression
            self.current_progression = Some(pending);
            self.chord_index = 0;
            self.iteration = 0;
            // Note: we keep audio_started true for seamless transition
        }

        let config = match &self.current_progression {
            Some(c) => c.clone(),
            None => return,
        };

        // Check if we've completed all iterations
        if let Some(max_loops) = config.loop_count {
            if self.iteration >= max_loops {
                self.current_progression = None;
                self.is_playing.store(false, Ordering::Relaxed);
                let _ = self.audio_handle.pause();
                self.audio_started = false;
                return;
            }
        }

        // Get current chord
        if self.chord_index >= config.progression.len() {
            // Move to next iteration
            self.chord_index = 0;
            self.iteration += 1;

            // Re-check loop count
            if let Some(max_loops) = config.loop_count {
                if self.iteration >= max_loops {
                    self.current_progression = None;
                    self.is_playing.store(false, Ordering::Relaxed);
                    let _ = self.audio_handle.pause();
                    self.audio_started = false;
                    return;
                }
            }
        }

        let chord_frequencies = &config.progression[self.chord_index];

        // Set the notes (seamless - no pause needed)
        if let Err(e) = self.audio_handle.set_notes(chord_frequencies.clone()) {
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

        // Wait for beat duration, checking for commands periodically
        let beat_end = self.scheduler.time_from_now(config.note_duration);
        self.wait_until_with_command_check(beat_end);

        // Handle gap if specified
        let gap_ms = config.gap_duration.to_millis(self.scheduler.get_bpm());
        if gap_ms > 0 {
            let _ = self.audio_handle.pause();
            self.scheduler.sleep(config.gap_duration);
            let _ = self.audio_handle.play();
        }
    }

    /// Wait until a specific time, but check for commands periodically
    fn wait_until_with_command_check(&mut self, target: Instant) {
        while Instant::now() < target {
            // Check for high-priority commands (Stop/Shutdown) more frequently
            match self.command_rx.try_recv() {
                Ok(PlaybackCommand::Stop) => {
                    self.current_progression = None;
                    self.pending_progression = None;
                    self.is_playing.store(false, Ordering::Relaxed);
                    let _ = self.audio_handle.pause();
                    self.audio_started = false;
                    return;
                }
                Ok(PlaybackCommand::Shutdown) => {
                    return;
                }
                Ok(PlaybackCommand::QueueProgression(config)) => {
                    // Queue for next beat
                    self.pending_progression = Some(config);
                }
                Ok(PlaybackCommand::PlayProgression(config)) => {
                    // Immediate switch even mid-beat
                    self.current_progression = Some(config);
                    self.pending_progression = None;
                    self.chord_index = 0;
                    self.iteration = 0;
                    self.scheduler.reset();
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

        assert_eq!(config.progression.len(), 2);
        assert!(config.loop_count.is_none());
    }

    #[test]
    fn test_playback_engine_creation() {
        match AudioPlayerHandle::new() {
            Ok(handle) => {
                let scheduler = Scheduler::new(120.0);
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler));
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
                let engine = PlaybackEngine::new(Arc::new(handle), Arc::new(scheduler));

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
}
