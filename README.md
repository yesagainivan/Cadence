# Cadence

A live-coding environment for musical patterns and algorave performances, written in Rust with a WebAssembly browser editor.

**[Try the Live Editor →](https://ivanowono.github.io/Cadence/)**

## Quick Example

```cadence
// Define a chord progression
let chords = "[C, E, G] [F, A, C] [G, B, D]"

// Play it with transformations
play chords |> slow 2 |> wave "saw" loop

// Layer a melody
on 2 play "C5 E5 G5 C6" |> fast 2 loop
```

## Features

- **Pattern Mini-Notation** — TidalCycles-inspired syntax for rhythmic sequences
- **Live Coding** — Real-time playback with queue and loop modifiers
- **Euclidean Rhythms** — `C(3,8)` distributes 3 hits across 8 steps
- **Polyrhythms** — `{C D E, F G}` plays patterns simultaneously
- **WebMIDI** — Send notes to external synths and DAWs
- **Functional Transforms** — Chain with `|>`: `fast`, `slow`, `transpose`, `rev`

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
| `C E G` | Sequence — notes in order |
| `[C E]` | Group — subdivide a step |
| `[C, E, G]` | Chord — play together |
| `C*4` | Repeat — `C C C C` |
| `_` | Rest — silence |
| `<C D E>` | Alternate — cycle through |
| `C(3,8)` | Euclidean — 3 pulses in 8 steps |
| `{A B, C D E}` | Polyrhythm — overlay patterns |

See [docs/syntax.md](docs/syntax.md) for the full reference.

## License

MIT
