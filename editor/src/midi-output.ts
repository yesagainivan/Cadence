/**
 * WebMIDI Output Service
 * 
 * Manages MIDI output connections and sends note/clock/transport messages
 * to external MIDI devices and DAWs.
 */

// MIDI message constants
const NOTE_ON = 0x90;
const NOTE_OFF = 0x80;
const CLOCK = 0xF8;
const START = 0xFA;
const CONTINUE = 0xFB;
const STOP = 0xFC;

// Pulses per quarter note (MIDI standard)
const PPQN = 24;

interface ScheduledNote {
    timeoutId: number;
    noteOffId: number;
}

/**
 * Singleton service for WebMIDI output
 */
class MidiOutput {
    private midiAccess: MIDIAccess | null = null;
    private selectedOutput: MIDIOutput | null = null;
    private clockIntervalId: number | null = null;
    private scheduledNotes: ScheduledNote[] = [];
    private _enabled: boolean = false;
    private currentChannel: number = 0; // MIDI channel 1 (0-indexed)

    /**
     * Request MIDI access from the browser
     * @returns true if MIDI access was granted
     */
    async init(): Promise<boolean> {
        if (this.midiAccess) {
            return true;
        }

        if (!navigator.requestMIDIAccess) {
            console.warn('🎹 WebMIDI not supported in this browser');
            return false;
        }

        try {
            this.midiAccess = await navigator.requestMIDIAccess({ sysex: false });
            console.log('🎹 WebMIDI access granted');

            // Listen for device changes
            this.midiAccess.onstatechange = (e) => {
                console.log('🎹 MIDI device change:', e.port?.name, e.port?.state);
            };

            return true;
        } catch (e) {
            console.error('🎹 WebMIDI access denied:', e);
            return false;
        }
    }

    /**
     * Get list of available MIDI output ports
     */
    getOutputs(): MIDIOutput[] {
        if (!this.midiAccess) {
            return [];
        }

        const outputs: MIDIOutput[] = [];
        this.midiAccess.outputs.forEach((output) => {
            outputs.push(output);
        });
        return outputs;
    }

    /**
     * Select a MIDI output port by ID
     */
    selectOutput(id: string): void {
        if (!this.midiAccess) {
            console.warn('🎹 MIDI not initialized');
            return;
        }

        if (!id) {
            // Deselect
            this.selectedOutput = null;
            this._enabled = false;
            console.log('🎹 MIDI output disabled');
            return;
        }

        const output = this.midiAccess.outputs.get(id);
        if (output) {
            this.selectedOutput = output;
            this._enabled = true;
            console.log(`🎹 MIDI output: ${output.name}`);
        } else {
            console.warn(`🎹 MIDI output not found: ${id}`);
        }
    }

    /**
     * Check if MIDI output is enabled
     */
    isEnabled(): boolean {
        return this._enabled && this.selectedOutput !== null;
    }

    /**
     * Set the MIDI channel (1-16, will be converted to 0-15 internally)
     */
    setChannel(channel: number): void {
        this.currentChannel = Math.max(0, Math.min(15, channel - 1));
    }

    // =========================================================================
    // Note Messages
    // =========================================================================

    /**
     * Send Note On message immediately
     */
    noteOn(note: number, velocity: number = 100, channel?: number): void {
        if (!this.selectedOutput) return;

        const ch = channel ?? this.currentChannel;
        const msg = [NOTE_ON | ch, note, velocity];
        this.selectedOutput.send(msg);
    }

    /**
     * Send Note Off message immediately
     */
    noteOff(note: number, channel?: number): void {
        if (!this.selectedOutput) return;

        const ch = channel ?? this.currentChannel;
        const msg = [NOTE_OFF | ch, note, 0];
        this.selectedOutput.send(msg);
    }

