import { defineConfig, mergeConfig } from 'vite'
import { resolve } from 'node:path'
import base from './vite.config'

export default mergeConfig(base, defineConfig({
  build: { outDir: 'demo-dist', rollupOptions: { input: resolve(import.meta.dirname, 'demo.html') } },
}))
