import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: { port: 1420, strictPort: true, proxy: {
    '/__qq': { target: 'https://u.y.qq.com', changeOrigin: true, rewrite: path => path.replace(/^\/__qq/, ''), headers: { referer: 'https://y.qq.com/', origin: 'https://y.qq.com', 'user-agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36' } },
  } },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'safari15.4', sourcemap: false },
})
