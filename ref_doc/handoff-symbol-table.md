# Symbol Table Architecture - Handoff Document

> **Branch**: `feat/proper-hover-architecture`  
> **Status**: In Progress  
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

## Data Structures

### SymbolTable

```rust
pub struct SymbolTable {
    pub functions: HashMap<String, FunctionSymbol>,
    pub variables: HashMap<String, VariableSymbol>,
}

pub struct FunctionSymbol {
    pub name: String,
    pub params: Vec<String>,
    pub span: Span,
    pub doc_comment: Option<String>,  // Future: /// comments
}

pub struct VariableSymbol {
    pub name: String,
    pub value_type: Option<String>,  // Inferred or annotated
    pub span: Span,
}

pub struct Span {
    pub start: usize,
    pub end: usize,
    pub utf16_start: usize,
    pub utf16_end: usize,
}
```

---

## Implementation Phases

### Phase 1: Symbol Table Core ⏳

**Files to create/modify:**
- `cadence-core/src/parser/symbols.rs` (NEW)
- `cadence-core/src/parser/mod.rs` (add module)

**Tasks:**
1. Create `SymbolTable`, `FunctionSymbol`, `VariableSymbol`, `Span`
2. Add `new()`, `add_function()`, `add_variable()`, `get_function()`, `get_at_position()`

### Phase 2: Binder ⏳

**Files to create/modify:**
- `cadence-core/src/parser/binder.rs` (NEW)

**Tasks:**
1. Create `Binder` struct
2. Implement `bind_program(SpannedProgram) -> SymbolTable`
3. Walk AST, extract function defs → add to table
4. Walk AST, extract let bindings → add to table
5. Handle nested scopes (push/pop scope in binder)

### Phase 3: WASM Exposure ⏳

**Files to modify:**
- `cadence-core/src/wasm.rs`

**Tasks:**
1. Add `bind_and_get_symbols(code: &str) -> JsValue`
2. Returns all symbols for the editor's internal use
3. Add `get_symbol_at_position(code: &str, pos: usize) -> JsValue`
4. Returns the symbol (if any) at cursor position

### Phase 4: Editor Integration ⏳

**Files to modify:**
- `editor/src/hover.ts` - use new API
- `editor/src/lang-cadence.ts` or new `editor/src/language-service.ts`
- `editor/src/main.ts` - wire debounced updates

**Tasks:**
1. On text change (debounced 100ms), call `bind_and_get_symbols()`
2. Store symbols in module-level state
3. `hoverTooltip` calls `get_symbol_at_position()` or looks up in local cache
4. Remove dependency on `audioEngine` / Play button

---

## Progress Log

| Date | Session | Progress |
|------|---------|----------|
| 2025-12-30 | 1 | Created branch, research, architecture doc |

---

## Notes for Next Session

- Start with Phase 1: `symbols.rs`
- The `Span` type already exists partially in `ast.rs` as `SpannedStatement`
- Consider if we need separate spans or can reuse existing infrastructure

---

## Before Merging

1. [ ] All 4 phases complete
2. [ ] Tests for Symbol Table and Binder
3. [ ] Manual test: hover works on user functions
4. [ ] Manual test: functions disappear when commented
5. [ ] Clean up old WIP code (cache in hover.ts, audio-engine hooks)
6. [ ] Update README if needed
