If you get a compile error, try:
```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" wasm-pack build --target web --features wasm
```

or release build:
```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" wasm-pack build --release --target web --features wasm
```