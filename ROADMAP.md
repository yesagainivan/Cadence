# Cadence Roadmap

A production-ready music programming language for chord progressions and harmonic exploration, inspired by TidalCycles and Sonic Pi.

---

## Phase 0: Foundation Fixes *(Current)*

### 0.1 REPL Refactoring
- [ ] Extract command parsing into a `CommandRegistry` pattern
- [ ] Remove giant if-else chain in `repl.rs`
- [ ] Add structured command parsing with arguments

### 0.2 Test Health
- [ ] Fix 7 failing tests (chord display, roman numerals, etc.)
- [ ] Add integration tests for audio playback

### 0.3 Parser/AST Separation
- [ ] Create proper AST types (separate from evaluation)
- [ ] Add AST pretty-printing for debugging
- [ ] Prepare for control flow constructs

---

## Phase 1: Basic Scripting

### 1.1 File Loading
- [ ] Load and execute `.cadence` files
- [ ] REPL command: `load "path/to/file.cadence"`
- [ ] Sequential execution of statements

### 1.2 Variables & Bindings
- [ ] `let prog = ii_V_I(C)`
- [ ] Variable substitution in expressions
- [ ] Scope management

### 1.3 Comments & Formatting
- [ ] Single-line comments: `// comment`
- [ ] Multi-line comments: `/* comment */`
- [ ] Whitespace/newline handling

---

## Phase 2: Control Flow

### 2.1 Loops
- [ ] `repeat 4 { ... }` - fixed iterations
- [ ] `loop { ... }` - infinite (with break)
- [ ] `every n beats { ... }` - time-synced loops

### 2.2 Conditionals
- [ ] `if condition { ... } else { ... }`
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
| REPL | ⚠️ Needs refactoring |
| Parser/AST | ⚠️ Needs separation |
| Scripting | ❌ Not started |
| Live Coding | ❌ Not started |
