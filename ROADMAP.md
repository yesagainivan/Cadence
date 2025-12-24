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

## Phase 1: Basic Scripting *(In Progress)*

### 1.1 File Loading ✅
- [x] Implement `load "path/to/file.cadence"` in Interpreter
- [x] Read file contents, parse, and execute
- [x] Handle file errors gracefully

### 1.2 Variables & Bindings ✅
- [x] `let prog = ii_V_I(C)` - parsing implemented
- [x] Variable resolution in Evaluator (Environment integration)
- [ ] Variable updates: `prog = other_prog` *(re-assignment)*

### 1.3 Comments & Formatting
- [ ] Single-line comments: `// comment`
- [ ] Multi-line comments: `/* comment */`
- [x] Better whitespace/newline handling as statement separators
  - *Lexer now emits `Token::Newline` - multi-line files work naturally*

### 1.4 Lexer Improvements
- [x] Handle numbers > 127 (changed from i8 to i32 for tempo support)
- [x] Unified expression parsing in StatementParser (no string reconstruction hack)
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
| Lexer | ✅ 25 tokens + i32 numbers + newlines |
| StatementParser | ✅ Unified expression parsing |
| Environment | ✅ Scoped variable storage |
| Interpreter | ✅ Actions-based architecture |
| Variable Resolution | ✅ Environment-aware evaluation |
| File Loading | ✅ load "file.cadence" works |
| Script Audio | ✅ play/tempo/stop trigger audio |
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

1. **Comments** - Add `//` single-line and `/* */` multi-line comments
2. **Variable Re-assignment** - Implement `prog = other_prog` (currently only `let` binding)
3. **Error Line Info** - Improve error messages with line/column info