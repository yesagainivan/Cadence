import { defineConfig } from 'vite';

export default defineConfig({
    // Enable top-level await for WASM
    optimizeDeps: {
        exclude: ['./src/wasm/cadence_core_bg.wasm'],
    },
    build: {
        target: 'esnext',
    },
    // Serve WASM files with correct MIME type
    server: {
        headers: {
            'Cross-Origin-Opener-Policy': 'same-origin',
            'Cross-Origin-Embedder-Policy': 'require-corp',
        },
    },
});
