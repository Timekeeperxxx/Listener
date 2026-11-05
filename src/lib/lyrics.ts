import { decryptQrcHex, parseLrc, parseQrc, type LyricLine } from '@applemusic-like-lyrics/lyric'
import type { RawLyrics } from '../types'

export interface ParsedLyrics {
  lines: LyricLine[]
  mode: 'word' | 'line' | 'none'
  warnings: string[]
}

function entities(text: string): string {
  return text.replace(/&(?:amp|lt|gt|quot|apos|nbsp);|&#(?:x[0-9a-f]+|\d+);/gi, entity => {
    const named: Record<string, string> = { '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&apos;': "'", '&nbsp;': ' ' }
    if (entity in named) return named[entity]
    const hex = entity.startsWith('&#x')
    const code = Number.parseInt(entity.slice(hex ? 3 : 2, -1), hex ? 16 : 10)
    return code <= 0x10ffff ? String.fromCodePoint(code) : entity
  })
}

export function decodeLyric(raw: string): string {
  let text = raw.trim()
  if (!text) return ''
  if (/^[0-9a-f]+$/i.test(text) && text.length % 16 === 0) {
    if (text.length > 1024 * 1024) throw new Error('歌词数据过大，无法解析。')
    text = decryptQrcHex(text)
  } else if (!text.includes('[') && /^[A-Za-z0-9+/]+={0,2}$/.test(text) && text.length % 4 === 0) {
    text = new TextDecoder('utf-8', { fatal: true }).decode(Uint8Array.from(atob(text), c => c.charCodeAt(0)))
  }
  const xml = text.match(/LyricContent\s*=\s*"([\s\S]*?)"\s*\/?\s*>/)
  return entities(xml ? xml[1] : text).replace(/\r\n/g, '\n').replace(/^\uFEFF/, '').trim()
}

function parseText(text: string): LyricLine[] {
  return /\[\d+,\d+\]/.test(text) ? parseQrc(text) : parseLrc(text)
}

export function parseLyrics(raw: RawLyrics, duration: number): ParsedLyrics {
  const warnings = [...raw.warnings]
  let lines: LyricLine[] = []
  let wordText = ''
  if (raw.word) {
    try {
      wordText = decodeLyric(raw.word)
      lines = parseText(wordText)
    } catch (error) {
      warnings.push(`逐字歌词解析失败：${error instanceof Error ? error.message : String(error)}`)
    }
  }
  if (!lines.length && raw.line) lines = parseText(decodeLyric(raw.line))
  const mode = lines.some(line => line.words.length > 1 && line.words.some(word => word.startTime !== line.startTime)) ? 'word' : lines.length ? 'line' : 'none'

  // LRC only supplies line start times. Set line ends from the next timestamp;
  // never invent per-word timestamps when the provider did not supply them.
  if (mode === 'line') {
    lines = lines.map((line, index) => {
      const endTime = index + 1 < lines.length ? lines[index + 1].startTime : Math.max(line.startTime, duration * 1000)
      return { ...line, endTime, words: line.words.map(word => ({ ...word, endTime })) }
    })
  }
  for (const [key, source] of [['translatedLyric', raw.translation], ['romanLyric', raw.romanization]] as const) {
    if (!source) continue
    try {
      const auxiliary = parseText(decodeLyric(source))
      for (const line of lines) {
        const match = auxiliary.find(other => Math.abs(other.startTime - line.startTime) <= 80)
        if (match) {
          const text = match.words.map(word => word.word).join('')
          // QQ uses a standalone // for lines with no translation.
          line[key] = key === 'translatedLyric' && text.trim() === '//' ? '' : text
        }
      }
    } catch (error) {
      warnings.push(`辅助歌词解析失败：${error instanceof Error ? error.message : String(error)}`)
    }
  }
  return { lines, mode, warnings }
}
