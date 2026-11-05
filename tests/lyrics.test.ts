import { describe, expect, it } from 'vitest'
import { encryptQrcHex } from '@applemusic-like-lyrics/lyric'
import { decodeLyric, parseLyrics } from '../src/lib/lyrics'
import type { RawLyrics } from '../src/types'

const raw = (values: Partial<RawLyrics>): RawLyrics => ({ word: '', line: '', translation: '', romanization: '', warnings: [], ...values })

describe('QQ lyric data boundary', () => {
  it('decrypts the QRC XML envelope and preserves actual word timing', () => {
    const xml = '<?xml version="1.0"?><QrcInfos><LyricInfo><Lyric_1 LyricContent="[1000,2000]听(1000,700)见(1700,1300)"/></LyricInfo></QrcInfos>'
    const parsed = parseLyrics(raw({ word: encryptQrcHex(xml) }), 10)
    expect(parsed.mode).toBe('word')
    expect(parsed.lines[0].words.map(word => [word.word, word.startTime, word.endTime])).toEqual([['听',1000,1700],['见',1700,3000]])
  })
  it('decodes UTF-8 base64 and attaches translation at the same timestamp', () => {
    const text = '[00:01.00]听见 &amp; 你\n[00:05.00]下一句'
    const parsed = parseLyrics(raw({ line: Buffer.from(text).toString('base64'), translation: '[00:01.00]Hear you' }), 12)
    expect(parsed.mode).toBe('line')
    expect(parsed.lines[0].words[0].word).toBe('听见 & 你')
    expect(parsed.lines[0].endTime).toBe(5000)
    expect(parsed.lines[1].endTime).toBe(12000)
    expect(parsed.lines[0].translatedLyric).toBe('Hear you')
  })
  it('keeps line lyrics when the optional word payload cannot be decoded and reports why', () => {
    const parsed = parseLyrics(raw({ word: 'a'.repeat(32), line: '[00:01.00]仍然可以听' }), 10)
    expect(parsed.mode).toBe('line')
    expect(parsed.warnings[0]).toContain('逐字歌词解析失败')
  })
  it('does not turn absent lyrics into invented word timestamps', () => {
    expect(parseLyrics(raw({}),10)).toMatchObject({ mode: 'none', lines: [] })
    expect(decodeLyric('')).toBe('')
  })
  it.each(['line', 'word'] as const)('removes QQ translation placeholders from %s lyrics without changing real slashes', kind => {
    const line = '[00:01.00]Woo woo\n[00:04.00]AC/DC\n[00:08.00]Next line'
    const word = '[1000,2000]Woo(1000,500) woo(1500,1500)\n[4000,2000]AC/(4000,500)DC(4500,1500)\n[8000,2000]Next(8000,500) line(8500,1500)'
    const parsed = parseLyrics(raw({ [kind]: kind === 'line' ? line : word,
      translation: '[00:01.00]  //  \n[00:04.00]真实 / 译文 // 保留\n[00:08.00]下一句',
    }), 12)
    expect(parsed.lines[0].translatedLyric).toBe('')
    expect(parsed.lines[0].words.map(word => word.word).join('')).toBe('Woo woo')
    expect(parsed.lines[1].words.map(word => word.word).join('')).toBe('AC/DC')
    expect(parsed.lines[1].translatedLyric).toBe('真实 / 译文 // 保留')
    expect(parsed.lines[2].translatedLyric).toBe('下一句')
  })
})
