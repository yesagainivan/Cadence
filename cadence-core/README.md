# Cadence Core

Core library for the Cadence music programming language. Contains the parser, evaluator, pattern system, and WASM bindings.

## Features

- **Parser** — Lexer, AST, and evaluator for Cadence syntax
- **Pattern System** — TidalCycles-inspired mini-notation with Euclidean rhythms and polyrhythms
- **WASM Support** — Compile to WebAssembly for browser use

## Building

### Native (for CLI/REPL)

```bash
cargo build --release
```

### WASM (for Editor)

```bash
# Install wasm-pack if needed
cargo install wasm-pack

# Build WASM module
wasm-pack build --target web --features wasm --out-dir ../editor/src/wasm
```

## Crate Features

| Feature | Description |
|---------|-------------|
| `default` | Includes `colored` for terminal output |
| `wasm` | WASM bindings + serde serialization |
| `serde` | JSON serialization support |
| `colored` | Colored terminal output |

## Usage

```rust
use cadence_core::{parse_and_evaluate, Environment};

let env = Environment::new();
let result = parse_and_evaluate("\"C E G\" |> fast 2", &env)?;
```