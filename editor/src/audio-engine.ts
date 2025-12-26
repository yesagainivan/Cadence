/**
 * Cadence Audio Engine
 * 
 * Web Audio API-based audio engine for playing Cadence patterns.
 * Uses OscillatorNodes with GainNode envelopes for synthesis.
 */

import type { PlayEvent, Action } from './cadence-wasm';

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
     * Play a list of events (from pattern)
     */
    playEvents(
        events: PlayEvent[],
        looping: boolean = false,
        customEnvelope?: [number, number, number, number] | null,
        customWaveform?: string | null,
    ): void {
        // Stop any previous playback first
        this.stop();

        // Apply custom waveform if provided
        if (customWaveform) {
            this.setWaveform(customWaveform);
        }

        // Apply custom envelope if provided  
        if (customEnvelope) {
            this.adsr = {
                attack: customEnvelope[0],
                decay: customEnvelope[1],
                sustain: customEnvelope[2],
                release: customEnvelope[3],
            };
            console.log(`🎹 Envelope: A=${customEnvelope[0]} D=${customEnvelope[1]} S=${customEnvelope[2]} R=${customEnvelope[3]}`);
        }

        const ctx = this.ensureContext();
        this.isPlaying = true;

        // Calculate cycle duration in seconds
        let cycleDuration = 0;
        for (const event of events) {
            cycleDuration += this.beatsToSeconds(event.duration);
        }

        // Track the absolute time where the next cycle should start
        let nextCycleStart = ctx.currentTime + 0.05; // Small initial buffer

        /**
         * Schedule one complete cycle of events starting at the given time
         */
        const scheduleCycle = (startTime: number): void => {
            let time = startTime;

            for (const event of events) {
                const durationSec = this.beatsToSeconds(event.duration);

                if (!event.is_rest && event.frequencies.length > 0) {
                    for (const freq of event.frequencies) {
                        this.scheduleNote(freq, time, durationSec);
                    }
                }

                time += durationSec;
            }
        };

        // Schedule the first cycle
        scheduleCycle(nextCycleStart);

        if (looping) {
            // Lookahead scheduler: checks every 100ms and schedules notes ahead
            const LOOKAHEAD = 0.2; // Schedule 200ms ahead
            const SCHEDULE_INTERVAL = 100; // Check every 100ms

            const scheduler = (): void => {
                if (!this.isPlaying) return;

                const ctx = this.ensureContext();
                const now = ctx.currentTime;

                // Schedule cycles that fall within our lookahead window
                while (nextCycleStart < now + LOOKAHEAD) {
                    nextCycleStart += cycleDuration;
                    scheduleCycle(nextCycleStart);
                }

                const timeoutId = window.setTimeout(scheduler, SCHEDULE_INTERVAL);
                this.scheduledTimeouts.push(timeoutId);
            };

            // Start the scheduler after a short delay
            const timeoutId = window.setTimeout(scheduler, SCHEDULE_INTERVAL);
            this.scheduledTimeouts.push(timeoutId);
        }
    }

    /**
     * Handle an action from the interpreter
     */
    handleAction(action: Action): void {
        switch (action.type) {
            case 'Play':
                this.playEvents(action.events, action.looping, action.envelope, action.waveform);
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
