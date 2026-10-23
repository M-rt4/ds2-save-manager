import { resolve } from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The UI is a plain Vite + React app; Tauri loads it from dist/ (or the dev server).
export default defineConfig({
  root: resolve('src'),
  publicDir: resolve('src/public'),
  plugins: [react()],
  resolve: { alias: { '@': resolve('src') } },
  clearScreen: false,
  server: { port: 5174, strictPort: true },
  build: {
    outDir: resolve('dist'),
    emptyOutDir: true,
    target: 'es2022'
  }
});
