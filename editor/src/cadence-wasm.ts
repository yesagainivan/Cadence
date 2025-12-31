/**
 * WASM Loader for Cadence Core
 * 
 * Loads and initializes the cadence-core WASM module,
 * providing access to tokenization and parsing functions.
 */

import init, { tokenize, parse_and_check, run_script, get_events_at_position, get_context_at_cursor, get_documentation, get_symbols, get_symbol_at_position, get_definition_by_name, get_use_statements, WasmInterpreter } from './wasm/cadence_core.js';
import { getFileSystemService, FileSystemService } from './filesystem-service.js';

export interface HighlightSpan {
    start_line: number;
    start_col: number;
    end_line: number;
    end_col: number;
    token_type: string;
    text: string;
    /** UTF-16 code unit offset from start of source (for JavaScript string operations) */
    utf16_start: number;
    /** UTF-16 code unit length of token */
    utf16_len: number;
}

export interface ParseError {
    message: string;
    line: number;
    column: number;
    start: number;
    end: number;
}

export interface ParseResult {
    success: boolean;
    error: ParseError | null;
    errors: ParseError[];
}

/** Documentation for built-in functions */
export interface DocItem {
    name: string;
    category: string;
    description: string;
    signature: string;
}

// ============================================================================
// Script Execution Types
// ============================================================================

/** Rational number for precise timing (numerator / denominator) */
export interface RationalJS {
    /** Numerator */
    n: number;
    /** Denominator */
    d: number;
}

/** Convert RationalJS to float */
export function rationalToFloat(r: RationalJS): number {
    return r.n / r.d;
}

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
    /** Drum sounds to play (for percussion) */
    drums: string[];
    /** Start time in beats relative to pattern start (rational) */
    start_beat: RationalJS;
    /** Duration in beats (rational) */
    duration: RationalJS;
    /** Whether this is a rest (silence) */
    is_rest: boolean;
}

/** Pattern events with cycle timing info (returned by get_events_at_position) */
export interface PatternEvents {
    /** Individual playback events */
    events: PlayEvent[];
    /** Total beats in one pattern cycle (rational, affected by fast/slow) */
    beats_per_cycle: RationalJS;
    /** Optional error message if evaluation failed */
    error: string | null;
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
    /** Stereo pan position (0.0 = left, 0.5 = center, 1.0 = right) */
    pan: number | null;
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

// ============================================================================
// Cursor Context Types (for Properties Panel)
// ============================================================================

/** Editable properties for a cursor context */
export interface EditableProperties {
    /** Current waveform (if pattern/play) */
    waveform: string | null;
    /** Current ADSR envelope: [attack, decay, sustain, release] */
    envelope: [number, number, number, number] | null;
    /** Current tempo (if tempo statement) */
    tempo: number | null;
    /** Current volume (if volume statement) */
    volume: number | null;
    /** Beats per cycle (if pattern) */
    beats_per_cycle: number | null;
}

/** Source span information */
export interface SpanInfo {
    start: number;
    end: number;
    /** UTF-16 code unit offset for JavaScript string operations */
    utf16_start: number;
    /** UTF-16 code unit end position */
    utf16_end: number;
}

/** Cursor context for the Properties Panel */
export interface CursorContext {
    /** Type of statement at cursor */
    statement_type: string;
    /** The evaluated value type (if applicable) */
    value_type: string | null;
    /** Editable properties for this context */
    properties: EditableProperties | null;
    /** Source span for replacement */
    span: SpanInfo;
    /** Variable name if this is a let/assign statement */
    variable_name: string | null;
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
        return {
            success: false,
            error: { message: 'WASM not initialized', line: 0, column: 0, start: 0, end: 0 },
            errors: [] // Fallback
        };
    }

