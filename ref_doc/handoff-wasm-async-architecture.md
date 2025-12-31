# Proper Architecture for WASM Async Module Resolution

## The Core Problem

WASM runs **synchronously** but browser file APIs (OPFS) are **async**. When Cadence encounters `use "file.cadence"`, it needs to:
1. Read the file (async in browser)
2. Parse and bind exports (sync WASM)
3. Continue execution (sync WASM)

This creates a fundamental impedance mismatch.

---

## Current State (Hacky)

```
User code → get_use_statements → preResolveImports → load → play
                    ↓
           [Suppress errors in preview]
```

**Problems:**
- Piano roll can't show preview for code with imports
- Properties panel doesn't know about imported symbols
- Errors are suppressed instead of handled properly
- Two separate interpreters (preview vs playback)

---

## Proper Architectural Options

### Option A: Shared Persistent Interpreter

**Architecture:**
```
                 ┌──────────────────────────────────────────┐
                 │      SharedInterpreter (Singleton)       │
                 │                                          │
                 │  • environment (all resolved imports)    │
                 │  • resolvedModules: Map<path, exports>   │
                 └────────────────┬─────────────────────────┘
                                  │
          ┌───────────────────────┼───────────────────────┐
          │                       │                       │
          ▼                       ▼                       ▼
    Piano Roll              Properties            Audio Engine
    (sync read)             (sync read)           (sync exec)
```

**Implementation:**
1. Create a singleton `SharedInterpreter` in TypeScript
2. On file save in OPFS: trigger async `resolveModule()` to update interpreter
3. All features (piano roll, properties, playback) read from same interpreter
4. The interpreter is always "current" with imports

**Pros:**
- Single source of truth
- All features have access to imports
- No suppression of errors needed

**Cons:**
- More complex state management
- Need to handle module invalidation when files change

---

### Option B: Web Worker + SyncAccessHandle

**Architecture:**
```
    Main Thread                      Web Worker
    ┌─────────────┐                  ┌─────────────────────────┐
    │   Editor    │◄────postMessage────│  WasmInterpreter      │
    │   UI        │                    │                       │
    │             │────postMessage────►│  FileProvider:        │
    └─────────────┘                    │  • createSyncHandle() │
                                       │  • readSync()         │
                                       └───────────┬───────────┘
                                                   │
                                                   ▼
                                             OPFS (sync!)
```

**Implementation:**
1. Move WASM interpreter to Web Worker
2. Use `FileSystemSyncAccessHandle` for synchronous file I/O
3. Worker exposes API via `postMessage`
4. Main thread sends commands, receives results

**Pros:**
- True synchronous file I/O
- Native `use` statement works unmodified
- Matches REPL behavior exactly

**Cons:**
- Significant refactoring (all WASM interaction becomes async)
- UI updates need serialization/deserialization
- Worker-based architecture complexity

---

### Option C: Virtual Module Registry (Recommended First Step)

**Architecture:**
```
    ┌────────────────────┐
    │  FileSystemService │──────┐
    │  (watches changes) │      │
    └────────────────────┘      │
              │                 │
              ▼                 ▼
    ┌─────────────────────────────────────┐
    │        ModuleRegistry               │
    │                                     │
    │  modules: Map<path, {              │
    │    content: string,                 │
    │    exports: Map<name, Value>,       │
    │    dependencies: string[]           │
    │  }>                                 │
    └─────────────────────────────────────┘
              │
              ▼
    ┌─────────────────────────────────────┐
    │      WasmInterpreter.inject(        │
    │        registry.getExports()        │
    │      )                              │
    └─────────────────────────────────────┘
```

**Implementation:**
1. Create `ModuleRegistry` that watches OPFS
2. When any `.cadence` file changes, parse it and cache exports
3. Before any evaluation (piano roll, play, properties), inject cached exports
4. WASM `use` statement becomes a no-op (exports already injected)

**Pros:**
- Incremental improvement over current state
- All features share same module knowledge
- Can be implemented without major refactoring

**Cons:**
- Module updates lag slightly behind file saves
- Still need to track dependencies for proper invalidation

---

## Recommendation

### Phase 1: Virtual Module Registry (Immediate)
- Implement `ModuleRegistry` that watches file changes
- Pre-parse all `.cadence` files and cache their exports
- Inject exports into interpreter before any evaluation
- Remove error suppression hack

### Phase 2: Web Worker Migration (Future)
- Move interpreter to Web Worker
- Use SyncAccessHandle for true sync file I/O
- Full parity with native REPL

---

## Do Web Users Need `use`?

**Yes.** The web editor should support multi-file projects:

1. **Reusability**: Define patterns/functions once, use everywhere
2. **Organization**: Split large compositions into logical files
3. **Sharing**: Import community libraries (future)
4. **Parity**: Same code runs in REPL and editor

Without `use`, web editor users are limited to single-file projects, which doesn't scale for serious compositions.

---

## Implementation Priority

```
[ ] Phase 1: ModuleRegistry
    [ ] FileSystemService: add watch/onChange callback
    [ ] ModuleRegistry: parse & cache exports on change
    [ ] Interpreter: add injectExports(Map<name, Value>)
    [ ] Piano roll: use shared registry
    [ ] Properties: use shared registry
    [ ] Remove error suppression hack

[ ] Phase 2: Web Worker (if needed)
    [ ] Move WasmInterpreter to worker
    [ ] Implement worker message protocol
    [ ] Add SyncAccessHandle file provider
    [ ] Refactor all WASM call sites
```