    /**
     * Schedule a note to play at a future time
     * @param note MIDI note number (0-127)
     * @param delayMs Milliseconds from now to start the note
     * @param durationMs Duration in milliseconds
     * @param velocity Note velocity (0-127)
     */
    scheduleNote(note: number, delayMs: number, durationMs: number, velocity: number = 100): void {
        if (!this.selectedOutput) return;

        // Clamp to valid MIDI range
        const midiNote = Math.max(0, Math.min(127, Math.round(note)));
        const midiVelocity = Math.max(1, Math.min(127, Math.round(velocity)));

        // Schedule Note On
        const noteOnId = window.setTimeout(() => {
            this.noteOn(midiNote, midiVelocity);
        }, Math.max(0, delayMs));

        // Schedule Note Off
        const noteOffId = window.setTimeout(() => {
            this.noteOff(midiNote);
        }, Math.max(0, delayMs + durationMs));

        this.scheduledNotes.push({ timeoutId: noteOnId, noteOffId });
    }

    /**
     * Cancel all scheduled notes and send All Notes Off
     */
    allNotesOff(): void {
        // Clear scheduled notes
        for (const note of this.scheduledNotes) {
            clearTimeout(note.timeoutId);
            clearTimeout(note.noteOffId);
        }
        this.scheduledNotes = [];

        // Send All Notes Off on current channel (CC 123)
        if (this.selectedOutput) {
            const ch = this.currentChannel;
            this.selectedOutput.send([0xB0 | ch, 123, 0]);
        }
    }

    // =========================================================================
    // Transport Messages (System Real-Time)
    // =========================================================================

    /**
     * Send MIDI Start message (0xFA)
     * Tells external devices to start playback from the beginning
     */
    sendStart(): void {
        if (!this.selectedOutput) return;
        this.selectedOutput.send([START]);
        console.log('🎹 MIDI Start');
    }

    /**
     * Send MIDI Stop message (0xFC)
     * Tells external devices to stop playback
     */
    sendStop(): void {
        if (!this.selectedOutput) return;
        this.selectedOutput.send([STOP]);
        this.allNotesOff();
        console.log('🎹 MIDI Stop');
    }

    /**
     * Send MIDI Continue message (0xFB)
     * Tells external devices to resume from current position
     */
    sendContinue(): void {
        if (!this.selectedOutput) return;
        this.selectedOutput.send([CONTINUE]);
        console.log('🎹 MIDI Continue');
    }

    // =========================================================================
    // MIDI Clock
    // =========================================================================

    /**
     * Start sending MIDI clock pulses at the specified tempo
     * @param bpm Beats per minute
     */
    startClock(bpm: number): void {
        this.stopClock(); // Stop any existing clock

        if (!this.selectedOutput) return;

        // Calculate interval: PPQN pulses per quarter note
        // At 120 BPM: 60/120 = 0.5 seconds per beat = 500ms
        // 500ms / 24 = ~20.83ms per pulse
        const msPerBeat = 60000 / bpm;
        const msPerPulse = msPerBeat / PPQN;

        this.clockIntervalId = window.setInterval(() => {
            if (this.selectedOutput) {
                this.selectedOutput.send([CLOCK]);
            }
        }, msPerPulse);

        console.log(`🎹 MIDI Clock started at ${bpm} BPM (${msPerPulse.toFixed(2)}ms per pulse)`);
    }

    /**
     * Stop sending MIDI clock pulses
     */
    stopClock(): void {
        if (this.clockIntervalId !== null) {
            clearInterval(this.clockIntervalId);
            this.clockIntervalId = null;
            console.log('🎹 MIDI Clock stopped');
        }
    }

    /**
     * Update clock tempo (restarts clock if running)
     */
    setClockTempo(bpm: number): void {
        if (this.clockIntervalId !== null) {
            this.startClock(bpm);
        }
    }

    /**
     * Full cleanup - stop clock, clear notes, close output
     */
    dispose(): void {
        this.stopClock();
        this.allNotesOff();
        this.selectedOutput = null;
        this._enabled = false;
    }
}

// Singleton export
export const midiOutput = new MidiOutput();
