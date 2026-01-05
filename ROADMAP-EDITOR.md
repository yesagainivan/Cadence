# Cadence Editor Roadmap

A web-based editor for the Cadence music programming language with live syntax highlighting, MIDI visualization, and contextual property editing.

---

## Phase 0: WASM Foundation ✅ Complete

### 0.1 Type Extraction ✅
- [x] Create `types/audio_config.rs` with WASM-compatible types
- [x] Move `Waveform`, `AdsrParams`, `QueueMode` from audio module
- [x] Update all imports across codebase
- [x] All 232 tests passing

### 0.2 Crate Split ✅
- [x] Create workspace with `cadence-core` crate
- [x] Move types + parser modules to core crate
- [x] Add `serde` and `wasm` features for JSON/WASM interop
- [x] Test `wasm32-unknown-unknown` compilation target
- [x] Handle `colored` crate with conditional compilation
- [x] Add Comment token support to lexer

### 0.3 Crate Consolidation ✅ (New)
- [x] `cadence` re-exports from `cadence-core` (no duplicate code)
- [x] Interpreter moved to core (WASM-accessible)
- [x] File loading conditionally compiled (disabled in WASM)
- [x] Full script execution now possible in browser

---

## Phase 1: Syntax Highlighting ✅ Complete

### 1.1 Tokenization API ✅
- [x] Create `tokenize_for_highlighting(input: &str) -> Vec<HighlightSpan>` 
- [x] Map Token types to highlight classes (keyword, note, number, operator, etc.)
- [x] Handle partial/incomplete input gracefully
- [x] Fix token position after whitespace (span captured after skip)

### 1.2 CodeMirror 6 Integration ✅
- [x] Create `editor/` folder with Vite + TypeScript setup
- [x] WASM bindings via `wasm-bindgen` + `serde-wasm-bindgen`
- [x] Custom CodeMirror language mode using WASM tokenizer
- [x] Real-time highlighting as user types
- [x] Dark theme with music production colors
- [x] Real-time code validation via `parse_and_check`

---

## Phase 2: Live MIDI Display ✅ Complete

### 2.1 Piano Roll Visualization ✅
- [x] Canvas-based piano roll component (`piano-roll.ts`)
- [x] Parse patterns to extract notes with timing (`get_events_at_position`)
- [x] Color-code notes by pitch class (12 colors in `NOTE_COLORS`)
- [x] Playhead indicator for current beat
- [x] Animated playhead synced to audio scheduler (`startAnimation()`)

### 2.2 Pattern Data API ✅
- [x] `to_events()` returns frequencies, durations, rest flags
- [x] Expose `get_events_at_position(code, pos)` WASM function for visualization
- [x] Include cycle timing from pattern mini-notation (`beats_per_cycle`)
- [x] Beat sync via `audioEngine.getPlaybackPosition()`

### 2.3 Staff Notation (Stretch)
- [ ] VexFlow or similar for traditional notation
- [ ] Real-time update as code changes

---

## Phase 3: Properties Panel ✅ Complete

### 3.1 Cursor Context API ✅
- [x] `get_context_at_cursor(input: &str, pos: usize) -> CursorContext`
- [x] Return AST node type, parent context, editable properties
- [x] Handle `Statement::Track` to extract inner Play target
- [x] Handle `Statement::Play`, `Let`, `Assign`, `Expression`

### 3.2 Property Editors ✅
- [x] **Envelope Editor**: Visual ADSR curve (attack, decay, sustain, release)
- [x] **Waveform Picker**: Sine, saw, square, triangle with preview
- [ ] **Pattern Editor**: Step sequencer view for pattern mini-notation *(stretch)*
- [ ] **Chord Wheel**: Circle of fifths / chord quality selector *(stretch)*

### 3.3 Bidirectional Sync ✅
- [x] Editing in panel updates source code
- [x] Source code changes update panel in real-time
- [x] Smart insertion point (before `loop`/`queue` keywords)

---

## Phase 4: Web Audio Playback ✅ Complete

### 4.1 Web Audio Engine ✅
- [x] JavaScript-based oscillator/envelope generation
- [x] ADSR envelope with customizable parameters
- [x] Pattern scheduling with Web Audio clock
- [x] Reactive playback via `WasmInterpreter` tick system
- [x] Live coding support (update without cycle reset)
- [ ] AudioWorklet for lower latency (stretch goal)

