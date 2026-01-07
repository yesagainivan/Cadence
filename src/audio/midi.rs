//! MIDI output module for Cadence
//!
//! Provides thread-safe MIDI output using midir, with a channel-based
//! architecture that mirrors AudioPlayerHandle.

use anyhow::{anyhow, Result};
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, RwLock};
use std::thread::{self, JoinHandle};

/// Convert a Note (pitch_class + octave) to MIDI note number
/// MIDI note 60 = Middle C (C4 in scientific pitch notation)
/// Formula: midi_note = (octave + 1) * 12 + pitch_class
pub fn note_to_midi(pitch_class: u8, octave: i8) -> u8 {
    let midi_note = (octave + 1) as i16 * 12 + pitch_class as i16;
    midi_note.clamp(0, 127) as u8
}

/// Convert frequency back to MIDI note number (approximate)
/// Uses A4 = 440Hz = MIDI 69 as reference
pub fn frequency_to_midi(freq: f32) -> u8 {
    if freq <= 0.0 {
        return 0;
    }
    let midi_note = 69.0 + 12.0 * (freq / 440.0).log2();
    (midi_note.round() as i32).clamp(0, 127) as u8
}

/// MIDI channel mode configuration
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiChannelMode {
    /// Each track maps to its own MIDI channel (Track 0 → Ch 1, Track 1 → Ch 2, etc.)
    PerTrack,
    /// All tracks output to a single MIDI channel
    Mono(u8),
}

impl Default for MidiChannelMode {
    fn default() -> Self {
        MidiChannelMode::PerTrack
    }
}

/// Output mode: audio only, MIDI only, or both
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum OutputMode {
    /// Output both internal audio synthesis and MIDI (default)
    #[default]
    Both,
    /// Output MIDI only (silence from internal synth)
    MidiOnly,
    /// Output audio only (no MIDI messages sent)
    AudioOnly,
}

/// Commands that can be sent to the MIDI output thread
#[derive(Debug, Clone)]
pub enum MidiCommand {
    /// Connect to a MIDI port by name
    Connect { port_name: String },
    /// Send Note On: channel (0-15), note (0-127), velocity (0-127)
    NoteOn { channel: u8, note: u8, velocity: u8 },
    /// Send Note Off: channel (0-15), note (0-127)
    NoteOff { channel: u8, note: u8 },
    /// Send Control Change: channel, controller number, value
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// Send All Notes Off on specified channel
    AllNotesOff { channel: u8 },
    /// Disconnect from MIDI port
    Disconnect,
    /// Shutdown the MIDI thread
    Shutdown,
}

/// Internal MIDI output handler that owns the connection
struct MidiOutputInternal {
    connection: Option<MidiOutputConnection>,
    command_rx: std::sync::mpsc::Receiver<MidiCommand>,
}

impl MidiOutputInternal {
    fn new(command_rx: std::sync::mpsc::Receiver<MidiCommand>) -> Self {
        Self {
            connection: None,
            command_rx,
        }
    }

