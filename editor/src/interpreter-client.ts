/**
 * Interpreter Client
 * 
 * Async wrapper for communicating with the Cadence Web Worker.
 * All WASM operations go through this client, which handles the
 * postMessage/onmessage protocol with the worker.
 */

// Import types from cadence-wasm for now (we'll consolidate later)
import type {
    ParseResult,
    PatternEvents,
    CursorContext,
    SymbolsResult,
    ScriptResult,
    HighlightSpan
} from './cadence-wasm';

// Worker request/response types
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

// Pending request tracking
interface PendingRequest {
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
}

/**
 * InterpreterClient - Singleton that manages communication with the Cadence Worker
 */
export class InterpreterClient {
    private static instance: InterpreterClient | null = null;
    private worker: Worker | null = null;
    private pending: Map<number, PendingRequest> = new Map();
    private nextId = 0;
    private initPromise: Promise<void> | null = null;
    private initialized = false;

    private constructor() { }

    /**
     * Get the singleton instance
     */
    static getInstance(): InterpreterClient {
        if (!InterpreterClient.instance) {
            InterpreterClient.instance = new InterpreterClient();
        }
        return InterpreterClient.instance;
    }

    /**
     * Initialize the worker and WASM
     */
    async initialize(): Promise<void> {
        if (this.initialized) return;

        if (this.initPromise) {
            return this.initPromise;
        }

        this.initPromise = this.doInitialize();
        return this.initPromise;
    }

    private async doInitialize(): Promise<void> {
        console.log('[Client] Creating Cadence Worker...');

        // Create the worker
        // Note: The path needs to match your build output
        this.worker = new Worker(
            new URL('./cadence-worker.ts', import.meta.url),
            { type: 'module' }
        );

        // Set up message handler
        this.worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
            const { id, success, result, error } = e.data;
            const pending = this.pending.get(id);

            if (pending) {
                this.pending.delete(id);
                if (success) {
                    pending.resolve(result);
                } else {
                    pending.reject(new Error(error || 'Unknown worker error'));
                }
            }
        };

        this.worker.onerror = (e) => {
            console.error('[Client] Worker error:', e);
        };

        // Initialize WASM in the worker
        await this.call('initialize', {});
        this.initialized = true;
        console.log('[Client] Worker initialized successfully');
    }

    /**
     * Call a method on the worker
     */
    private call(method: string, args: Record<string, unknown>): Promise<unknown> {
        return new Promise((resolve, reject) => {
            if (!this.worker) {
                reject(new Error('Worker not created'));
                return;
            }

            const id = this.nextId++;
            this.pending.set(id, { resolve, reject });

            const request: WorkerRequest = { id, method, args };
            this.worker.postMessage(request);
        });
    }

    /**
     * Check if the client is ready
     */
    isReady(): boolean {
        return this.initialized;
    }

    // =========================================================================
    // Public API - All async, mirrors WasmInterpreter methods
    // =========================================================================

    /**
     * Load and execute a script
     */
    async load(code: string): Promise<ScriptResult> {
        await this.initialize();
        return await this.call('load', { code }) as ScriptResult;
    }

    /**
     * Advance time by one beat
     */
    async tick(): Promise<ScriptResult> {
        await this.initialize();
        return await this.call('tick', {}) as ScriptResult;
    }

    /**
     * Update script without resetting cycle
     */
    async update(code: string): Promise<ScriptResult> {
        await this.initialize();
        return await this.call('update', { code }) as ScriptResult;
    }

    /**
     * Get pattern events at cursor position
     * Uses interpreter's environment (with resolved imports)
     */
    async getEventsAtPosition(code: string, position: number): Promise<PatternEvents | null> {
        await this.initialize();
        const result = await this.call('getEventsAtPosition', { code, position });

        // Check if we got a "needs WASM method" marker
        if (result && typeof result === 'object' && 'needsWasmMethod' in result) {
            // TODO: Once we add get_events_for_statement to WASM, this will work
            console.warn('[Client] getEventsAtPosition needs new WASM method');
            return null;
        }

        return result as PatternEvents | null;
    }

    /**
     * Get cursor context for properties panel
     * Uses interpreter's environment (with resolved imports)
     */
    async getContextAtCursor(code: string, position: number): Promise<CursorContext | null> {
        await this.initialize();
        const result = await this.call('getContextAtCursor', { code, position });

        if (result && typeof result === 'object' && 'needsWasmMethod' in result) {
            console.warn('[Client] getContextAtCursor needs new WASM method');
            return null;
        }

        return result as CursorContext | null;
    }

    /**
     * Tokenize code for syntax highlighting
     */
    async tokenize(input: string): Promise<HighlightSpan[]> {
        await this.initialize();
        return await this.call('tokenize', { input }) as HighlightSpan[];
    }

    /**
     * Parse and validate code
     */
    async parseAndCheck(input: string): Promise<ParseResult> {
        await this.initialize();
        return await this.call('parseAndCheck', { input }) as ParseResult;
    }

    /**
     * Get symbols from code
     */
    async getSymbols(code: string): Promise<SymbolsResult> {
        await this.initialize();
        return await this.call('getSymbols', { code }) as SymbolsResult;
    }

    /**
     * Manually resolve a module
     */
    async resolveModule(path: string): Promise<{ success: boolean; exports?: string[]; error?: string }> {
        await this.initialize();
        return await this.call('resolveModule', { path }) as { success: boolean; exports?: string[]; error?: string };
    }

    /**
     * Read a file from OPFS
     */
    async readFile(path: string): Promise<string> {
        await this.initialize();
        return await this.call('readFile', { path }) as string;
    }

    /**
     * Terminate the worker
     */
    terminate(): void {
        if (this.worker) {
            this.worker.terminate();
            this.worker = null;
            this.initialized = false;
            this.initPromise = null;
            this.pending.clear();
        }
    }
}

// Export singleton getter for convenience
export function getInterpreterClient(): InterpreterClient {
    return InterpreterClient.getInstance();
}

// Export async initialization helper
export async function initializeInterpreter(): Promise<InterpreterClient> {
    const client = getInterpreterClient();
    await client.initialize();
    return client;
}
