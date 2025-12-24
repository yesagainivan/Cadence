use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration as StdDuration, Instant};

/// Musical duration representation
#[derive(Clone, Copy, Debug)]
pub enum Duration {
    /// Duration in beats (quarter notes in 4/4 time)
    Beats(f32),
    /// Absolute duration in seconds
    Seconds(f32),
    /// Duration in bars (measures, assuming 4/4 time)
    Bars(f32),
}

impl Duration {
    /// Convert to milliseconds based on current BPM
    pub fn to_millis(&self, bpm: f32) -> u64 {
        match self {
            Duration::Beats(beats) => ((beats * 60000.0) / bpm) as u64,
            Duration::Seconds(secs) => (secs * 1000.0) as u64,
            Duration::Bars(bars) => ((bars * 4.0 * 60000.0) / bpm) as u64, // 4 beats per bar
        }
    }

    /// Convert to std::time::Duration based on current BPM
    pub fn to_std_duration(&self, bpm: f32) -> StdDuration {
        StdDuration::from_millis(self.to_millis(bpm))
    }
}

/// High-precision scheduler for musical timing with beat position tracking
pub struct Scheduler {
    bpm: Arc<AtomicU64>, // Store as bits for atomic operations
    running: Arc<AtomicBool>,
    start_time: Arc<std::sync::Mutex<Option<Instant>>>,
    _handle: Option<JoinHandle<()>>,
}

impl Scheduler {
    /// Create a new scheduler with the given BPM
    pub fn new(bpm: f32) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let bpm_atomic = Arc::new(AtomicU64::new(bpm.to_bits() as u64));

        Scheduler {
            bpm: bpm_atomic,
            running,
            start_time: Arc::new(std::sync::Mutex::new(None)),
            _handle: None,
        }
    }

    /// Start the scheduler (marks the start time for beat tracking)
    pub fn start(&self) {
        let mut start = self.start_time.lock().unwrap();
        if start.is_none() {
            *start = Some(Instant::now());
        }
    }

    /// Reset the scheduler (resets start time)
    pub fn reset(&self) {
        let mut start = self.start_time.lock().unwrap();
        *start = Some(Instant::now());
    }

    /// Set the BPM (tempo)
    pub fn set_bpm(&self, bpm: f32) {
        self.bpm.store(bpm.to_bits() as u64, Ordering::Relaxed);
    }

    /// Get the current BPM
    pub fn get_bpm(&self) -> f32 {
        f32::from_bits(self.bpm.load(Ordering::Relaxed) as u32)
    }

    /// Get duration of a single beat in milliseconds
    pub fn beat_duration_ms(&self) -> u64 {
        let bpm = self.get_bpm();
        (60000.0 / bpm) as u64
    }

    /// Get the current beat position (fractional beats since start)
    pub fn current_beat(&self) -> f64 {
        let start = self.start_time.lock().unwrap();
        if let Some(start_time) = *start {
            let elapsed = start_time.elapsed();
            let bpm = self.get_bpm() as f64;
            let beats_per_sec = bpm / 60.0;
            elapsed.as_secs_f64() * beats_per_sec
        } else {
            0.0
        }
    }

    /// Get the time until the next beat boundary
    pub fn time_to_next_beat(&self) -> StdDuration {
        let current = self.current_beat();
        let next_beat = current.ceil();
        let beats_until_next = next_beat - current;

        let bpm = self.get_bpm() as f64;
        let seconds_per_beat = 60.0 / bpm;
        let seconds_to_next = beats_until_next * seconds_per_beat;

        StdDuration::from_secs_f64(seconds_to_next.max(0.001)) // At least 1ms
    }

    /// Get the Instant when the next beat will occur
    pub fn next_beat_time(&self) -> Instant {
        Instant::now() + self.time_to_next_beat()
    }

    /// Wait until the next beat boundary
    pub fn wait_for_next_beat(&self) -> bool {
        if !self.running.load(Ordering::Relaxed) {
            return false;
        }
        thread::sleep(self.time_to_next_beat());
        true
    }

    /// Create a future time instant based on a duration
    pub fn time_from_now(&self, duration: Duration) -> Instant {
        let bpm = self.get_bpm();
        Instant::now() + duration.to_std_duration(bpm)
    }

    /// Sleep for a musical duration
    pub fn sleep(&self, duration: Duration) {
        let bpm = self.get_bpm();
        let std_duration = duration.to_std_duration(bpm);
        thread::sleep(std_duration);
    }

    /// Sleep until a specific instant, checking periodically if we should stop
    pub fn sleep_until(&self, target: Instant, running: &Arc<AtomicBool>) -> bool {
        while Instant::now() < target {
            if !running.load(Ordering::Relaxed) {
                return false; // Interrupted
            }
            // Sleep in small increments for responsiveness
            thread::sleep(StdDuration::from_millis(5));
        }
        true // Completed sleep
    }

    /// Check if scheduler is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_bpm() {
        let scheduler = Scheduler::new(120.0);
        assert_eq!(scheduler.get_bpm(), 120.0);

        scheduler.set_bpm(90.0);
        assert_eq!(scheduler.get_bpm(), 90.0);
    }

    #[test]
    fn test_beat_duration() {
        let scheduler = Scheduler::new(120.0);
        // At 120 BPM, one beat = 0.5 seconds = 500ms
        assert_eq!(scheduler.beat_duration_ms(), 500);

        scheduler.set_bpm(60.0);
        // At 60 BPM, one beat = 1 second = 1000ms
        assert_eq!(scheduler.beat_duration_ms(), 1000);
    }

    #[test]
    fn test_duration_conversion() {
        let bpm = 120.0;

        // 1 beat at 120 BPM = 500ms
        assert_eq!(Duration::Beats(1.0).to_millis(bpm), 500);

        // 2 beats at 120 BPM = 1000ms
        assert_eq!(Duration::Beats(2.0).to_millis(bpm), 1000);

        // 1 bar (4 beats) at 120 BPM = 2000ms
        assert_eq!(Duration::Bars(1.0).to_millis(bpm), 2000);

        // 1.5 seconds = 1500ms
        assert_eq!(Duration::Seconds(1.5).to_millis(bpm), 1500);
    }

    #[test]
    fn test_duration_at_different_tempos() {
        // 1 beat at different BPMs
        assert_eq!(Duration::Beats(1.0).to_millis(60.0), 1000); // 60 BPM
        assert_eq!(Duration::Beats(1.0).to_millis(120.0), 500); // 120 BPM
        assert_eq!(Duration::Beats(1.0).to_millis(180.0), 333); // 180 BPM
    }

    #[test]
    fn test_sleep_duration() {
        let scheduler = Scheduler::new(120.0);
        let start = Instant::now();

        // Sleep for a very short duration to keep test fast
        scheduler.sleep(Duration::Beats(0.1)); // 50ms at 120 BPM

        let elapsed = start.elapsed();
        // Allow some tolerance for timing
        assert!(elapsed.as_millis() >= 40 && elapsed.as_millis() <= 100);
    }
}
