# Cadence Roadmap

A production-ready music programming language for chord progressions and harmonic exploration, inspired by TidalCycles and Sonic Pi.

---

## Phase 0: Foundation Fixes ✅ *Complete*

### 0.1 REPL Refactoring ✅
- [x] Extract command parsing into a `CommandRegistry` pattern
- [x] Remove giant if-else chain in `repl.rs` (598 → 207 lines)
- [x] Add structured command parsing with arguments

### 0.2 Test Health ✅
- [x] Fix 7 failing tests (chord display, roman numerals, etc.)
- [x] All 157 tests passing

### 0.3 Parser/AST Separation ✅
- [x] Create proper AST types (`Statement`, `Program` in `ast.rs`)
- [x] Extended Lexer with 24 new tokens (keywords, braces, operators)
- [x] Created `StatementParser` for scripting constructs
- [x] Added `Environment` for scoped variable storage
- [x] Created `Interpreter` for statement execution
- [x] Integrated new Interpreter with REPL

---

## Phase 1: Basic Scripting *(Next)*

### 1.1 File Loading
- [ ] Implement `load "path/to/file.cadence"` in Interpreter
- [ ] Read file contents, parse, and execute
- [ ] Handle file errors gracefully

### 1.2 Variables & Bindings *(Partially Done)*
- [x] `let prog = ii_V_I(C)` - parsing implemented
- [ ] Variable resolution in Evaluator (Environment integration)
- [ ] Variable updates: `prog = other_prog`

### 1.3 Comments & Formatting
- [ ] Single-line comments: `// comment`
- [ ] Multi-line comments: `/* comment */`
- [ ] Better whitespace/newline handling as statement separators

### 1.4 Lexer Improvements
- [ ] Handle numbers > 127 (currently limited to i8, affects tempo values)
- [ ] Improve error messages with line/column info

---

## Phase 2: Control Flow

### 2.1 Loops
- [x] `repeat 4 { ... }` - fixed iterations (parsing done)
- [x] `loop { ... }` - infinite with break (parsing done)
- [ ] `every n beats { ... }` - time-synced loops

### 2.2 Conditionals
- [x] `if condition { ... } else { ... }` (parsing done)
- [ ] Pattern matching on chord qualities

### 2.3 Functions
- [ ] User-defined functions: `fn my_pattern(key) { ... }`
- [ ] Higher-order functions (map, filter)
- [ ] Closures for pattern capture

---

## Phase 3: Live Coding (TidalCycles-inspired)

### 3.1 Live Reload
- [ ] File watch with hot-reload
- [ ] Changes apply at next beat/bar boundary
- [ ] Error recovery without stopping playback

### 3.2 Pattern System
- [ ] Cycle-based patterns: `"C E G _"` (underscore = rest)
- [ ] Pattern operators: `fast`, `slow`, `rev`, `every`
- [ ] Mini-notation parser

### 3.3 Multiple Voices
- [ ] Named tracks/voices
- [ ] Parallel execution
- [ ] Per-voice volume/effects

---

## Phase 4: Production Features

### 4.1 Audio Enhancements
- [ ] ADSR envelopes (configurable attack/decay/sustain/release)
- [ ] Multiple waveforms (saw, square, triangle)
- [ ] Basic effects (reverb, delay, filter)

### 4.2 MIDI Output
- [ ] MIDI device enumeration
- [ ] Note-on/note-off events
- [ ] Control change messages

### 4.3 Recording & Export
- [ ] Record session to WAV
- [ ] Export patterns to MIDI files

### 4.4 OSC Support
- [ ] OSC input for external control
- [ ] OSC output for visualization

---

## Architecture Principles

1. **Beat-quantized everything** - All changes sync to musical time
2. **Never stop on error** - Graceful degradation during live coding
3. **Minimal latency** - Audio thread isolation, lock-free where possible
4. **Composable patterns** - Everything is a pattern that can be transformed

---

## Current Status

| Component | Status |
|-----------|--------|
| Core Types (Note, Chord, Progression) | ✅ Stable |
| Audio Engine (crossfade, beat-sync) | ✅ Production-ready |
| Scheduler (beat tracking) | ✅ Complete |
| REPL | ✅ Refactored with CommandRegistry |
| Parser/AST | ✅ Separated, scripting-ready |
| Lexer | ✅ 24 new tokens for scripting |
| StatementParser | ✅ Parses let, play, if, loop, etc. |
| Environment | ✅ Scoped variable storage |
| Interpreter | ✅ Statement execution |
| File Loading | ⚠️ Parsing done, execution pending |
| Live Coding | ❌ Not started |

---

## Key Files Added (Phase 0.3)

| File | Purpose |
|------|---------|
| `src/parser/statement_parser.rs` | Parses scripting statements |
| `src/parser/environment.rs` | Scoped variable storage |
| `src/parser/interpreter.rs` | Statement execution |
| `src/commands/` | Command registry pattern for REPL |

## Next Session Priorities

1. **File Loading** - Implement `load "file.cadence"` execution
2. **Variable Resolution** - Wire Environment into Evaluator
3. **Tempo fix** - Handle numbers > 127 in Lexer (i16 or Float)