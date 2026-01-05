If you get a compile error, try:
```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" wasm-pack build --target web --features wasm
```

or release build:
```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" wasm-pack build --release --target web --features wasm
```

To build and copy to editor:
```bash
cd cadence-core && wasm-pack build --target web --features wasm 2>&1 && cp pkg/cadence_core.js pkg/cadence_core_bg.wasm pkg/cadence_core.d.ts ../editor/src/wasm/
```