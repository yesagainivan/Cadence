# Symbol Table Architecture - Handoff Document

> **Branch**: `feat/proper-hover-architecture`  
> **Status**: In Progress (Phase 3)  
> **Last Updated**: 2025-12-30

## Overview

Building a proper Symbol Table layer to enable reactive hover, autocomplete, and future IDE features. This replaces the WIP cache-based approach.

---

## Architecture

```
┌─────────────┐     ┌─────────┐     ┌──────────────┐
│ Source Code │ ──▶ │ Parser  │ ──▶ │ SpannedAST   │
└─────────────┘     └─────────┘     └──────────────┘
                                           │
                                           ▼
                                    ┌──────────────┐
                                    │   Binder     │
                                    └──────────────┘
                                           │
                                           ▼
                                    ┌──────────────┐
                                    │ Symbol Table │
                                    └──────────────┘
                                           │
                    ┌──────────────────────┼──────────────────────┐
                    ▼                      ▼                      ▼
             ┌────────────┐         ┌────────────┐         ┌────────────┐
             │   Hover    │         │ Completion │         │ Diagnostics│
             └────────────┘         └────────────┘         └────────────┘
```

---

## Implementation Phases

### Phase 1: Symbol Table Core ✅

**Files created:**
- [symbols.rs](file:///Users/ivanowono/Documents/Code/Rusty/DSP/cadence/cadence-core/src/parser/symbols.rs)

**Completed:**
- `SymbolTable`, `FunctionSymbol`, `VariableSymbol`, `Span` structs
- `get()`, `get_at_position()`, `add_function()`, `add_variable()` methods
- Unit tests passing

---

### Phase 2: Binder ✅

**Files created:**
- [binder.rs](file:///Users/ivanowono/Documents/Code/Rusty/DSP/cadence/cadence-core/src/parser/binder.rs)

**Completed:**
- `Binder::bind(SpannedProgram) -> SymbolTable`
- Extracts function definitions and variable bindings
- Handles nested blocks (if, loop, track, etc.)
- 4 unit tests passing

---

### Phase 3: WASM Exposure ⏳ **CURRENT**

**Files to modify:**
- [wasm.rs](file:///Users/ivanowono/Documents/Code/Rusty/DSP/cadence/cadence-core/src/wasm.rs)

**Tasks:**
1. [ ] Add `get_symbols(code: &str) -> JsValue` - returns all symbols as JSON
2. [ ] Add `get_symbol_at_position(code: &str, pos: usize) -> JsValue` - for hover

---

### Phase 4: Editor Integration ⏳

**Files to modify:**
- `editor/src/hover.ts`
- `editor/src/main.ts` or new `language-service.ts`

**Tasks:**
1. [ ] Call `get_symbols()` on text change (debounced)
2. [ ] Use `get_symbol_at_position()` for hover tooltips
3. [ ] Remove old cache-based approach and audio-engine dependency

---

## Progress Log

| Date | Session | Progress |
|------|---------|----------|
| 2025-12-30 | 1 | Created branch, research, architecture doc. Phase 1 & 2 complete. |

---

## Before Merging

- [ ] All 4 phases complete
- [ ] Tests passing
- [ ] Manual test: hover works on user functions
- [ ] Manual test: functions disappear when commented
- [ ] Clean up old WIP code
