import { type Action, WasmInterpreter } from './cadence-wasm';

/** ADSR envelope parameters */
export interface AdsrParams {
    attack: number;   // seconds
    decay: number;    // seconds
    sustain: number;  // level (0-1)
    release: number;  // seconds
}

/** Waveform type for oscillators */
export type WaveformType = 'sine' | 'square' | 'sawtooth' | 'triangle';

/** Active oscillator for cleanup */
interface ActiveOscillator {
    oscillator: OscillatorNode;
    gain: GainNode;
}

/**
 * Audio engine for Cadence pattern playback
 */
export class CadenceAudioEngine {
    private audioContext: AudioContext | null = null;
    private tempo: number = 120;
    private volume: number = 0.3;  // Default to safer listening level
    private waveform: WaveformType = 'sine';
    private isPlaying: boolean = false;
    private activeOscillators: ActiveOscillator[] = [];
    private scheduledTimeouts: number[] = [];
    private interpreter: WasmInterpreter | null = null;
    private currentBeat: number = 0;       // Current beat counter
    private lastBeatTime: number = 0;      // AudioContext time of last beat (for smooth interpolation)

    // Per-track volume (0-1 scale)
    private trackVolumes: Map<number, number> = new Map();

    // Default ADSR (more pronounced for audibility)
    private adsr: AdsrParams = {
        attack: 0.01,    // 10ms attack
        decay: 0.15,     // 150ms decay
        sustain: 0.6,    // 60% sustain level
        release: 0.25,   // 250ms release
    };

    /**
     * Ensure AudioContext is created (requires user gesture)
     */
    private ensureContext(): AudioContext {
        if (!this.audioContext) {
            this.audioContext = new AudioContext();
        }
        if (this.audioContext.state === 'suspended') {
            this.audioContext.resume();
        }
        return this.audioContext;
    }

    /**
     * Set the tempo in BPM
     */
    setTempo(bpm: number): void {
        this.tempo = Math.max(20, Math.min(300, bpm));
        console.log(`🎵 Tempo: ${this.tempo} BPM`);
    }

    /**
     * Set the master volume (0-1)
     */
    setVolume(vol: number): void {
        this.volume = Math.max(0, Math.min(1, vol));
    }

    /**
     * Set volume for a specific track (0.0-1.0 scale, already normalized by Rust)
     */
    setTrackVolume(trackId: number, vol: number): void {
        // Rust already normalizes to 0.0-1.0, just clamp for safety
        const clamped = Math.max(0, Math.min(1, vol));
        this.trackVolumes.set(trackId, clamped);
    }

    /**
     * Get volume for a track (defaults to master volume)
     */
    getTrackVolume(trackId: number): number {
        return this.trackVolumes.get(trackId) ?? this.volume;
    }

    /**
     * Set the waveform type
     */
    setWaveform(type: string): void {
        const validTypes: WaveformType[] = ['sine', 'square', 'sawtooth', 'triangle'];
        const normalized = type.toLowerCase() as WaveformType;
        if (validTypes.includes(normalized)) {
            this.waveform = normalized;
            console.log(`🔊 Waveform: ${this.waveform}`);
        } else if (type.toLowerCase() === 'saw') {
            this.waveform = 'sawtooth';
        }
    }

    /**
     * Convert beats to seconds based on current tempo
     */
    private beatsToSeconds(beats: number): number {
        return (60 / this.tempo) * beats;
    }

    /**
     * Play a single note at a specific frequency
     * @param freq Frequency in Hz
     * @param startTime When to start (AudioContext time)
     * @param durationSec Duration in seconds
     * @param noteGain Gain for this note (0-1), used to normalize chords
     */
    private scheduleNote(
        freq: number,
        startTime: number,
        durationSec: number,
        noteGain: number = this.volume,
    ): void {
        const ctx = this.ensureContext();

        const oscillator = ctx.createOscillator();
        const gainNode = ctx.createGain();

        oscillator.type = this.waveform;
        oscillator.frequency.setValueAtTime(freq, startTime);

        oscillator.connect(gainNode);
        gainNode.connect(ctx.destination);

        const { attack, decay, sustain, release } = this.adsr;

        // Ensure minimum duration for envelope to complete attack+decay
        const minEnvelopeDuration = attack + decay + 0.05;
        const effectiveDuration = Math.max(durationSec, minEnvelopeDuration);

        const peakTime = startTime + attack;
        const sustainTime = peakTime + decay;
        const releaseTime = startTime + effectiveDuration;

        // ADSR envelope with normalized gain
        gainNode.gain.setValueAtTime(0, startTime);
        gainNode.gain.linearRampToValueAtTime(noteGain, peakTime);
        gainNode.gain.linearRampToValueAtTime(noteGain * sustain, sustainTime);
        gainNode.gain.setValueAtTime(noteGain * sustain, releaseTime);
        gainNode.gain.linearRampToValueAtTime(0, releaseTime + release);

        oscillator.start(startTime);
        oscillator.stop(releaseTime + release + 0.01);

        // Track for cleanup
        const active = { oscillator, gain: gainNode };
        this.activeOscillators.push(active);

        oscillator.onended = () => {
            const idx = this.activeOscillators.indexOf(active);
            if (idx !== -1) {
                this.activeOscillators.splice(idx, 1);
            }
        };
    }