    fn connect(&mut self, port_name: &str) -> Result<()> {
        let midi_out = MidiOutput::new("Cadence")?;
        let ports = midi_out.ports();

        let port = ports
            .iter()
            .find(|p| {
                midi_out
                    .port_name(p)
                    .map(|name| name.contains(port_name))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow!("MIDI port '{}' not found", port_name))?;

        let connection = midi_out.connect(port, "cadence-out")?;
        self.connection = Some(connection);
        Ok(())
    }

    fn run(&mut self) {
        while let Ok(cmd) = self.command_rx.recv() {
            match cmd {
                MidiCommand::Connect { port_name } => {
                    if let Err(e) = self.connect(&port_name) {
                        eprintln!("MIDI connect error: {}", e);
                    }
                }
                MidiCommand::NoteOn {
                    channel,
                    note,
                    velocity,
                } => {
                    if let Some(conn) = &mut self.connection {
                        // MIDI Note On: 0x90 + channel, note, velocity
                        let _ = conn.send(&[0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F]);
                    }
                }
                MidiCommand::NoteOff { channel, note } => {
                    if let Some(conn) = &mut self.connection {
                        // MIDI Note Off: 0x80 + channel, note, velocity 0
                        let _ = conn.send(&[0x80 | (channel & 0x0F), note & 0x7F, 0]);
                    }
                }
                MidiCommand::ControlChange {
                    channel,
                    controller,
                    value,
                } => {
                    if let Some(conn) = &mut self.connection {
                        // MIDI CC: 0xB0 + channel, controller, value
                        let _ =
                            conn.send(&[0xB0 | (channel & 0x0F), controller & 0x7F, value & 0x7F]);
                    }
                }
                MidiCommand::AllNotesOff { channel } => {
                    if let Some(conn) = &mut self.connection {
                        // All Notes Off: CC 123, value 0
                        let _ = conn.send(&[0xB0 | (channel & 0x0F), 123, 0]);
                    }
                }
                MidiCommand::Disconnect => {
                    // Graceful disconnect: send All Notes Off on all channels first
                    if let Some(conn) = &mut self.connection {
                        for ch in 0..16u8 {
                            let _ = conn.send(&[0xB0 | ch, 123, 0]);
                        }
                        // Give CoreMIDI time to process the messages before closing
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    self.connection = None;
                }
                MidiCommand::Shutdown => {
                    // Send All Notes Off on all channels before shutting down
                    if let Some(conn) = &mut self.connection {
                        for ch in 0..16u8 {
                            let _ = conn.send(&[0xB0 | ch, 123, 0]);
                        }
                    }
                    break;
                }
            }
        }
    }
}

/// Thread-safe handle to the MIDI output
/// Uses internal channels to communicate with the MIDI thread
pub struct MidiOutputHandle {
    command_tx: Sender<MidiCommand>,
    /// Thread handle wrapped in Option so we can take() it in Drop
    thread: Option<JoinHandle<()>>,
    /// Current channel mode
    channel_mode: RwLock<MidiChannelMode>,
    /// Output mode: Both, MidiOnly, or AudioOnly
    output_mode: RwLock<OutputMode>,
    /// Track which notes are currently active per channel for proper Note Off
    /// Key: (channel, note), Value: true if active
    active_notes: Mutex<std::collections::HashSet<(u8, u8)>>,
    /// Whether we're connected to a MIDI port
    connected: RwLock<bool>,
    /// Name of the connected port
    port_name: RwLock<Option<String>>,
    /// Cached MidiOutput for port enumeration (avoids creating new CoreMIDI client each time)
    port_enumerator: Mutex<Option<MidiOutput>>,
}

impl MidiOutputHandle {
    /// Create a new MIDI output handle (not connected to any port yet)
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel();

        let thread = thread::spawn(move || {
            let mut internal = MidiOutputInternal::new(rx);
            internal.run();
        });

        // NOTE: We do NOT create MidiOutput here!
        // Creating a CoreMIDI client at startup can deadlock the system MIDI server
        // if a previous instance didn't clean up properly. We defer creation until
        // the user explicitly calls list_ports() or connect().
        let port_enumerator = None;

        Ok(Self {
            command_tx: tx,
            thread: Some(thread),
            channel_mode: RwLock::new(MidiChannelMode::default()),
            output_mode: RwLock::new(OutputMode::default()),
            active_notes: Mutex::new(std::collections::HashSet::new()),
            connected: RwLock::new(false),
            port_name: RwLock::new(None),
            port_enumerator: Mutex::new(port_enumerator),
        })
    }

    /// List available MIDI output ports
    /// Uses cached MidiOutput to avoid creating new CoreMIDI client each time.
    /// Falls back to creating new one if cache is empty.
    pub fn list_ports(&self) -> Result<Vec<String>> {
        let mut enumerator = self.port_enumerator.lock().unwrap();

        // If we have a cached enumerator, use it
        if let Some(ref midi_out) = *enumerator {
            let ports = midi_out.ports();
            let names: Vec<String> = ports
                .iter()
                .filter_map(|p| midi_out.port_name(p).ok())
                .collect();
            return Ok(names);
        }

        // Otherwise try to create a new one
        match MidiOutput::new("Cadence-Enumerator") {
            Ok(midi_out) => {
                let ports = midi_out.ports();
                let names: Vec<String> = ports
                    .iter()
                    .filter_map(|p| midi_out.port_name(p).ok())
                    .collect();
                // Cache for next time
                *enumerator = Some(midi_out);
                Ok(names)
            }
            Err(e) => Err(anyhow!("Failed to initialize MIDI: {:?}", e)),
        }
    }

    /// Connect to a MIDI output port by name (partial match supported)
    /// Uses the shared port_enumerator to validate without creating redundant CoreMIDI clients.
    pub fn connect(&self, port_name: &str) -> Result<()> {
        // Validate port exists using the shared enumerator (avoid creating redundant CoreMIDI clients)
        let actual_name = {
            let mut enumerator = self.port_enumerator.lock().unwrap();

            // Create enumerator lazily if needed
            if enumerator.is_none() {
                *enumerator = Some(MidiOutput::new("Cadence-Enumerator")?);
            }

            let midi_out = enumerator.as_ref().unwrap();
            let ports = midi_out.ports();

            let port = ports
                .iter()
                .find(|p| {
                    midi_out
                        .port_name(p)
                        .map(|name| name.contains(port_name))
                        .unwrap_or(false)
                })
                .ok_or_else(|| anyhow!("MIDI port '{}' not found", port_name))?;

            midi_out.port_name(port)?
        };

        // Send connect command to the MIDI thread (which will create its own connection)
        self.command_tx
            .send(MidiCommand::Connect {
                port_name: port_name.to_string(),
            })
            .map_err(|e| anyhow!("Failed to send connect command: {}", e))?;

        // Update connection state
        {
            let mut connected = self.connected.write().unwrap();
            let mut stored_name = self.port_name.write().unwrap();
            *connected = true;
            *stored_name = Some(actual_name);
        }

        Ok(())
    }

    /// Disconnect from the current MIDI port
    pub fn disconnect(&self) -> Result<()> {
        self.command_tx
            .send(MidiCommand::Disconnect)
            .map_err(|e| anyhow!("Failed to send disconnect: {}", e))?;

        {
            let mut connected = self.connected.write().unwrap();
            let mut stored_name = self.port_name.write().unwrap();
            *connected = false;
            *stored_name = None;
        }

        // Clear active notes
        if let Ok(mut notes) = self.active_notes.lock() {
            notes.clear();
        }

        Ok(())
    }

    /// Check if connected to a MIDI port
    pub fn is_connected(&self) -> bool {
        *self.connected.read().unwrap()
    }

    /// Get the name of the connected port
    pub fn connected_port(&self) -> Option<String> {
        self.port_name.read().unwrap().clone()
    }

    /// Set the channel mode
    pub fn set_channel_mode(&self, mode: MidiChannelMode) {
        if let Ok(mut m) = self.channel_mode.write() {
            *m = mode;
        }
    }

    /// Get the channel mode
    pub fn channel_mode(&self) -> MidiChannelMode {
        *self.channel_mode.read().unwrap()
    }

    /// Set the output mode (Both, MidiOnly, or AudioOnly)
    pub fn set_output_mode(&self, mode: OutputMode) {
        if let Ok(mut m) = self.output_mode.write() {
            *m = mode;
        }
    }

    /// Get the output mode
    pub fn output_mode(&self) -> OutputMode {
        *self.output_mode.read().unwrap()
    }

    /// Check if audio output is enabled
    pub fn audio_enabled(&self) -> bool {
        matches!(self.output_mode(), OutputMode::Both | OutputMode::AudioOnly)
    }

    /// Check if MIDI output is enabled
    pub fn midi_enabled(&self) -> bool {
        matches!(self.output_mode(), OutputMode::Both | OutputMode::MidiOnly)
    }

    /// Get the MIDI channel for a given track ID
    pub fn channel_for_track(&self, track_id: usize) -> u8 {
        match self.channel_mode() {
            MidiChannelMode::PerTrack => (track_id as u8) & 0x0F, // Clamp to 0-15
            MidiChannelMode::Mono(ch) => ch & 0x0F,
        }
    }

    /// Send Note On for a track
    pub fn note_on(&self, track_id: usize, note: u8, velocity: u8) -> Result<()> {
        let channel = self.channel_for_track(track_id);

        // Track active note
        if let Ok(mut notes) = self.active_notes.lock() {
            notes.insert((channel, note));
        }

        self.command_tx
            .send(MidiCommand::NoteOn {
                channel,
                note,
                velocity,
            })
            .map_err(|e| anyhow!("Failed to send note on: {}", e))
    }

    /// Send Note Off for a track
    pub fn note_off(&self, track_id: usize, note: u8) -> Result<()> {
        let channel = self.channel_for_track(track_id);

        // Remove from active notes
        if let Ok(mut notes) = self.active_notes.lock() {
            notes.remove(&(channel, note));
        }

        self.command_tx
            .send(MidiCommand::NoteOff { channel, note })
            .map_err(|e| anyhow!("Failed to send note off: {}", e))
    }

    /// Send Note On for multiple notes (chord)
    pub fn notes_on(&self, track_id: usize, notes: &[u8], velocity: u8) -> Result<()> {
        for &note in notes {
            self.note_on(track_id, note, velocity)?;
        }
        Ok(())
    }

    /// Send Note Off for multiple notes (chord)
    pub fn notes_off(&self, track_id: usize, notes: &[u8]) -> Result<()> {
        for &note in notes {
            self.note_off(track_id, note)?;
        }
        Ok(())
    }

    /// Turn off all active notes for a track
    pub fn all_notes_off_for_track(&self, track_id: usize) -> Result<()> {
        let channel = self.channel_for_track(track_id);

        // Get and clear active notes for this channel
        let notes_to_off: Vec<u8> = if let Ok(mut notes) = self.active_notes.lock() {
            let channel_notes: Vec<u8> = notes
                .iter()
                .filter(|(ch, _)| *ch == channel)
                .map(|(_, note)| *note)
                .collect();

            for note in &channel_notes {
                notes.remove(&(channel, *note));
            }
            channel_notes
        } else {
            vec![]
        };

        // Send Note Off for each
        for note in notes_to_off {
            self.command_tx
                .send(MidiCommand::NoteOff { channel, note })
                .map_err(|e| anyhow!("Failed to send note off: {}", e))?;
        }

        Ok(())
    }

    /// Send All Notes Off message for a track's channel
    pub fn panic(&self, track_id: usize) -> Result<()> {
        let channel = self.channel_for_track(track_id);
        self.command_tx
            .send(MidiCommand::AllNotesOff { channel })
            .map_err(|e| anyhow!("Failed to send all notes off: {}", e))
    }

    /// Send All Notes Off on all channels (MIDI panic)
    pub fn panic_all(&self) -> Result<()> {
        for ch in 0..16u8 {
            self.command_tx
                .send(MidiCommand::AllNotesOff { channel: ch })
                .map_err(|e| anyhow!("Failed to send all notes off: {}", e))?;
        }

        // Clear active notes tracking
        if let Ok(mut notes) = self.active_notes.lock() {
            notes.clear();
        }

        Ok(())
    }

    /// Send Control Change message for a track
    /// controller: 0-127 (standard MIDI CC numbers)
    /// value: 0-127
    pub fn cc(&self, track_id: usize, controller: u8, value: u8) -> Result<()> {
        let channel = self.channel_for_track(track_id);
        self.command_tx
            .send(MidiCommand::ControlChange {
                channel,
                controller: controller & 0x7F,
                value: value & 0x7F,
            })
            .map_err(|e| anyhow!("Failed to send CC: {}", e))
    }

    /// Send Control Change on a specific channel (bypasses track mapping)
    pub fn cc_on_channel(&self, channel: u8, controller: u8, value: u8) -> Result<()> {
        self.command_tx
            .send(MidiCommand::ControlChange {
                channel: channel & 0x0F,
                controller: controller & 0x7F,
                value: value & 0x7F,
            })
            .map_err(|e| anyhow!("Failed to send CC: {}", e))
    }
}

impl Drop for MidiOutputHandle {
    fn drop(&mut self) {
        // Send shutdown command to cleanly close the MIDI connection
        let _ = self.command_tx.send(MidiCommand::Shutdown);

        // Wait for the thread to properly close the CoreMIDI connection
        // Use take() since we need to move ownership out of the Option
        if let Some(thread) = self.thread.take() {
            // Use a timeout approach: spawn a joiner thread and wait briefly
            // This prevents blocking forever if the MIDI server is stuck
            let joiner = std::thread::spawn(move || {
                let _ = thread.join();
            });
            // Wait up to 200ms for graceful shutdown
            std::thread::sleep(std::time::Duration::from_millis(200));
            // If joiner is still running, it will be dropped (thread detached)
            drop(joiner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_to_midi() {
        // C4 = MIDI 60
        assert_eq!(note_to_midi(0, 4), 60);
        // A4 = MIDI 69
        assert_eq!(note_to_midi(9, 4), 69);
        // C-1 = MIDI 0
        assert_eq!(note_to_midi(0, -1), 0);
        // G9 = MIDI 127 (clamped)
        assert_eq!(note_to_midi(7, 9), 127);
        // C5 = MIDI 72
        assert_eq!(note_to_midi(0, 5), 72);
        // B3 = MIDI 59
        assert_eq!(note_to_midi(11, 3), 59);
    }

    #[test]
    fn test_frequency_to_midi() {
        // A4 = 440Hz = MIDI 69
        assert_eq!(frequency_to_midi(440.0), 69);
        // A5 = 880Hz = MIDI 81
        assert_eq!(frequency_to_midi(880.0), 81);
        // C4 ≈ 261.63Hz = MIDI 60
        assert_eq!(frequency_to_midi(261.63), 60);
        // Edge case: 0 Hz
        assert_eq!(frequency_to_midi(0.0), 0);
    }

    #[test]
    fn test_channel_mode_per_track() {
        let handle = MidiOutputHandle::new().unwrap();
        handle.set_channel_mode(MidiChannelMode::PerTrack);

        assert_eq!(handle.channel_for_track(0), 0);
        assert_eq!(handle.channel_for_track(1), 1);
        assert_eq!(handle.channel_for_track(15), 15);
        assert_eq!(handle.channel_for_track(16), 0); // Wraps
    }

    #[test]
    fn test_channel_mode_mono() {
        let handle = MidiOutputHandle::new().unwrap();
        handle.set_channel_mode(MidiChannelMode::Mono(5));

        assert_eq!(handle.channel_for_track(0), 5);
        assert_eq!(handle.channel_for_track(1), 5);
        assert_eq!(handle.channel_for_track(15), 5);
    }

    #[test]
    fn test_list_ports() {
        // This test just verifies the function doesn't panic
        // Actual ports depend on the system
        let handle = MidiOutputHandle::new().unwrap();
        let result = handle.list_ports();
        assert!(result.is_ok());
    }
}
