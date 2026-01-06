If you get a compile error, try:
```bash
wasm-pack build --target web --features wasm --out-dir ../editor/src/wasm
```

or release build:
```bash
wasm-pack build --release --target web --features wasm --out-dir ../editor/src/wasm
```

To build and copy to editor:
```bash
cd cadence-core && wasm-pack build --target web --out-dir ../editor/src/wasm --features wasm
```