    try {
        const result = parse_and_check(input);
        return result as ParseResult;
    } catch (e) {
        console.error('Parse error:', e);
        return {
            success: false,
            error: { message: String(e), line: 0, column: 0, start: 0, end: 0 },
            errors: [] // Fallback
        };
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

/**
 * Get cursor context for the statement at the given position
 * Returns CursorContext with statement metadata and editable properties
 */
export function getContextAtCursor(code: string, position: number): CursorContext | null {
    if (!wasmInitialized) {
        return null;
    }

    try {
        const result = get_context_at_cursor(code, position);
        return result as CursorContext | null;
    } catch (e) {
        console.error('Get context at cursor error:', e);
        return null;
    }
}

// Re-export for convenience
/**
 * Get documentation for all built-in functions
 */
export function getDocumentation(): DocItem[] {
    if (!wasmInitialized) {
        return [];
    }

    try {
        const result = get_documentation();
        return result as DocItem[];
    } catch (e) {
        console.error('Get documentation error:', e);
        return [];
    }
}

// ============================================================================
// Symbol API (for Language Service features)
// ============================================================================

/** A function symbol from the source code */
export interface FunctionSymbol {
    kind: 'Function';
    name: string;
    params: string[];
    signature: string;
    start: number;  // UTF-16 position
    end: number;    // UTF-16 position
    doc_comment: string | null;
    return_type: string | null;
}

/** A variable symbol from the source code */
export interface VariableSymbol {
    kind: 'Variable';
    name: string;
    value_type: string | null;
    start: number;  // UTF-16 position
    end: number;    // UTF-16 position
    doc_comment: string | null;
}

export type Symbol = FunctionSymbol | VariableSymbol;

export interface SymbolsResult {
    success: boolean;
    symbols: Symbol[];
    error: string | null;
}

/**
 * Get all symbols from source code (parses fresh each time)
 */
export function getSymbols(code: string): SymbolsResult {
    if (!wasmInitialized) {
        return { success: false, symbols: [], error: 'WASM not initialized' };
    }

    try {
        return get_symbols(code) as SymbolsResult;
    } catch (e) {
        console.error('Get symbols error:', e);
        return { success: false, symbols: [], error: String(e) };
    }
}

/**
 * Get the symbol at a specific cursor position (for hover)
 */
export function getSymbolAtPosition(code: string, position: number): Symbol | null {
    if (!wasmInitialized) {
        return null;
    }

    try {
        const result = get_symbol_at_position(code, position);
        return result as Symbol | null;
    } catch (e) {
        console.error('Get symbol at position error:', e);
        return null;
    }
}

/**
 * Get user-defined functions from an interpreter instance
 * @deprecated Use getSymbols() instead for reactive updates
 */
export function getUserFunctions(interpreter: WasmInterpreter): DocItem[] {
    try {
        const result = interpreter.get_user_functions();
        return result as DocItem[];
    } catch (e) {
        console.error('Get user functions error:', e);
        return [];
    }
}

// Re-export for convenience
export { tokenize, parse_and_check, run_script, get_events_at_position, get_context_at_cursor, get_documentation, get_symbols, get_symbol_at_position, get_definition_by_name, WasmInterpreter };

// ============================================================================
// Module Resolution (for use statements)
// ============================================================================

/** Result of resolving a module import */
export interface ModuleResolutionResult {
    success: boolean;
    exports?: string[];
    count?: number;
    error?: string;
}

/**
 * Create a WasmInterpreter with a file provider connected to the virtual filesystem.
 * This enables `use "file.cadence"` to work in the browser.
 */
export async function createInterpreterWithFileSystem(): Promise<WasmInterpreter> {
    if (!wasmInitialized) {
        await initWasm();
    }

    const interpreter = new WasmInterpreter();
    const fs = getFileSystemService();

    // Check if filesystem is available
    if (!FileSystemService.isSupported()) {
        console.warn('[WASM] OPFS not supported, module imports will not work');
        return interpreter;
    }

    // Initialize filesystem if not already done
    try {
        await fs.initialize();
    } catch (e) {
        console.warn('[WASM] Failed to initialize filesystem:', e);
        return interpreter;
    }

    // Create a synchronous file provider callback for WASM
    // Note: WASM calls are synchronous, so we can't use async here directly
    // The WASM module will call this function and expect a string back
    const fileProvider = (_path: string): string => {
        // We need a workaround for async/sync mismatch
        // For now, we'll use a cached approach
        throw new Error(`Async file loading not yet supported. Use resolve_module() manually.`);
    };

    // Set the file provider (even though it throws, it registers the capability)
    try {
        interpreter.set_file_provider(fileProvider);
    } catch (e) {
        console.warn('[WASM] Could not set file provider:', e);
    }

    return interpreter;
}

/**
 * Resolve a module import manually (async-safe approach)
 * Call this from JS after detecting a `use` statement
 */
export async function resolveModule(
    interpreter: WasmInterpreter,
    path: string
): Promise<ModuleResolutionResult> {
    const fs = getFileSystemService();

    try {
        // Initialize filesystem if needed
        await fs.initialize();

        // Read the file content
        console.log(`[resolveModule] Reading file: ${path}`);
        const content = await fs.readFile(path);
        console.log(`[resolveModule] File content (${content.length} chars):`, content.substring(0, 100));

        // Pass to WASM for parsing and binding
        const wasmResult = interpreter.resolve_module(path, content);
        console.log('[resolveModule] WASM result:', wasmResult);

        // The WASM result might be a Map (from serde_wasm_bindgen) or a plain object
        // Handle both cases
        let result: ModuleResolutionResult;
        if (wasmResult instanceof Map) {
            result = {
                success: wasmResult.get('success') as boolean,
                exports: wasmResult.get('exports') as string[],
                error: wasmResult.get('error') as string | undefined
            };
        } else {
            result = wasmResult as ModuleResolutionResult;
        }

        console.log('[resolveModule] Parsed result:', result);
        return result;
    } catch (e) {
        console.error(`[resolveModule] Error loading '${path}':`, e);
        return {
            success: false,
            error: `Failed to load module '${path}': ${e}`
        };
    }
}

/** Result of get_use_statements call */
export interface UseStatementsResult {
    success: boolean;
    paths: string[];
    error?: string;
}

/**
 * Get all `use` statement paths from code
 * Used for pre-resolving imports before execution
 */
export function getUseStatements(code: string): UseStatementsResult {
    if (!wasmInitialized) {
        return { success: false, paths: [], error: 'WASM not initialized' };
    }

    try {
        return get_use_statements(code) as UseStatementsResult;
    } catch (e) {
        return { success: false, paths: [], error: String(e) };
    }
}

/**
 * Pre-resolve all imports in a script before execution.
 * This handles the async/sync bridge by resolving all `use` statements
 * before the synchronous WASM execution begins.
 * 
 * @param interpreter The WasmInterpreter instance to resolve into
 * @param code The source code containing potential `use` statements
 * @param onProgress Optional callback for progress updates
 * @returns List of resolved module paths, or throws on error
 */
export async function preResolveImports(
    interpreter: WasmInterpreter,
    code: string,
    onProgress?: (resolved: string[], remaining: number) => void
): Promise<string[]> {
    const resolved = new Set<string>();
    const pending = new Set<string>();
    const failed: string[] = [];

    // Get initial use statements
    const result = getUseStatements(code);
    console.log('[preResolveImports] Use statements result:', result);
    if (!result.success) {
        throw new Error(`Failed to parse code: ${result.error}`);
    }
    console.log('[preResolveImports] Found use paths:', result.paths);

    // Add to pending queue
    for (const path of result.paths) {
        pending.add(path);
    }

    // Resolve until all done
    while (pending.size > 0) {
        // Get first pending path (convert to array for safe access)
        const pendingArray = Array.from(pending);
        const path = pendingArray[0];
        pending.delete(path);

        // Skip if already resolved
        if (resolved.has(path)) continue;

        // Notify progress
        if (onProgress) {
            onProgress(Array.from(resolved), pending.size);
        }

        // Resolve the module
        const moduleResult = await resolveModule(interpreter, path);

        if (!moduleResult.success) {
            console.warn(`Failed to resolve module '${path}': ${moduleResult.error}`);
            failed.push(path);
            continue;
        }

        resolved.add(path);

        // Read the module content to check for nested imports
        try {
            const fs = getFileSystemService();
            const content = await fs.readFile(path);
            const nestedResult = getUseStatements(content);

            if (nestedResult.success) {
                for (const nestedPath of nestedResult.paths) {
                    if (!resolved.has(nestedPath)) {
                        pending.add(nestedPath);
                    }
                }
            }
        } catch (e) {
            // If we can't read the module content for nested imports, that's okay
            // The module itself was already resolved
            console.debug(`Could not check nested imports for '${path}': ${e}`);
        }
    }

    if (failed.length > 0) {
        console.warn(`Failed to resolve modules: ${failed.join(', ')}`);
    }

    return Array.from(resolved);
}
