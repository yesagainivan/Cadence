/**
 * WASM Loader for Cadence Core
 * 
 * Loads and initializes the cadence-core WASM module,
 * providing access to tokenization and parsing functions.
 */

import init, { tokenize, parse_and_check } from './wasm/cadence_core.js';

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

// Re-export for convenience
export { tokenize, parse_and_check };
