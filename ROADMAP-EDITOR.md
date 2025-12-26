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

## Phase 2: Live MIDI Display 🔜 Next

### 2.1 Piano Roll Visualization
- [ ] Canvas-based piano roll component
- [ ] Parse patterns to extract notes with timing
- [ ] Color-code notes by pitch/velocity

### 2.2 Pattern Parser Integration
- [ ] Expose pattern parsing to WASM
- [ ] Include cycle timing from pattern mini-notation
- [ ] Handle rests and subdivisions

### 2.3 Staff Notation (Stretch)
- [ ] VexFlow or similar for traditional notation
- [ ] Real-time update as code changes

---

## Phase 3: Properties Panel

### 3.1 Cursor Context API
- [ ] `get_context_at_cursor(input: &str, pos: usize) -> CursorContext`
- [ ] Return AST node type, parent context, editable properties

### 3.2 Property Editors
- [ ] **Envelope Editor**: Visual ADSR curve (attack, decay, sustain, release)
- [ ] **Waveform Picker**: Sine, saw, square, triangle with preview
- [ ] **Pattern Editor**: Step sequencer view for pattern mini-notation
- [ ] **Chord Wheel**: Circle of fifths / chord quality selector

### 3.3 Bidirectional Sync
- [ ] Editing in panel updates source code
- [ ] Source code changes update panel in real-time

---

## Phase 4: Web Audio Playback

### 4.1 WASM Audio Engine
- [ ] Pure Rust oscillator/envelope generation to WASM
- [ ] AudioWorklet integration for low-latency playback
- [ ] Pattern scheduling with Web Audio clock

### 4.2 Transport Controls
- [ ] Play/Pause/Stop functionality
- [ ] Tempo control (BPM slider) connected to engine
- [ ] Loop/cycle visualization

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
| MIDI visualization | Canvas 2D | 🔜 Next |
| Audio playback | Web Audio API + AudioWorklet | ⬜ Planned |
| Property editing | Solid.js (recommended) | ⬜ Planned |

---

## Completed This Session

1. Created `cadence-core` workspace crate with WASM support
2. Implemented `tokenize()` and `parse_and_check()` WASM exports
3. Built web editor with CodeMirror 6 + custom Cadence language mode
4. Added Comment token to lexer for syntax highlighting
5. Fixed token positions (span captured after whitespace skip)
6. Real-time validation with Rust parser
7. Dark theme with music production colors

---

## Next Steps

1. **Script Execution in Editor** — Use Interpreter from WASM to run .cadence files
2. **InterpreterAction Handling** — Route Play/Stop/Tempo actions to Web Audio
3. **Piano Roll** — Parse patterns and visualize as simple grid
4. **Web Audio** — Basic oscillator playback from WASM
