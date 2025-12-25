//! Master Clock for synchronized multi-track playback
//!
//! Provides a centralized timing source that broadcasts tick events to all
//! audio tracks simultaneously, ensuring perfect synchronization.
//!
//! Follows the MIDI clock standard of 24 PPQN (pulses per quarter note).

use crossbeam_channel::{Receiver, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration as StdDuration, Instant};

/// Ticks per quarter note (MIDI standard)
pub const TICKS_PER_BEAT: u8 = 24;

/// A single clock tick event broadcast to all subscribers
#[derive(Clone, Debug)]
pub struct ClockTick {
    /// Current beat position (fractional, e.g., 4.5 = halfway through beat 5)
    pub beat: f64,
    /// Integer beat count since clock started (0-indexed)
    pub beat_number: u64,
    /// Tick within current beat (0-23 for 24 PPQN)
    pub tick_in_beat: u8,
    /// The instant this tick was generated (for precise timing)
    pub timestamp: Instant,
}

impl ClockTick {
    /// Returns true if this tick is on a beat boundary (tick 0 of a beat)
    pub fn is_beat_boundary(&self) -> bool {
        self.tick_in_beat == 0
    }

    /// Returns true if this tick is on a bar boundary (beat 0, 4, 8, 12... in 4/4 time)
    pub fn is_bar_boundary(&self) -> bool {
        self.tick_in_beat == 0 && self.beat_number % 4 == 0
    }

    /// Returns true if this tick is on a subdivision boundary.
    /// - subdivision 2: 8th notes (every 12 ticks)
    /// - subdivision 4: 16th notes (every 6 ticks)
    /// - subdivision 3: triplets (every 8 ticks)
    /// - subdivision 6: 16th triplets (every 4 ticks)
    pub fn is_subdivision_boundary(&self, subdivision: u8) -> bool {
        if subdivision == 0 {
            return false;
        }
        let ticks_per_subdivision = TICKS_PER_BEAT / subdivision;
        if ticks_per_subdivision == 0 {
            return true; // subdivision finer than our resolution, treat every tick as a boundary
        }
        self.tick_in_beat % ticks_per_subdivision == 0
    }

    /// Returns true if this tick is on a half-beat (8th note) boundary
    pub fn is_half_beat(&self) -> bool {
        self.is_subdivision_boundary(2)
    }

    /// Returns true if this tick is on a quarter-beat (16th note) boundary
    pub fn is_quarter_beat(&self) -> bool {
        self.is_subdivision_boundary(4)
    }
}

/// Commands that can be sent to the clock thread
#[derive(Debug)]
enum ClockCommand {
    Start,
    Stop,
    Reset,
    SetBpm(f32),
    AddSubscriber(CrossbeamSender<ClockTick>),
    Shutdown,
}

/// Master clock that runs in its own thread and broadcasts tick events
pub struct MasterClock {
    /// BPM stored as bits for atomic operations
    bpm: Arc<AtomicU64>,
    /// Whether the clock is currently running
    running: Arc<AtomicBool>,
    /// Command sender to control the clock thread
    command_tx: crossbeam_channel::Sender<ClockCommand>,
    /// Clock thread handle
    thread: Option<JoinHandle<()>>,
}

impl MasterClock {
    /// Create a new master clock with the given initial BPM
    pub fn new(bpm: f32) -> Self {
        let bpm_atomic = Arc::new(AtomicU64::new(bpm.to_bits() as u64));
        let running = Arc::new(AtomicBool::new(false));
        let (command_tx, command_rx) = crossbeam_channel::bounded(64);

        let bpm_clone = bpm_atomic.clone();
        let running_clone = running.clone();

        let thread = thread::spawn(move || {
            ClockThread::new(bpm_clone, running_clone, command_rx).run();
        });

        MasterClock {
            bpm: bpm_atomic,
            running,
            command_tx,
            thread: Some(thread),
        }
    }

    /// Create a new subscriber that will receive tick events
    ///
    /// Multiple subscribers can be created - all receive the same ticks simultaneously
    pub fn subscribe(&self) -> Receiver<ClockTick> {
        let (tx, rx) = unbounded();
        let _ = self.command_tx.send(ClockCommand::AddSubscriber(tx));
        rx
    }

    /// Start the clock (begins generating ticks)
    pub fn start(&self) {
        let _ = self.command_tx.send(ClockCommand::Start);
    }

    /// Stop the clock (pauses tick generation)
    pub fn stop(&self) {
        let _ = self.command_tx.send(ClockCommand::Stop);
    }

    /// Reset the clock (resets beat counter to 0)
    pub fn reset(&self) {
        let _ = self.command_tx.send(ClockCommand::Reset);
    }

    /// Set the tempo in BPM
    pub fn set_bpm(&self, bpm: f32) {
        self.bpm.store(bpm.to_bits() as u64, Ordering::Relaxed);
        let _ = self.command_tx.send(ClockCommand::SetBpm(bpm));
    }

    /// Get the current BPM
    pub fn get_bpm(&self) -> f32 {
        f32::from_bits(self.bpm.load(Ordering::Relaxed) as u32)
    }

    /// Check if the clock is currently running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get duration of a single beat in milliseconds at current BPM
    pub fn beat_duration_ms(&self) -> u64 {
        let bpm = self.get_bpm();
        (60000.0 / bpm) as u64
    }
}

