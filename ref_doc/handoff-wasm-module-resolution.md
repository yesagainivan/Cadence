# Handoff: Module Resolution in WASM (Async/Sync Bridge)

## Problem Summary

The `use "file.cadence"` statement works perfectly in the native REPL, but has a fundamental limitation in the web editor due to the **async/sync mismatch** between OPFS file access and WASM execution.

---

## The Technical Challenge

### Native REPL Flow (Works Perfectly)
```
Interpreter encounters Statement::Use
    → ModuleResolver::resolve(path)
        → NativeFileProvider::read_file(path)  // SYNC: std::fs::read_to_string
        → Parse module
        → Bind exports to environment
    → Continue execution
```

### Web/WASM Flow (The Problem)
```
Interpreter encounters Statement::Use
    → Need to call JavaScript to get file contents
    → JS FileSystemService.readFile() is ASYNC (returns Promise)
    → WASM is running SYNCHRONOUSLY 
    → ❌ WASM cannot await a Promise!
```

The core issue is that **WASM runs synchronously**, but the browser's file system APIs (OPFS) are all **async/Promise-based**. WASM cannot pause execution to wait for a Promise to resolve.

---

## Current Workaround

We've implemented a two-step approach:

### 1. Manual Resolution via JS
```typescript
// In TypeScript (async-safe):
const result = await resolveModule(interpreter, "drums.cadence");
// This: 1) Reads file from OPFS, 2) Passes to WASM for parsing, 3) Binds exports
```

### 2. WASM `resolve_module()` Method
```rust
// In wasm.rs - accepts already-loaded file content:
pub fn resolve_module(&mut self, path: &str, content: &str) -> JsValue {
    // Parse the content and bind exports to environment
    // This is SYNC because content is already loaded
}
```

---

## What This Means for Users

### Current State
- ✅ `use` statements **parse correctly** (syntax highlighting, no errors)
- ❌ `use` statements **don't resolve automatically** during playback in editor
- ✅ Files can be created/edited in the file tree
- ⚠️ To actually use imports, the editor needs to pre-resolve them

### Example of the Limitation
```cadence
// drums.cadence exists in file tree with:
// let kick = "bd bd bd bd"

// main.cadence
use "drums.cadence"  // ← Parses OK, but doesn't actually load in editor
play kick loop       // ← Error: undefined variable 'kick'
```

---

## Proposed Solutions

### Option A: Pre-Parse Imports (Recommended)

Before executing any script, scan for `use` statements and resolve them first:

```typescript
// Before calling play/run:
async function preResolveImports(code: string): Promise<void> {
    const useStatements = extractUseStatements(code);  // Parse AST for use statements
    
    for (const stmt of useStatements) {
        await resolveModule(interpreter, stmt.path);
        // Recursively resolve nested imports
    }
}

// Then execute
audioEngine.playScript(code);
```

**Pros:** 
- Works with existing WASM structure
- No changes to interpreter

**Cons:**
- Two-pass execution (parse → resolve → execute)
- Needs recursion tracking for circular imports

---

### Option B: Web Worker with Synchronous OPFS

OPFS provides synchronous access in Web Workers via `createSyncAccessHandle()`:

```typescript
// In a Web Worker:
const root = await navigator.storage.getDirectory();
const file = await root.getFileHandle("drums.cadence");
const handle = await file.createSyncAccessHandle();  // SYNC access!
const buffer = new ArrayBuffer(handle.getSize());
handle.read(buffer);  // SYNC read
```

Then WASM could call into this sync API via `js-sys::Function`.

**Pros:**
- True synchronous resolution during execution
- Matches native behavior exactly

**Cons:**
- Requires restructuring to run WASM in Worker
- More complex architecture
- Message passing between main thread and worker

---

### Option C: Atomics-Based Async Bridging

Use `Atomics.wait()` and `SharedArrayBuffer` to block WASM thread:

```
Main thread: Start async file read
Worker thread (WASM): Atomics.wait() on shared buffer
Main thread: When file loaded, Atomics.notify()
Worker thread: Continues with file content
```

**Pros:**
- True synchronous behavior from WASM perspective

**Cons:**
- Requires cross-origin isolation headers (COOP/COEP)
- Complex implementation
- May not work in all browsers

---

## Recommendation

**Option A (Pre-Parse Imports)** is the most pragmatic path:

1. Add a `extractUseStatements()` function that parses AST for `Statement::Use`
2. In `audioEngine.playScript()`, first call `preResolveImports(code)`
3. Then execute the script as normal

This can be implemented in ~50-100 lines of TypeScript and doesn't require WASM changes.

---

## Implementation Checklist

- [ ] Add WASM function `get_use_statements(code)` that returns list of import paths
- [ ] Implement `preResolveImports()` in TypeScript with recursion tracking
- [ ] Call `preResolveImports()` before `playScript()` in audio engine
- [ ] Handle circular imports (return cached exports)
- [ ] Show loading indicator for large module trees
- [ ] Error handling: show which module failed to load

---

## Files Involved

| File | Purpose |
|------|---------|
| `cadence-core/src/wasm.rs` | WASM interpreter, `resolve_module()` |
| `editor/src/filesystem-service.ts` | OPFS file access |
| `editor/src/cadence-wasm.ts` | `resolveModule()` helper |
| `editor/src/audio-engine.ts` | Where `playScript()` is called |
