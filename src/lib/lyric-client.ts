import type { ParsedLyrics } from './lyrics'
import type { RawLyrics } from '../types'

export function parseInWorker(raw: RawLyrics, duration: number): Promise<ParsedLyrics> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL('./lyric.worker.ts', import.meta.url), { type: 'module' })
    worker.onmessage = event => {
      worker.terminate()
      if (event.data.ok) resolve(event.data.value)
      else reject(new Error(event.data.message))
    }
    worker.onerror = () => {
      worker.terminate()
      reject(new Error('歌词解析进程启动失败，请重新加载歌词。'))
    }
    worker.postMessage({ raw, duration })
  })
}
