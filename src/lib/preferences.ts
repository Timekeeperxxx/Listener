import type { AppSettings } from '../types'
export const defaultSettings = (): AppSettings => ({
  volume: 0.7, animatedBackground: true, lyricOffset: 0, audioQuality: 'auto',
  lyrics: { fontScale: 100, lineHeight: 1.5, showTranslation: true, showRomanization: false },
  smartMix: false, preloadNext: true, closeToTray: true, cacheLimitMb: 64,
  shortcuts: { enabled: true, toggle: 'Control+Alt+Space', next: 'Control+Alt+ArrowRight', previous: 'Control+Alt+ArrowLeft', main: 'Control+Alt+KeyM' },
})
