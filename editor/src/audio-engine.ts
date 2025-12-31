import { type Action, WasmInterpreter, rationalToFloat, getUserFunctions, preResolveImports } from './cadence-wasm';
import { updateUserFunctions } from './hover';

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
     * Normalize waveform string to valid OscillatorType
     */
    private normalizeWaveform(type: string): WaveformType {
        const validTypes: WaveformType[] = ['sine', 'square', 'sawtooth', 'triangle'];
        const normalized = type.toLowerCase() as WaveformType;
        if (validTypes.includes(normalized)) {
            return normalized;
        } else if (type.toLowerCase() === 'saw') {
            return 'sawtooth';
        }
        return this.waveform; // Fall back to current default
    }

    /**
     * Play a single note at a specific frequency
     * @param freq Frequency in Hz
     * @param startTime When to start (AudioContext time)
     * @param durationSec Duration in seconds
     * @param noteGain Gain for this note (0-1), used to normalize chords
     * @param waveform Waveform type for this note
     * @param adsr ADSR envelope for this note
     * @param pan Optional stereo pan (0.0 = left, 0.5 = center, 1.0 = right)
     */
    private scheduleNote(
        freq: number,
        startTime: number,
        durationSec: number,
        noteGain: number,
        waveform: WaveformType,
        adsr: AdsrParams,
        pan?: number,
    ): void {
        const ctx = this.ensureContext();

        const oscillator = ctx.createOscillator();
        const gainNode = ctx.createGain();

        oscillator.type = waveform;
        oscillator.frequency.setValueAtTime(freq, startTime);

        oscillator.connect(gainNode);

        // Add stereo panning if specified
        if (pan !== undefined && pan !== null) {
            const panner = ctx.createStereoPanner();
            // Convert 0-1 range to -1 to +1 range
            panner.pan.setValueAtTime((pan - 0.5) * 2, startTime);
            gainNode.connect(panner);
            panner.connect(ctx.destination);
        } else {
            gainNode.connect(ctx.destination);
        }

        const { attack, decay, sustain, release } = adsr;

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
     * Schedule a synthesized drum sound
     * Uses simple synthesis techniques for kicks, snares, and hi-hats
     */
    private scheduleDrum(
        drumType: string,
        startTime: number,
        gain: number,
        pan?: number,
    ): void {
        const ctx = this.ensureContext();
        const gainNode = ctx.createGain();

        // Add panning if specified
        if (pan !== undefined && pan !== null) {
            const panner = ctx.createStereoPanner();
            panner.pan.setValueAtTime((pan - 0.5) * 2, startTime);
            gainNode.connect(panner);
            panner.connect(ctx.destination);
        } else {
            gainNode.connect(ctx.destination);
        }

        const drumSound = drumType.toLowerCase();

        if (drumSound === 'kick' || drumSound === 'k' || drumSound === 'bd') {
            // Kick: sine wave with pitch drop
            const osc = ctx.createOscillator();
            osc.type = 'sine';
            osc.frequency.setValueAtTime(150, startTime);
            osc.frequency.exponentialRampToValueAtTime(40, startTime + 0.1);
            osc.connect(gainNode);

            // Smooth envelope to prevent clicks
            gainNode.gain.setValueAtTime(gain * 0.8, startTime);
            gainNode.gain.exponentialRampToValueAtTime(0.01, startTime + 0.25);
            gainNode.gain.linearRampToValueAtTime(0, startTime + 0.35); // Fade to zero

            osc.start(startTime);
            osc.stop(startTime + 0.4);
        } else if (drumSound === 'snare' || drumSound === 's' || drumSound === 'sd') {
            // Snare: noise burst + tone
            const bufferSize = ctx.sampleRate * 0.1;
            const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
            const noise = buffer.getChannelData(0);
            for (let i = 0; i < bufferSize; i++) {
                noise[i] = Math.random() * 2 - 1;
            }

            const noiseSource = ctx.createBufferSource();
            noiseSource.buffer = buffer;

            const noiseGain = ctx.createGain();
            noiseSource.connect(noiseGain);
            noiseGain.connect(gainNode);

            noiseGain.gain.setValueAtTime(gain * 0.5, startTime);
            noiseGain.gain.exponentialRampToValueAtTime(0.01, startTime + 0.15);

            // Add a tone component
            const osc = ctx.createOscillator();
            osc.type = 'triangle';
            osc.frequency.setValueAtTime(180, startTime);
            osc.connect(gainNode);

            gainNode.gain.setValueAtTime(gain * 0.4, startTime);
            gainNode.gain.exponentialRampToValueAtTime(0.01, startTime + 0.1);

            noiseSource.start(startTime);
            noiseSource.stop(startTime + 0.2);
            osc.start(startTime);
            osc.stop(startTime + 0.15);
        } else if (drumSound === 'hh' || drumSound === 'h' || drumSound === 'hihat') {
            // Hi-hat: filtered noise
            const bufferSize = ctx.sampleRate * 0.05;
            const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
            const noise = buffer.getChannelData(0);
            for (let i = 0; i < bufferSize; i++) {
                noise[i] = Math.random() * 2 - 1;
            }

            const noiseSource = ctx.createBufferSource();
            noiseSource.buffer = buffer;

            const filter = ctx.createBiquadFilter();
            filter.type = 'highpass';
            filter.frequency.setValueAtTime(8000, startTime);

            noiseSource.connect(filter);
            filter.connect(gainNode);

            gainNode.gain.setValueAtTime(gain * 0.3, startTime);
            gainNode.gain.exponentialRampToValueAtTime(0.01, startTime + 0.05);

            noiseSource.start(startTime);
            noiseSource.stop(startTime + 0.08);
        } else if (drumSound === 'oh' || drumSound === 'openhat') {
            // Open hi-hat: longer filtered noise
            const bufferSize = ctx.sampleRate * 0.2;
            const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
            const noise = buffer.getChannelData(0);
            for (let i = 0; i < bufferSize; i++) {
                noise[i] = Math.random() * 2 - 1;
            }

            const noiseSource = ctx.createBufferSource();
            noiseSource.buffer = buffer;

            const filter = ctx.createBiquadFilter();
            filter.type = 'highpass';
            filter.frequency.setValueAtTime(7000, startTime);

            noiseSource.connect(filter);
            filter.connect(gainNode);

            gainNode.gain.setValueAtTime(gain * 0.25, startTime);
            gainNode.gain.exponentialRampToValueAtTime(0.01, startTime + 0.2);

            noiseSource.start(startTime);
            noiseSource.stop(startTime + 0.25);
        } else if (drumSound === 'clap' || drumSound === 'cp') {
            // Clap: multiple noise bursts
            for (let i = 0; i < 3; i++) {
                const bufferSize = ctx.sampleRate * 0.02;
                const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
                const noise = buffer.getChannelData(0);
                for (let j = 0; j < bufferSize; j++) {
                    noise[j] = Math.random() * 2 - 1;
                }

                const noiseSource = ctx.createBufferSource();
                noiseSource.buffer = buffer;

                const filter = ctx.createBiquadFilter();
                filter.type = 'bandpass';
                filter.frequency.setValueAtTime(1200, startTime + i * 0.01);

                const burstGain = ctx.createGain();
                noiseSource.connect(filter);
                filter.connect(burstGain);
                burstGain.connect(gainNode);

                burstGain.gain.setValueAtTime(gain * 0.4, startTime + i * 0.01);
                burstGain.gain.exponentialRampToValueAtTime(0.01, startTime + i * 0.01 + 0.05);

                noiseSource.start(startTime + i * 0.01);
                noiseSource.stop(startTime + i * 0.01 + 0.08);
            }
            gainNode.gain.setValueAtTime(1, startTime);
        } else {
            // Default: short noise burst for unknown drums
            const bufferSize = ctx.sampleRate * 0.05;
            const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
            const noise = buffer.getChannelData(0);
            for (let i = 0; i < bufferSize; i++) {
                noise[i] = Math.random() * 2 - 1;
            }

            const noiseSource = ctx.createBufferSource();
            noiseSource.buffer = buffer;
            noiseSource.connect(gainNode);

            gainNode.gain.setValueAtTime(gain * 0.3, startTime);
            gainNode.gain.exponentialRampToValueAtTime(0.01, startTime + 0.05);

            noiseSource.start(startTime);
            noiseSource.stop(startTime + 0.08);
        }
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

        // Update hover cache with user-defined functions from this script
        updateUserFunctions(getUserFunctions(this.interpreter));

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
     * Play script with async import resolution.
     * This pre-resolves all `use` statements before executing the script,
     * bridging the async/sync gap for module loading.
     */
    async playScriptWithImports(code: string): Promise<void> {
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

        // Pre-resolve all imports BEFORE loading the script
        try {
            const resolved = await preResolveImports(this.interpreter, code);
            if (resolved.length > 0) {
                console.log(`🔗 Resolved modules: ${resolved.join(', ')}`);
            }
        } catch (e) {
            console.warn('Module resolution warning:', e);
            // Continue anyway - modules might not exist yet
        }

        // Load code and get initial actions
        const result = this.interpreter.load(code);

        // Update hover cache with user-defined functions from this script
        updateUserFunctions(getUserFunctions(this.interpreter));

        // Process initial actions (like Setup)
        if (result.actions) {
            for (const action of result.actions) {
                this.handleAction(action as Action, ctx.currentTime);
            }
        }

        // Start scheduler loop
        const LOOKAHEAD = 0.1;
        const SCHEDULE_INTERVAL = 25;
        let nextBeatTime = ctx.currentTime + (60 / this.tempo);

        const scheduler = () => {
            if (!this.isPlaying || !this.interpreter) return;

            const now = ctx.currentTime;

            while (nextBeatTime < now + LOOKAHEAD) {
                const result = this.interpreter.tick();

                if (result.actions) {
                    for (const action of result.actions) {
                        this.handleAction(action as Action, nextBeatTime);
                    }
                }

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
                // Determine waveform for THIS action (action-specific or current default)
                const actionWaveform = action.waveform
                    ? this.normalizeWaveform(action.waveform)
                    : this.waveform;

                // Determine ADSR for THIS action (action-specific or current default)
                const actionAdsr: AdsrParams = action.envelope
                    ? {
                        attack: action.envelope[0],
                        decay: action.envelope[1],
                        sustain: action.envelope[2],
                        release: action.envelope[3],
                    }
                    : this.adsr;

                // Get volume for this track
                const trackVolume = this.getTrackVolume(action.track_id);

                // Get pan for this action (convert undefined to null for type safety)
                const actionPan = action.pan ?? undefined;

                // Schedule events relative to beat start time
                // Each tick delivers events for one beat, but we need sub-beat precision
                // for weighted steps. Use the fractional part of start_beat for offset.
                for (const event of action.events) {
                    // Get the fractional beat offset (within the current beat)
                    const startBeatFloat = rationalToFloat(event.start_beat);
                    const beatOffset = startBeatFloat % 1.0; // Fractional part
                    const eventTime = startTime + this.beatsToSeconds(beatOffset);

                    // Convert rational duration to seconds
                    const durationBeats = rationalToFloat(event.duration);
                    const durationSec = this.beatsToSeconds(durationBeats);

                    // Play melodic notes
                    if (!event.is_rest && event.frequencies.length > 0) {
                        // Normalize gain by number of notes to prevent saturation
                        const noteCount = event.frequencies.length;
                        const normalizedGain = trackVolume / Math.sqrt(noteCount);

                        for (const freq of event.frequencies) {
                            this.scheduleNote(freq, eventTime, durationSec, normalizedGain, actionWaveform, actionAdsr, actionPan);
                        }
                    }

                    // Play drum sounds
                    if (event.drums && event.drums.length > 0) {
                        for (const drumType of event.drums) {
                            this.scheduleDrum(drumType, eventTime, trackVolume, actionPan);
                        }
                    }
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
