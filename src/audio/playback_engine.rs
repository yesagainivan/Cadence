use crate::audio::audio::AudioPlayerHandle;
use crate::audio::scheduler::{Duration, Scheduler};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};

/// Configuration for progression playback
#[derive(Clone)]
pub struct ProgressionConfig {
    /// Each chord as a vector of frequencies (Hz)
    pub progression: Vec<Vec<f32>>,
    /// How long each chord plays
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

/// Handle for a running playback task
pub struct PlaybackTask {
    id: usize,
    running: Arc<AtomicBool>,
    _handle: Option<JoinHandle<()>>,
}

impl PlaybackTask {
    fn new(id: usize, running: Arc<AtomicBool>, handle: JoinHandle<()>) -> Self {
        Self {
            id,
            running,
            _handle: Some(handle),
        }
    }

    /// Stop the playback task
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if the task is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the task ID
    pub fn id(&self) -> usize {
        self.id
    }
}

impl Drop for PlaybackTask {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Engine for managing sequential progression playback
pub struct PlaybackEngine {
    audio_handle: Arc<AudioPlayerHandle>,
    scheduler: Arc<Scheduler>,
    current_task: Arc<std::sync::Mutex<Option<Arc<AtomicBool>>>>,
    task_counter: Arc<AtomicUsize>,
}

impl PlaybackEngine {
    /// Create a new playback engine
    pub fn new(audio_handle: Arc<AudioPlayerHandle>, scheduler: Arc<Scheduler>) -> Self {
        Self {
            audio_handle,
            scheduler,
            current_task: Arc::new(std::sync::Mutex::new(None)),
            task_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Play a progression with the given configuration
    pub fn play_progression(&self, config: ProgressionConfig) -> Result<PlaybackTask> {
        // Stop any existing playback
        self.stop();

        // Create new task ID
        let task_id = self.task_counter.fetch_add(1, Ordering::Relaxed);
        let running = Arc::new(AtomicBool::new(true));

        // Store the running flag
        {
            let mut current = self.current_task.lock().unwrap();
            *current = Some(running.clone());
        }

        // Clone necessary data for the thread
        let audio_handle = self.audio_handle.clone();
        let scheduler = self.scheduler.clone();
        let running_clone = running.clone();

        // Spawn playback thread
        let handle = thread::spawn(move || {
            let loop_count = config.loop_count.unwrap_or(usize::MAX);

            'outer: for _iteration in 0..loop_count {
                if !running_clone.load(Ordering::Relaxed) {
                    break;
                }

                for (i, chord_frequencies) in config.progression.iter().enumerate() {
                    // Check if we should stop
                    if !running_clone.load(Ordering::Relaxed) {
                        break 'outer;
                    }

                    // Set the notes for this chord
                    if let Err(e) = audio_handle.set_notes(chord_frequencies.clone()) {
                        eprintln!("Failed to set notes for chord {}: {}", i, e);
                        continue;
                    }

                    // Start playback if first chord
                    if i == 0 {
                        if let Err(e) = audio_handle.play() {
                            eprintln!("Failed to start audio playback: {}", e);
                            break 'outer;
                        }
                    }

                    // Wait for note duration
                    scheduler.sleep(config.note_duration);

                    // Optional gap between chords
                    if config.gap_duration.to_millis(scheduler.get_bpm()) > 0 {
                        // Pause during gap
                        let _ = audio_handle.pause();
                        scheduler.sleep(config.gap_duration);
                        let _ = audio_handle.play();
                    }
                }
            }

            // Stop audio when done
            let _ = audio_handle.pause();
            running_clone.store(false, Ordering::Relaxed);
        });

        Ok(PlaybackTask::new(task_id, running, handle))
    }

    /// Stop any currently playing progression
    pub fn stop(&self) {
        let mut current = self.current_task.lock().unwrap();
        if let Some(running) = current.as_ref() {
            running.store(false, Ordering::Relaxed);
        }
        *current = None;

        // Also pause the audio player
        let _ = self.audio_handle.pause();
    }

    /// Check if a progression is currently playing
    pub fn is_playing(&self) -> bool {
        let current = self.current_task.lock().unwrap();
        if let Some(running) = current.as_ref() {
            running.load(Ordering::Relaxed)
        } else {
            false
        }
    }
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
    fn test_playback_task_running() {
        let running = Arc::new(AtomicBool::new(true));
        let handle = thread::spawn(|| {
            thread::sleep(std::time::Duration::from_millis(10));
        });

        let task = PlaybackTask::new(1, running.clone(), handle);
        assert!(task.is_running());

        task.stop();
        assert!(!task.is_running());
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
}
