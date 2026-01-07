import { defineConfig } from 'vite';

export default defineConfig({
    // Base path is set via CLI: --base=/Cadence/ for GitHub Pages
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
