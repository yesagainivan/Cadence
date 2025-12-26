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
    private volume: number = 0.5;
    private waveform: WaveformType = 'sine';
    private isPlaying: boolean = false;
    private activeOscillators: ActiveOscillator[] = [];
    private scheduledTimeouts: number[] = [];
    private interpreter: WasmInterpreter | null = null;

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
     */
    private scheduleNote(
        freq: number,
        startTime: number,
        durationSec: number,
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

        // ADSR envelope
        gainNode.gain.setValueAtTime(0, startTime);
        gainNode.gain.linearRampToValueAtTime(this.volume, peakTime);
        gainNode.gain.linearRampToValueAtTime(this.volume * sustain, sustainTime);
        gainNode.gain.setValueAtTime(this.volume * sustain, releaseTime);
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
            case 'Play':
                // Apply custom waveform/envelope
                if (action.waveform) this.setWaveform(action.waveform);
                if (action.envelope) {
                    this.adsr = {
                        attack: action.envelope[0],
                        decay: action.envelope[1],
                        sustain: action.envelope[2],
                        release: action.envelope[3],
                    };
                    // console.log(`🎹 Envelope: ${this.adsr.attack}/${this.adsr.decay}/${this.adsr.sustain}/${this.adsr.release}`);
                }

                // Schedule events relative to beat start time
                let time = startTime;
                for (const event of action.events) {
                    const durationSec = this.beatsToSeconds(event.duration);
                    if (!event.is_rest && event.frequencies.length > 0) {
                        for (const freq of event.frequencies) {
                            this.scheduleNote(freq, time, durationSec);
                        }
                    }
                    time += durationSec;
                }
                break;
            case 'SetTempo':
                this.setTempo(action.bpm);
                break;
            case 'SetVolume':
                this.setVolume(action.volume);
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
}

// Singleton instance for the app
export const audioEngine = new CadenceAudioEngine();
