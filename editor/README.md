# Cadence Editor

Browser-based editor for live-coding with Cadence. Built with Vite, TypeScript, and CodeMirror.

**[Try it Live →](https://ivanowono.github.io/Cadence/)**

## Features

- **CodeMirror 6** — Syntax highlighting and editing
- **Web Audio** — Real-time audio synthesis
- **WebMIDI** — Send notes to external synths and DAWs
- **WASM** — Runs cadence-core in the browser

## Development

```bash
# Install dependencies
npm install

# Start dev server
npm run dev

# Build for production
npm run build
```

## Rebuilding WASM

When you make changes to `cadence-core`, rebuild the WASM module:

```bash
cd ../cadence-core
wasm-pack build --target web --features wasm --out-dir ../editor/src/wasm
```

## Project Structure

```
editor/
├── src/
│   ├── main.ts           # Entry point
│   ├── audio-engine.ts   # Web Audio synthesis
│   ├── midi-output.ts    # WebMIDI integration
│   ├── codemirror.ts     # Editor setup
│   ├── lang-cadence.ts   # Syntax highlighting
│   └── wasm/             # WASM binaries (gitignored)
├── index.html
└── vite.config.ts
```

See [ROADMAP.md](ROADMAP.md) for planned features.