impl Drop for MasterClock {
    fn drop(&mut self) {
        let _ = self.command_tx.send(ClockCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Alias for crossbeam sender
type CrossbeamSender<T> = crossbeam_channel::Sender<T>;

/// Internal clock thread that generates ticks
struct ClockThread {
    bpm: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    command_rx: Receiver<ClockCommand>,
    /// List of subscribers to broadcast ticks to
    subscribers: Vec<CrossbeamSender<ClockTick>>,

    // Timing state
    beat_number: u64,
    tick_in_beat: u8,
    start_time: Option<Instant>,
}

impl ClockThread {
    fn new(
        bpm: Arc<AtomicU64>,
        running: Arc<AtomicBool>,
        command_rx: Receiver<ClockCommand>,
    ) -> Self {
        Self {
            bpm,
            running,
            command_rx,
            subscribers: Vec::new(),
            beat_number: 0,
            tick_in_beat: 0,
            start_time: None,
        }
    }

    fn get_bpm(&self) -> f32 {
        f32::from_bits(self.bpm.load(Ordering::Relaxed) as u32)
    }

    /// Calculate duration between ticks based on current BPM
    fn tick_duration(&self) -> StdDuration {
        let bpm = self.get_bpm();
        let beat_duration_secs = 60.0 / bpm as f64;
        let tick_duration_secs = beat_duration_secs / TICKS_PER_BEAT as f64;
        StdDuration::from_secs_f64(tick_duration_secs)
    }

    fn run(&mut self) {
        let mut next_tick_time: Option<Instant> = None;

        loop {
            // Check for commands (non-blocking when running, blocking when stopped)
            if self.running.load(Ordering::Relaxed) {
                // Non-blocking check for commands while running
                match self.command_rx.try_recv() {
                    Ok(cmd) => {
                        if self.handle_command(cmd) {
                            break;
                        }
                    }
                    Err(_) => {} // No command, continue
                }

                // Generate tick if it's time
                let now = Instant::now();
                if let Some(target) = next_tick_time {
                    if now >= target {
                        self.emit_tick();
                        self.advance_tick();
                        next_tick_time = Some(target + self.tick_duration());
                    } else {
                        // Spin-wait with small sleeps for precision
                        let remaining = target - now;
                        if remaining > StdDuration::from_micros(500) {
                            thread::sleep(StdDuration::from_micros(100));
                        } else {
                            // Busy-wait for final precision
                            std::hint::spin_loop();
                        }
                    }
                } else if self.start_time.is_some() {
                    // Just started, emit first tick immediately
                    self.emit_tick();
                    self.advance_tick();
                    next_tick_time = Some(Instant::now() + self.tick_duration());
                }
            } else {
                // Blocking wait for commands when stopped
                match self.command_rx.recv() {
                    Ok(cmd) => {
                        if self.handle_command(cmd) {
                            break;
                        }
                        if self.running.load(Ordering::Relaxed) {
                            // Just started, set up next tick
                            next_tick_time = Some(Instant::now());
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: ClockCommand) -> bool {
        match cmd {
            ClockCommand::Start => {
                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }
                self.running.store(true, Ordering::Relaxed);
            }
            ClockCommand::Stop => {
                self.running.store(false, Ordering::Relaxed);
            }
            ClockCommand::Reset => {
                self.beat_number = 0;
                self.tick_in_beat = 0;
                self.start_time = Some(Instant::now());
            }
            ClockCommand::SetBpm(_bpm) => {
                // BPM is already stored atomically, tick_duration() will pick it up
            }
            ClockCommand::AddSubscriber(tx) => {
                self.subscribers.push(tx);
            }
            ClockCommand::Shutdown => {
                self.running.store(false, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    fn emit_tick(&mut self) {
        let beat = self.beat_number as f64 + (self.tick_in_beat as f64 / TICKS_PER_BEAT as f64);
        let tick = ClockTick {
            beat,
            beat_number: self.beat_number,
            tick_in_beat: self.tick_in_beat,
            timestamp: Instant::now(),
        };
        // Broadcast to all subscribers, removing disconnected ones
        self.subscribers.retain(|tx| tx.send(tick.clone()).is_ok());
    }

    fn advance_tick(&mut self) {
        self.tick_in_beat += 1;
        if self.tick_in_beat >= TICKS_PER_BEAT {
            self.tick_in_beat = 0;
            self.beat_number += 1;
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_creation() {
        let clock = MasterClock::new(120.0);
        assert_eq!(clock.get_bpm(), 120.0);
        assert!(!clock.is_running());
    }

    #[test]
    fn test_bpm_change() {
        let clock = MasterClock::new(120.0);
        clock.set_bpm(90.0);
        assert_eq!(clock.get_bpm(), 90.0);
    }

    #[test]
    fn test_beat_duration() {
        let clock = MasterClock::new(120.0);
        // At 120 BPM, one beat = 500ms
        assert_eq!(clock.beat_duration_ms(), 500);

        clock.set_bpm(60.0);
        // At 60 BPM, one beat = 1000ms
        assert_eq!(clock.beat_duration_ms(), 1000);
    }

    #[test]
    fn test_tick_is_beat_boundary() {
        let tick_on_beat = ClockTick {
            beat: 4.0,
            beat_number: 4,
            tick_in_beat: 0,
            timestamp: Instant::now(),
        };
        assert!(tick_on_beat.is_beat_boundary());

        let tick_off_beat = ClockTick {
            beat: 4.5,
            beat_number: 4,
            tick_in_beat: 12,
            timestamp: Instant::now(),
        };
        assert!(!tick_off_beat.is_beat_boundary());
    }

    #[test]
    fn test_clock_start_stop() {
        let clock = MasterClock::new(120.0);

        assert!(!clock.is_running());
        clock.start();
        thread::sleep(StdDuration::from_millis(50));
        assert!(clock.is_running());

        clock.stop();
        thread::sleep(StdDuration::from_millis(50));
        assert!(!clock.is_running());
    }
}
