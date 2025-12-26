# Cadence Editor Roadmap

A web-based editor for the Cadence music programming language with live syntax highlighting, MIDI visualization, and contextual property editing.

---

## Phase 0: WASM Foundation ✅ *In Progress*

### 0.1 Type Extraction ✅
- [x] Create `types/audio_config.rs` with WASM-compatible types
- [x] Move `Waveform`, `AdsrParams`, `QueueMode` from audio module
- [x] Update all imports across codebase
- [x] All 232 tests passing

### 0.2 Crate Split (Next)
- [ ] Create workspace with `cadence-core` crate
- [ ] Move types + parser modules to core crate
- [ ] Add `serde` feature for JSON serialization
- [ ] Test `wasm32-unknown-unknown` compilation target

**Modules for cadence-core:**
- `types/` (note, chord, pattern, audio_config, roman_numeral, voice_leading)
- `parser/` (lexer, ast, evaluator, statement_parser, environment)

**Stays in main crate:**
- `audio/` (audio.rs, midi.rs, playback_engine, clock) — requires cpal/midir
- `repl/` — terminal-specific
- `commands/` — CLI-specific

---

## Phase 1: Syntax Highlighting

### 1.1 Tokenization API
- [ ] Create `tokenize_for_highlighting(input: &str) -> Vec<HighlightSpan>` 
- [ ] Map Token types to highlight classes (keyword, note, chord, operator, etc.)
- [ ] Handle partial/incomplete input gracefully

### 1.2 CodeMirror 6 Integration
- [ ] Create `editor/` folder with Vite + TypeScript setup
- [ ] WASM bindings via `wasm-bindgen`
- [ ] Custom CodeMirror language mode using WASM tokenizer
- [ ] Real-time highlighting as user types

---

## Phase 2: Live MIDI Display

### 2.1 Parser Integration
- [ ] Expose `parse_to_events(input: &str) -> Vec<MidiEvent>` from WASM
- [ ] Include timing, pitch, duration, velocity
- [ ] Handle patterns with cycle timing

### 2.2 Piano Roll Visualization  
- [ ] Canvas-based piano roll component
- [ ] Sync playhead with parsed events
- [ ] Color-code by track/voice

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
- [ ] Play/Pause/Stop
- [ ] Tempo control (BPM slider)
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
│              │  │ highlighting   │ │                  │
│              │  └────────────────┘ │                  │
│              └─────────────────────┘                  │
└──────────────────────────────────────────────────────┘
```

---

## Key Technologies

| Component | Technology |
|-----------|------------|
| Editor | CodeMirror 6 |
| WASM bindings | wasm-bindgen, wasm-pack |
| Build tool | Vite |
| MIDI visualization | Canvas 2D / WebGL |
| Audio playback | Web Audio API + AudioWorklet |
| Property editing | Custom reactive UI (Solid.js or Vue) |

---

## Next Steps

1. **Create workspace** — Add `Cargo.toml` workspace config
2. **Extract cadence-core** — Move parser/types to new crate
3. **Add serde derives** — Enable JSON serialization for JS interop
4. **Test WASM build** — Verify `cargo build --target wasm32-unknown-unknown`
5. **Create editor scaffold** — Vite + CodeMirror 6 skeleton
