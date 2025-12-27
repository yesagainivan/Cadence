/**
 * WASM Loader for Cadence Core
 * 
 * Loads and initializes the cadence-core WASM module,
 * providing access to tokenization and parsing functions.
 */

import init, { tokenize, parse_and_check, run_script, get_events_at_position, WasmInterpreter } from './wasm/cadence_core.js';

export interface HighlightSpan {
    start_line: number;
    start_col: number;
    end_line: number;
    end_col: number;
    token_type: string;
    text: string;
}

export interface ParseResult {
    success: boolean;
    error: string | null;
}

// ============================================================================
// Script Execution Types
// ============================================================================

/** Rich information about a single note */
export interface NoteInfo {
    /** MIDI note number (0-127) */
    midi: number;
    /** Frequency in Hz */
    frequency: number;
    /** Display name with octave (e.g., "C#4", "Bb3") */
    name: string;
    /** Pitch class (0-11): C=0, C#=1, D=2, etc. */
    pitch_class: number;
    /** Octave in scientific pitch notation */
    octave: number;
}

/** A single playback event with rich note data */
export interface PlayEvent {
    /** Rich note information (MIDI, frequency, name, etc.) */
    notes: NoteInfo[];
    /** Frequencies to play (for backward compatibility) */
    frequencies: number[];
    /** Start time in beats relative to pattern start */
    start_beat: number;
    /** Duration in beats */
    duration: number;
    /** Whether this is a rest (silence) */
    is_rest: boolean;
}

/** Pattern events with cycle timing info (returned by get_events_at_position) */
export interface PatternEvents {
    /** Individual playback events */
    events: PlayEvent[];
    /** Total beats in one pattern cycle (affected by fast/slow) */
    beats_per_cycle: number;
}

/** Play action with pattern events */
export interface PlayAction {
    type: 'Play';
    events: PlayEvent[];
    looping: boolean;
    track_id: number;
    /** Custom ADSR envelope: [attack, decay, sustain, release] */
    envelope: [number, number, number, number] | null;
    /** Custom waveform name */
    waveform: string | null;
}

/** Set tempo action */
export interface SetTempoAction {
    type: 'SetTempo';
    bpm: number;
}

/** Set volume action */
export interface SetVolumeAction {
    type: 'SetVolume';
    volume: number;
    track_id: number;
}

/** Set waveform action */
export interface SetWaveformAction {
    type: 'SetWaveform';
    waveform: string;
    track_id: number;
}

/** Stop action */
export interface StopAction {
    type: 'Stop';
    track_id: number | null;
}

/** All possible actions from script execution */
export type Action = PlayAction | SetTempoAction | SetVolumeAction | SetWaveformAction | StopAction;

/** Result of running a script */
export interface ScriptResult {
    success: boolean;
    actions: Action[];
    error: string | null;
    output: string[];
}

let wasmInitialized = false;
let initPromise: Promise<void> | null = null;

/**
 * Initialize the WASM module (call once at app startup)
 */
export async function initWasm(): Promise<void> {
    if (wasmInitialized) return;

    if (initPromise) {
        return initPromise;
    }

    initPromise = init().then(() => {
        wasmInitialized = true;
        console.log('✓ WASM module initialized');
    });

    return initPromise;
}

/**
 * Check if WASM is ready
 */
export function isWasmReady(): boolean {
    return wasmInitialized;
}

/**
 * Tokenize Cadence code and get highlight spans
 * Returns an array of HighlightSpan objects for syntax highlighting
 */
export function tokenizeCode(input: string): HighlightSpan[] {
    if (!wasmInitialized) {
        console.warn('WASM not initialized, call initWasm() first');
        return [];
    }

    try {
        const result = tokenize(input);
        return result as HighlightSpan[];
    } catch (e) {
        console.error('Tokenization error:', e);
        return [];
    }
}

/**
 * Parse and validate Cadence code
 * Returns success/failure and any error message
 */
export function parseCode(input: string): ParseResult {
    if (!wasmInitialized) {
        console.warn('WASM not initialized, call initWasm() first');
        return { success: false, error: 'WASM not initialized' };
    }

    try {
        const result = parse_and_check(input);
        return result as ParseResult;
    } catch (e) {
        console.error('Parse error:', e);
        return { success: false, error: String(e) };
    }
}

/**
 * Run a Cadence script and get actions for playback
 */
export function runScript(input: string): ScriptResult {
    if (!wasmInitialized) {
        console.warn('WASM not initialized, call initWasm() first');
        return { success: false, actions: [], error: 'WASM not initialized', output: [] };
    }

    try {
        const result = run_script(input);
        return result as ScriptResult;
    } catch (e) {
        console.error('Script execution error:', e);
        return { success: false, actions: [], error: String(e), output: [] };
    }
}

/**
 * Get play events for the statement at the given cursor position
 * Returns PatternEvents with events and cycle timing for piano roll rendering
 */
export function getEventsAtPosition(code: string, position: number): PatternEvents | null {
    if (!wasmInitialized) {
        return null;
    }

    try {
        const result = get_events_at_position(code, position);
        return result as PatternEvents | null;
    } catch (e) {
        console.error('Get events at position error:', e);
        return null;
    }
}

// Re-export for convenience
export { tokenize, parse_and_check, run_script, get_events_at_position, WasmInterpreter };
