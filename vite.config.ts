import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  css: { postcss: { plugins: [{
    postcssPlugin: 'listener-pointer-hover',
    Once(root, { AtRule }) {
      const file = root.source?.input.file
      if (!file?.match(/\/(?:src\/(?:style|redesign)\.css|@applemusic-like-lyrics\/core\/dist\/style\.css)$/)) return
      // WebKit can retain :hover after a tap. Keep keyboard and selected rules
      // outside this media query, even when they share a rule with :hover.
      const changes: (() => void)[] = []
      root.walkRules(/:hover\b/, rule => {
        const selectors = rule.selectors
        const hover = selectors.filter(selector => /:hover\b/.test(selector))
        const other = selectors.filter(selector => !/:hover\b/.test(selector))
        changes.push(() => {
          const media = new AtRule({ name: 'media', params: '(hover: hover)' })
          if (other.length) {
            rule.selectors = other
            rule.after(media)
            media.append(rule.clone({ selector: hover.join(', ') }))
          } else {
            rule.before(media)
            media.append(rule)
          }
        })
      })
      changes.forEach(apply => apply())
    },
  }] } },
  optimizeDeps: { entries: ['index.html', 'demo.html'] },
  worker: { format: 'es' },
  server: { port: 1420, strictPort: true, proxy: {
    '/__qq': { target: 'https://u.y.qq.com', changeOrigin: true, rewrite: path => path.replace(/^\/__qq/, ''), headers: { referer: 'https://y.qq.com/', origin: 'https://y.qq.com', 'user-agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36' } },
  } },
  clearScreen: false,
  build: { target: 'safari15.4', sourcemap: false },
})
