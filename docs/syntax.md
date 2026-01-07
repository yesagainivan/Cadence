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

### Operators Reference

| Symbol | Name | Description | Example |
|--------|------|-------------|---------|
| ` ` (Space) | Sequence | Separates events, played in order | `"C E G"` → 3 events in 1 cycle |
| `[ ]` | Group | Subdivides a step into equal parts | `"C [E G] C"` → E & G share middle beat |
| `_` | Rest | Silence for one step | `"C _ G"` → Note, silence, note |
| `*N` | Repeat | Repeat step N times | `"C*4"` → `C C C C` |
| `,` | Chord | Play notes simultaneously | `"[C,E,G]"` → C+E+G chord |
| `<>` | Alternation | Cycle through elements on each loop | `"<C D E>"` → C on loop 1, D on loop 2, E on loop 3 |
| `(n,k)` | Euclidean | Distribute n pulses across k steps | `"C(3,8)"` → 3 C notes evenly in 8 slots |
| `{}` | Polyrhythm | Overlay patterns at different tempos | `"{C D E, F G}"` → 3-step + 2-step simultaneously |
| `(vel)` | Velocity | Set MIDI velocity | `"C5(100)"` → velocity 100; `"C5(0.5)"` → half velocity |
| `@N` | Weighted | Step takes N units of duration | `"C@2 D"` → C gets 2/3, D gets 1/3 of time |

### Basic Examples
```cadence
"C E G B"       // 4 notes, 1 beat each (in 4/4)
"C [E G] B"     // C(1 beat), E(0.5), G(0.5), B(1 beat)
"C*2 G*2"       // C C G G
"C _ G _"       // Note, Rest, Note, Rest
```

### Euclidean Rhythms
Euclidean rhythms distribute pulses as evenly as possible using the Bjorklund algorithm. Common patterns:

| Pattern | Rhythm | Description |
|---------|--------|-------------|
| `C(3,8)` | `x . . x . . x .` | Cuban tresillo |
| `C(5,8)` | `x . x x . x x .` | Cinquillo |
| `C(4,12)` | `x . . x . . x . . x . .` | 12/8 bell pattern |

### Alternation
The alternation operator `<>` cycles through its elements on each pattern loop:
```cadence
"<C D E> G"     // Loop 1: C G, Loop 2: D G, Loop 3: E G, Loop 4: C G, ...
"<[C,E] [D,F]>" // Alternates between C minor and D minor chords
```

### Polyrhythms
Polyrhythms overlay multiple patterns, each playing at its own tempo within the same cycle:
```cadence
"{C D E, F G}"     // 3-note pattern plays 3 notes/cycle, 2-note plays 2 notes/cycle
"{kick _ _ _, snare _ snare _}"  // Cross-rhythm drum pattern
```

### Velocity
Control MIDI velocity (note loudness) with parentheses after a note:
```cadence
"C(127) D(64) E(32)"   // Loud, medium, quiet (0-127 scale)
"C(1.0) D(0.5) E(0.25)" // Same using 0.0-1.0 float scale
```

### Weighted Steps
Use `@N` to give a step more time relative to others:
```cadence
"C@2 D"        // C plays for 2/3 of cycle, D for 1/3
"C@3 D@1 E@2"  // C: 3/6, D: 1/6, E: 2/6 of cycle
```

### Drum Sounds
Use drum names directly in patterns. All drums support multiple aliases:

| Drum | Aliases | MIDI Note |
|------|---------|-----------|
| Kick | `kick`, `k`, `bd`, `bass` | 36 |
| Snare | `snare`, `s`, `sn`, `sd` | 38 |
| Hi-Hat | `hihat`, `hh`, `h`, `ch` | 42 |
| Open Hi-Hat | `openhat`, `oh`, `ho` | 46 |
| Clap | `clap`, `cp`, `cl` | 39 |
| Tom | `tom`, `t`, `lt` | 45 |
| Crash | `crash`, `cr`, `cc` | 49 |
| Ride | `ride`, `rd`, `ri` | 51 |
| Rim | `rim`, `rm`, `rs` | 37 |
| Cowbell | `cowbell`, `cb`, `cow` | 56 |

```cadence
"kick snare hh hh"           // Basic 4/4 beat
"kick(3,8) snare@2 hh*4"     // Euclidean kick, long snare, fast hats
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
