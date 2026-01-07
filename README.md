# Cadence

A live-coding language for composing musical patterns. Cadence uses a TidalCycles-inspired mini-notation for rhythms and sequences, with a native Rust core and WebAssembly-powered browser editor.

**[Try the Live Editor →](https://ivanowono.github.io/Cadence/)**

## Quick Example

```cadence
// Define a chord progression
let chords = "[C, E, G] [F, A, C] [G, B, D]"

// Play it with transformations
play chords.slow(2).wave("saw") loop

// Layer a melody
on 2 play "C5 E5 G5 C6".fast(2) loop
```

## Features

- **Pattern Mini-Notation** — TidalCycles-inspired syntax for rhythmic sequences
- **Live Coding** — Real-time playback with queue and loop modifiers
- **Euclidean Rhythms** — `C(3,8)` distributes 3 hits across 8 steps
- **Polyrhythms** — `{C D E, F G}` plays patterns simultaneously
- **WebMIDI** — Send notes to external synths and DAWs
- **Functional Transforms** — Chain with `.`: `fast`, `slow`, `transpose`, `rev`

## Project Structure

```
cadence/
├── cadence-core/    # Core language & WASM bindings
├── editor/          # Browser-based editor (Vite + TypeScript)
├── src/             # Native CLI + REPL with audio
├── examples/        # Example .cadence files
└── docs/            # Documentation
```

## Getting Started

### Browser Editor

Visit the [live editor](https://ivanowono.github.io/Cadence/) — no install required.

### CLI (Native)

```bash
# Build
cargo build --release

# Run REPL
cargo run

# Play a file
cargo run -- examples/demo.cadence
```

### Development

```bash
# Build WASM for editor
cd cadence-core
wasm-pack build --target web --features wasm --out-dir ../editor/src/wasm

# Run editor dev server
cd editor
npm install
npm run dev
```

## Syntax Overview

| Syntax | Description |
|--------|-------------|
| `C E G` | Sequence — play notes in order, evenly spaced |
| `[C E]` | Group — subdivide a step into equal parts |
| `[C, E, G]` | Chord — play notes simultaneously |
| `C*4` | Repeat — repeat step N times (`C C C C`) |
| `_` | Rest — silence for one step |
| `<C D E>` | Alternate — play one element per cycle (C, then D, then E on loop) |
| `C(3,8)` | Euclidean — distribute 3 pulses evenly across 8 steps |
| `{A B, C D E}` | Polyrhythm — overlay patterns at their own tempos |
| `C5(100)` | Velocity — set MIDI velocity (0–127 or 0.0–1.0) |
| `C@2` | Weighted — step takes 2 units of duration |
| `kick snare hh` | Drums — use drum names directly in patterns |

See [docs/syntax.md](docs/syntax.md) for the full reference.

## License

MIT
