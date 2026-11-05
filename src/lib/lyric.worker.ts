import { parseLyrics } from './lyrics'
import type { RawLyrics } from '../types'

self.onmessage = (event: MessageEvent<{ raw: RawLyrics; duration: number }>) => {
  try {
    self.postMessage({ ok: true, value: parseLyrics(event.data.raw, event.data.duration) })
  } catch (error) {
    self.postMessage({ ok: false, message: error instanceof Error ? error.message : String(error) })
  }
}