    /**
     * Play script reactively using WasmInterpreter
     */
    playScript(code: string): void {
        // Stop previous playback
        this.stop();

        const ctx = this.ensureContext();
        this.isPlaying = true;
        this.currentBeat = 0;
        this.lastBeatTime = ctx.currentTime;

        // Initialize interpreter
        if (!this.interpreter) {
            this.interpreter = new WasmInterpreter();
        }

        // Load code and get initial actions - this also pre-populates env including _cycle=0
        const result = this.interpreter.load(code);

        // Process initial actions (like Setup)
        if (result.actions) {
            for (const action of result.actions) {
                // If it's a Play action from load(), it's the beat 0 event
                // We should schedule it immediately
                this.handleAction(action as Action, ctx.currentTime);
            }
        }

        // Start scheduler loop starting from next beat
        // Current cycle 0 actions were already handled above.
        // We need to advance to next beat.

        const LOOKAHEAD = 0.1; // 100ms
        const SCHEDULE_INTERVAL = 25; // 25ms
        let nextBeatTime = ctx.currentTime + (60 / this.tempo); // Start next beat after 1 beat duration

        const scheduler = () => {
            if (!this.isPlaying || !this.interpreter) return;

            const now = ctx.currentTime;

            // Schedule beats that fall within lookahead window
            while (nextBeatTime < now + LOOKAHEAD) {
                // Tick interpreter for this beat (advances cycle)
                const result = this.interpreter.tick();

                if (result.actions) {
                    for (const action of result.actions) {
                        this.handleAction(action as Action, nextBeatTime);
                    }
                }

                // Advance by 1 beat
                this.currentBeat++;
                this.lastBeatTime = nextBeatTime;
                const beatDuration = 60 / this.tempo;
                nextBeatTime += beatDuration;
            }

            const timeoutId = window.setTimeout(scheduler, SCHEDULE_INTERVAL);
            this.scheduledTimeouts.push(timeoutId);
        };

        const timeoutId = window.setTimeout(scheduler, SCHEDULE_INTERVAL);
        this.scheduledTimeouts.push(timeoutId);
    }

    /**
     * Update running script without resetting cycle (for live coding)
     */
    updateScript(code: string): void {
        if (!this.isPlaying || !this.interpreter) return;

        // Call update on interpreter (preserves cycle count)
        const result = this.interpreter.update(code);

        // Handle any immediate actions from update (e.g. tempo change)
        if (result.actions) {
            const ctx = this.ensureContext();
            for (const action of result.actions) {
                // Only handle basic state changes immediately, ignore Play actions 
                // as they will be picked up by the next tick()
                if (action.type !== 'Play') {
                    this.handleAction(action as Action, ctx.currentTime);
                }
            }
        }
    }

    /**
     * Handle an action from the interpreter
     */
    handleAction(action: Action, startTime: number): void {
        switch (action.type) {
            case 'Play': {
                // Apply custom waveform/envelope
                if (action.waveform) this.setWaveform(action.waveform);
                if (action.envelope) {
                    this.adsr = {
                        attack: action.envelope[0],
                        decay: action.envelope[1],
                        sustain: action.envelope[2],
                        release: action.envelope[3],
                    };
                }

                // Get volume for this track
                const trackVolume = this.getTrackVolume(action.track_id);

                // Schedule events relative to beat start time
                let time = startTime;
                for (const event of action.events) {
                    const durationSec = this.beatsToSeconds(event.duration);
                    if (!event.is_rest && event.frequencies.length > 0) {
                        // Normalize gain by number of notes to prevent saturation
                        const noteCount = event.frequencies.length;
                        const normalizedGain = trackVolume / Math.sqrt(noteCount);

                        for (const freq of event.frequencies) {
                            this.scheduleNote(freq, time, durationSec, normalizedGain);
                        }
                    }
                    time += durationSec;
                }
                break;
            }
            case 'SetTempo':
                this.setTempo(action.bpm);
                break;
            case 'SetVolume':
                // Use per-track volume with 0-100 to 0-1 scaling
                this.setTrackVolume(action.track_id, action.volume);
                break;
            case 'SetWaveform':
                this.setWaveform(action.waveform);
                break;
            case 'Stop':
                this.stop();
                break;
        }
    }

    /**
     * Stop all playback
     */
    stop(): void {
        this.isPlaying = false;

        // Clear scheduled loops
        for (const id of this.scheduledTimeouts) {
            clearTimeout(id);
        }
        this.scheduledTimeouts = [];

        // Stop all active oscillators
        const ctx = this.audioContext;
        if (ctx) {
            const now = ctx.currentTime;
            for (const active of this.activeOscillators) {
                try {
                    active.gain.gain.cancelScheduledValues(now);
                    active.gain.gain.setValueAtTime(active.gain.gain.value, now);
                    active.gain.gain.linearRampToValueAtTime(0, now + 0.02);
                    active.oscillator.stop(now + 0.03);
                } catch {
                    // Oscillator may already be stopped
                }
            }
        }
        this.activeOscillators = [];
        console.log('⏹ Stopped');
    }

    /**
     * Get current playback state
     */
    get playing(): boolean {
        return this.isPlaying;
    }

    /**
     * Get current tempo
     */
    get currentTempo(): number {
        return this.tempo;
    }

    /**
     * Get current playback position in beats (with smooth interpolation)
     */
    getPlaybackPosition(): { beat: number; isPlaying: boolean } {
        if (!this.isPlaying || !this.audioContext) {
            return { beat: 0, isPlaying: false };
        }
        // Interpolate between last beat and next beat for smooth animation
        const beatDuration = 60 / this.tempo;
        const elapsed = this.audioContext.currentTime - this.lastBeatTime;
        const fraction = Math.min(elapsed / beatDuration, 1);  // Clamp to 1
        return { beat: this.currentBeat + fraction, isPlaying: true };
    }
}

// Singleton instance for the app
export const audioEngine = new CadenceAudioEngine();