### 4.2 Transport Controls ✅
- [x] Play/Stop functionality
- [x] Tempo control (BPM slider) connected to engine
- [x] Loop/cycle visualization (playhead indicator)

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    Web Editor UI                      │
│  ┌───────────────┬───────────────┬──────────────────┐│
│  │ CodeMirror 6  │  Piano Roll   │ Properties Panel ││
│  │(syntax editor)│ (MIDI display)│ (contextual edit)││
│  └───────────────┴───────────────┴──────────────────┘│
│                         ▲                             │
│                         │ JS/TS                       │
├─────────────────────────┼─────────────────────────────┤
│                   wasm-bindgen                        │
├─────────────────────────┼─────────────────────────────┤
│              ┌──────────┴──────────┐                  │
│              │   cadence-core.wasm │                  │
│              │  ┌────────────────┐ │                  │
│              │  │ types/         │ │                  │
│              │  │ parser/        │ │                  │
│              │  │ interpreter/   │ │  ← NEW!          │
│              │  │ wasm.rs (API)  │ │                  │
│              │  └────────────────┘ │                  │
│              └─────────────────────┘                  │
└──────────────────────────────────────────────────────┘
```

---

## Key Technologies

| Component | Technology | Status |
|-----------|------------|--------|
| Editor | CodeMirror 6 | ✅ Integrated |
| WASM bindings | wasm-bindgen, wasm-pack | ✅ Working |
| Build tool | Vite + TypeScript | ✅ Setup |
| Tokenization | WASM (Rust lexer) | ✅ Working |
| Validation | WASM (Rust parser) | ✅ Working |
| Audio playback | Web Audio API | ✅ Working |
| MIDI visualization | Canvas 2D | ✅ Working |
| Property editing | Properties Panel | ✅ Working |

---

## Completed This Session

1. Created `cadence-core` workspace crate with WASM support
2. Implemented `tokenize()` and `parse_and_check()` WASM exports
3. Built web editor with CodeMirror 6 + custom Cadence language mode
4. Added Comment token to lexer for syntax highlighting
5. Fixed token positions (span captured after whitespace skip)
6. Real-time validation with Rust parser
7. Dark theme with music production colors
8. **Reactive playback architecture** — Step-sequencer pattern with per-cycle caching
9. **Phase-preserving updates** — Variable reassignment doesn't stutter playback
10. **WasmInterpreter** — Stateful interpreter for browser script execution
11. **Web Audio playback** — Full oscillator/ADSR synthesis in `audio-engine.ts`
12. **Transport controls** — Play/Stop buttons and tempo slider
13. **Live coding** — `updateScript()` preserves cycle position during edits
14. **Statement span tracking** — `SpannedStatement`/`SpannedProgram` types with byte offset tracking
15. **Cursor-aware piano roll** — `get_events_at_position()` WASM function shows statement at cursor
16. **Go-to-Definition** — Cmd+Click and F12 to jump to symbol definition
17. **Simplification to Single-File Playground** — Removed multi-file infrastructure (~2,000 lines):
    - Deleted: `file-tree.ts`, `tab-bar.ts`, `filesystem-service.ts`, `cadence-worker.ts`, `interpreter-client.ts`
    - Direct `WasmInterpreter` playback (no Web Worker)
    - localStorage auto-save for code persistence
    - `use` statements reserved for CLI/IDE only

---

## Design Decision: Single-File Playground

The web editor was simplified from a multi-file IDE to a **single-file playground** (similar to Strudel):

- **Goal**: Fast, focused live-coding experience
- **Removed**: File tree, tabs, virtual filesystem, Web Worker bridge
- **Preserved**: Piano roll, properties panel, syntax highlighting, live coding
- **Module imports**: `use` statements work in CLI; future web imports via GitHub URLs

---

## Next Steps

1. **Piano Roll Lock** — Allow user to lock piano roll on a specific pattern
   - Lock icon in piano roll panel header
   - Locked state: piano roll stays on locked pattern regardless of cursor
   - Unlocked state: follows cursor (current behavior)
2. **Code Cleanup** — Refactor WASM API
   - Unify `get_context_at_cursor()` and `get_events_at_position()` via shared `get_visualizable_expression()` helper
   - Remove unused imports in `evaluator.rs` and `wasm.rs`
3. **GitHub URL Imports** — Load libraries/samples via URL (e.g., `import "https://..."`)
4. **AudioWorklet** — Lower latency audio scheduling *(stretch)*
5. **Staff Notation** — VexFlow integration for traditional notation *(stretch)*
