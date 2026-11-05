import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  // Web Worker の中でも import を使えるようにする（WASM を読み込むため）
  worker: { format: 'es' },
  // Three.js が大きいため、警告の基準を少し上げる
  build: { chunkSizeWarningLimit: 1000 },
})
