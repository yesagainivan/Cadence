# Cadence Music Programming Language - Development Plan

## Project Overview
A domain-specific programming language for harmonic relationships and melodic manipulation, built in Rust, targeting composers who code.

## Phase 1: Core Foundation (MVP)

### 1.1 Basic Data Types & Parsing
**Goal**: Handle notes, chords, and basic syntax
```rust
// Core types
Note: C, D#, Bb (chromatic representation internally as 0-11)
Chord: [C, E, G] (array of notes)
Progression: [[C, E, G], [F, A, C], [G, B, D]]
```

**Implementation**:
- Lexer/Parser using `pest` or `nom`
- Internal representation: notes as integers (0-11 chromatic)
- Support for sharps/flats, enharmonic equivalents
- Basic error handling for invalid notes

### 1.2 Arithmetic Operations
**Goal**: Transpose and manipulate chords mathematically
```rust
[C, E, G] + 2    // transpose up 2 semitones -> [D, F#, A]
[C, E, G] - 5    // transpose down 5 semitones -> [G, B, D]
```

**Implementation**:
- Overload `+`, `-` operators for Note and Chord types
- Handle octave wrapping and enharmonic spelling
- Validate operations (prevent invalid intervals)

### 1.3 REPL Infrastructure
**Goal**: Interactive environment for experimentation
```rust
cadence> [C, E, G]
C Major: [C, E, G]

cadence> [C, E, G] + 7
G Major: [G, B, D]
```

**Implementation**:
- Basic REPL loop with `rustyline` for input handling
- Pretty printing for chords (show both notes and chord names when possible)
- History and basic editing capabilities

## Phase 2: Harmonic Operations

### 2.1 Set Operations
**Goal**: Analyze harmonic relationships
```rust
[C, E, G] & [A, C, E]    // intersection: [C, E] (common tones)
[C, E, G] | [F, A, C]    // union: [C, E, F, G, A] (extended harmony)
[C, E, G] ^ [A, C, E]    // difference: [G] (non-common tones)
```

### 2.2 Chord Transformations
**Goal**: Musical transformations as functions
```rust
invert([C, E, G])        // -> [E, G, C] (first inversion)
invert([C, E, G], 2)     // -> [G, C, E] (second inversion)
retrograde([C, D, E, F]) // -> [F, E, D, C] (melodic reversal)
```

### 2.3 Progression Operations
**Goal**: Work with chord sequences
```rust
prog = [[C, E, G], [F, A, C], [G, B, D]]
prog + 5             // transpose entire progression
map(invert, prog)    // invert all chords
```

## Phase 3: Advanced Features

### 3.1 Voice Leading Analysis
```rust
voice_leading([C, E, G], [F, A, C])  // analyze movement between chords
smooth_voice_leading(prog)           // optimize voice leading in progression
```

### 3.2 Scale and Mode Support
```rust
C_major = scale(C, major)            // generate scale
modes(C_major)                       // all modes of C major
in_scale([C, E, G], C_major)        // check if chord fits scale
```

### 3.3 Harmonic Analysis
```rust
analyze([C, E, G])                   // -> "C Major triad"
roman_numeral([F, A, C], key=C)     // -> "IV" (four chord in C major)
```

## Phase 4: Output and Integration

### 4.1 Audio Playback
- Integration with `cpal` or `rodio` for basic audio output
- Simple synthesis (sine waves initially)
- Play chords and progressions

### 4.2 MIDI Export
- Generate MIDI files from chord progressions
- Integration with existing DAW workflows

### 4.3 Notation Export
- Basic MusicXML or LilyPond output
- Visual representation of harmonic relationships

## Technical Architecture

### Core Modules
```
src/
├── main.rs              // REPL entry point
├── lexer/
│   └── mod.rs           // Tokenization
├── parser/
│   └── mod.rs           // AST generation
├── types/
│   ├── note.rs          // Note type and operations
│   ├── chord.rs         // Chord type and operations
│   └── progression.rs   // Progression type
├── evaluator/
│   └── mod.rs           // Expression evaluation
├── operations/
│   ├── arithmetic.rs    // +, -, *, /
│   ├── set_ops.rs       // &, |, ^
│   └── transforms.rs    // invert, retrograde, etc.
└── repl/
    └── mod.rs           // Interactive shell
```

### Dependencies (Cargo.toml)
```toml
[dependencies]
pest = "2.7"           # Parsing
pest_derive = "2.7"    # Parser macros
rustyline = "12.0"     # REPL interface
anyhow = "1.0"         # Error handling
clap = "4.0"           # CLI arguments
```

## Development Milestones

### Week 1-2: Foundation
- [ ] Basic note and chord types
- [ ] Simple parser for `[C, E, G]` syntax
- [ ] Arithmetic operations (+, -)
- [ ] Basic REPL

### Week 3-4: Harmonic Operations
- [ ] Set operations (&, |, ^)
- [ ] Chord transformations (invert, etc.)
- [ ] Progression support

### Week 5-6: Polish & Testing
- [ ] Comprehensive error handling
- [ ] Unit tests for all operations
- [ ] Documentation and examples

### Month 2+: Advanced Features
- [ ] Audio playback
- [ ] MIDI/notation export
- [ ] Advanced harmonic analysis

## Success Criteria
1. Can express and manipulate chords programmatically
2. Intuitive for programmers familiar with music theory
3. Useful for exploring harmonic relationships
4. Extensible architecture for future features
5. Fast and responsive REPL experience

## Future Extensions
- Custom tuning systems and microtonal support
- Advanced voice leading algorithms
- Integration with music theory databases
- Visual harmonic relationship graphs
- Live coding performance features
