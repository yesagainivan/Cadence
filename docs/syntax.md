# Cadence Language Syntax

Cadence is a coding environment for musical patterns and algorave performances. It combines a clean, declarative syntax for define musical structures with a reactive playback engine.

## Basics

### Comments
```cadence
// This is a one-line comment
```

### Data Types
- **Notes**: `C`, `D#4`, `Ab5`. Default octave is 4 if unspecified.
- **Chords**: `[C, E, G]`, `[A3, C4, E4]`. Comma-separated notes in brackets.
- **Patterns**: `"C E G"`, `"C [E G] *"`. String literals representing rhythmic sequences.
- **Numbers**: `120`, `0.5`, `-12`. Integers and floats.
- **Booleans**: `true`, `false`.
- **Strings**: `"path/to/file.cadence"`.

## Variables
Define variables using `let`. They are mutable by default.
```cadence
let key = [C, E, G]
let melody = "C D E F"
key = [G, B, D] // Reassignment
```

## Audio & Playback

### Basic Commands
```cadence
tempo 120       // Set global tempo (BPM)
volume 80       // Set global volume (0-100)
stop            // Stop all audio
```

### Playback
The `play` command starts playback on the current track (default 1).
```cadence
play [C, E, G]          // Play a single chord/note immediately
play "C E G" loop       // Loop a pattern indefinitely
play "C E G" queue loop // Seamlessly switch at next boundary
```

### Modifiers
- `loop`: Repeat the playback indefinitely.
- `queue`: Wait for the next beat/bar boundary before switching (prevents overlapping).
- `duration`: Set explicit duration in beats.

```cadence
play "C E G" queue loop // Seamlessly switch at next boundary
play "C E G" queue bar // Seamlessly switch at next bar
// play "C E G" queue 4 // Seamlessly switch at next 4 beats
```

### Tracks
Organize instruments or layers on separate tracks (ID 1+).
```cadence
track 1 {
    play "C E G" loop
}
track 2 {
    play "C G" loop
}

// 'on' is an alias for 'track'
on 3 play "kick snare" loop
```

## Pattern Mini-Notation
Strings like `"C E G"` are interpreted as rhythmic patterns, inspired by TidalCycles.
A pattern defines what happens in **one cycle** (default 4 beats).

| Symbol | Description | Example |
|--------|-------------|---------|
| `Space` | Sequentially separates events | `"C E G"` (3 events in 1 cycle) |
| `[ ]` | **Group**: Subdivides a step | `"C [E G] C"` (E & G share the middle beat) |
| `_` | **Rest**: Silence | `"C _ G"` (Rest in middle) |
| `*` | **Repeat**: Repeat step N times | `"C*4"` (Same as `"C C C C"`) |
| `,` | **Chord**: Notes in a group | `"C [E,G] C"` (Plays chord E+G in middle) |

**Examples**:
```cadence
"C E G B"       // 4 notes, 1 beat each (in 4/4)
"C [E G] B"     // C(1), E(0.5), G(0.5), B(1)
"C*2 G*2"       // C C G G
"C _ G _"       // Note, Rest, Note, Rest
```

## Functions & Methods

### Method Chaining
Transform patterns and chords using dot syntax.

**Pattern Methods**:
- `.fast(n)`: Speed up by factor `n`.
- `.slow(n)`: Slow down by factor `n`.
- `.rev()`: Reverse the pattern.
- `.transpose(n)`: Shift pitch by `n` semitones.
- `.wave("waveform")`: Set oscillator waveform (`sine`, `saw`, `square`, `triangle`). 
- `.env("preset")`: Set envelope (`pluck`, `pad`, `perc`, `organ`).
- `.optimize_voice_leading()`: Reorder chords for smooth transitions.

**Chord Methods**:
- `.invert()`: Invert the chord (C-E-G -> E-G-C).

### Built-in Functions
- `invert(chord)`: Returns inverted chord.
- `smooth_voice_leading(pattern)`: Returns pattern with optimized voice leading.
- `progression(name, key)`: Generate common chord progressions.
  - `ii_V_I(key)`
  - `I_IV_V(key)`
  - And many more...

### User-Defined Functions
Define your own reusable logic.
```cadence
fn jazz_comp(key, osc) {
    return ii_V_I(key).wave(osc).env("pad")
}

play jazz_comp(C, "saw") loop
```

## Control Flow
Standard procedural control flow.

```cadence
// Loops
repeat 4 {
    play [C, E, G]
}

loop {
    play [C, E, G]
    play [F, A, C]
    break // Exit loop
}

// Conditionals
if condition {
    play [C, Major]
} else {
    play [C, Minor]
}
```

## File Management
Load and run other Cadence files.
```cadence
load "songs/verse.cadence"
```
