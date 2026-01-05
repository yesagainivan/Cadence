/**
 * Cadence Web Worker
 * 
 * Runs the WASM interpreter in a dedicated worker thread with synchronous
 * file access via FileSystemSyncAccessHandle. This enables true synchronous
 * `use` statement resolution, matching native REPL behavior.
 */

// Import WASM module
import init, { WasmInterpreter, tokenize, parse_and_check, get_symbols, get_use_statements } from './wasm/cadence_core.js';

// Type definitions for messages
interface WorkerRequest {
    id: number;
    method: string;
    args: Record<string, unknown>;
}

interface WorkerResponse {
    id: number;
    success: boolean;
    result?: unknown;
    error?: string;
}

// Worker state
let interpreter: WasmInterpreter | null = null;
let opfsRoot: FileSystemDirectoryHandle | null = null;
let initialized = false;

// NOTE: True sync file access via FileSystemSyncAccessHandle is the ultimate goal.
// For now, we pre-resolve imports asynchronously before script execution.
// This still gives us the key benefit: ALL UI features share the same interpreter.

/**
 * Read a file asynchronously (fallback for initial setup)
 */
async function readFileAsync(path: string): Promise<string> {
    if (!opfsRoot) {
        throw new Error('OPFS not initialized');
    }

    const normalizedPath = path.replace(/^\/+/, '');
    const parts = normalizedPath.split('/').filter(p => p.length > 0);

    if (parts.length === 0) {
        throw new Error('Invalid file path');
    }

    const fileName = parts.pop()!;

    // Navigate to parent directory
    let current: FileSystemDirectoryHandle = opfsRoot;
    for (const part of parts) {
        current = await current.getDirectoryHandle(part);
    }

    // Get file and read content
    const fileHandle = await current.getFileHandle(fileName);
    const file = await fileHandle.getFile();
    return await file.text();
}

/**
 * Helper to pre-resolve all imports for a piece of code.
 * Uses the interpreter's environment which persists across calls.
 */
async function preResolveImports(code: string): Promise<void> {
    if (!interpreter) return;

    const useResult = get_use_statements(code);
    const usePaths = (useResult as { paths?: string[] }).paths || [];

    for (const path of usePaths) {
        try {
            const content = await readFileAsync(path);
            interpreter.resolve_module(path, content);
        } catch (e) {
            // Module may not exist yet - that's fine
        }
    }
}

/**
 * Initialize the worker: load WASM, connect to OPFS
 */
async function initialize(): Promise<void> {
    if (initialized) return;

    console.log('[Worker] Initializing WASM...');
    await init();
    console.log('[Worker] WASM initialized');

    console.log('[Worker] Connecting to OPFS...');
    opfsRoot = await navigator.storage.getDirectory();
    console.log('[Worker] OPFS connected');

    // Create interpreter with file provider
    interpreter = new WasmInterpreter();
    console.log('[Worker] Interpreter created');

    initialized = true;
}

/**
 * Handle method calls from the main thread
 */
async function handleMethod(method: string, args: Record<string, unknown>): Promise<unknown> {
    if (!initialized && method !== 'initialize') {
        throw new Error('Worker not initialized. Call initialize() first.');
    }

    switch (method) {
        case 'initialize':
            await initialize();
            return { success: true };

        case 'load': {
            if (!interpreter) throw new Error('No interpreter');
            const code = args.code as string;

            // Pre-resolve imports before loading
            const useResult = get_use_statements(code);
            const usePaths = (useResult as { paths?: string[] }).paths || [];

            for (const path of usePaths) {
                try {
                    const content = await readFileAsync(path);
                    interpreter.resolve_module(path, content);
                    console.log(`[Worker] Resolved module: ${path}`);
                } catch (e) {
                    console.warn(`[Worker] Failed to resolve ${path}:`, e);
                }
            }

            return interpreter.load(code);
        }

        case 'tick': {
            if (!interpreter) throw new Error('No interpreter');
            return interpreter.tick();
        }

        case 'update': {
            if (!interpreter) throw new Error('No interpreter');
            const code = args.code as string;
            return interpreter.update(code);
        }

        case 'resolveModule': {
            if (!interpreter) throw new Error('No interpreter');
            const path = args.path as string;
            const content = await readFileAsync(path);
            return interpreter.resolve_module(path, content);
        }

        case 'getEventsAtPosition': {
            // Uses the interpreter's environment which has resolved imports
            if (!interpreter) throw new Error('No interpreter');
            const code = args.code as string;
            const position = args.position as number;

            // Pre-resolve any imports first
            await preResolveImports(code);

            // Call the new WASM method that uses interpreter's environment
            return interpreter.get_events_for_statement(code, position);
        }

        case 'getContextAtCursor': {
            if (!interpreter) throw new Error('No interpreter');
            const code = args.code as string;
            const position = args.position as number;

            // Pre-resolve any imports first
            await preResolveImports(code);

            // Call the new WASM method that uses interpreter's environment
            return interpreter.get_context_for_statement(code, position);
        }

        case 'tokenize': {
            const input = args.input as string;
            return tokenize(input);
        }

        case 'parseAndCheck': {
            const input = args.input as string;
            return parse_and_check(input);
        }

        case 'getSymbols': {
            const code = args.code as string;
            return get_symbols(code);
        }

        case 'readFile': {
            const path = args.path as string;
            return await readFileAsync(path);
        }

        default:
            throw new Error(`Unknown method: ${method}`);
    }
}

/**
 * Message handler for the worker
 */
self.onmessage = async (e: MessageEvent<WorkerRequest>) => {
    const { id, method, args } = e.data;

    try {
        const result = await handleMethod(method, args);
        self.postMessage({ id, success: true, result } as WorkerResponse);
    } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        self.postMessage({ id, success: false, error: errorMessage } as WorkerResponse);
    }
};

// Signal that the worker is ready
console.log('[Worker] Cadence Worker loaded, waiting for initialization...');
