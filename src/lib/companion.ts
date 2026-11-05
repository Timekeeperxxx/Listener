import { watch } from 'vue'
import { errorMessage } from './api'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Track } from '../types'
import type { usePlayerStore } from '../stores/player'
export interface CompanionState {
  current: Track | null
  playing: boolean
  time: number
  duration: number
  lyricTime: number
  quality: string
  loading: boolean
  words: { word: string; startTime: number; endTime: number }[]
  translation: string
  nextLine: string
}
export async function bindCompanions(player: ReturnType<typeof usePlayerStore>) {
  function publish() {
    let index = -1
    for (let i = player.lyricLines.length - 1; i >= 0; i--) if (player.lyricLines[i].startTime <= player.lyricTime) { index = i; break }
    const line = index >= 0 ? player.lyricLines[index] : null
    const next = index + 1 < player.lyricLines.length ? player.lyricLines[index + 1] : null
    const payload: CompanionState = { current: player.current, playing: player.playing, time: player.time, duration: player.duration,
      lyricTime: player.lyricTime, quality: player.quality, loading: player.sourceState === 'loading',
      words: line ? line.words.map(word => ({ word: word.word, startTime: word.startTime, endTime: word.endTime })) : [],
      translation: line ? line.translatedLyric : '', nextLine: next ? next.words.map(word => word.word).join('') : '' }
    void invoke('publish_companions', { payload }).catch(cause => { player.notice = `桌面窗口同步失败：${errorMessage(cause)}` })
  }
  const stop = watch([() => Math.floor(player.time / 200), () => player.current, () => player.playing, () => player.lyricLines, () => player.sourceState], publish)
  const unlisten = await listen<string>('companion-action', event => {
    if (event.payload === 'toggle') void player.toggle()
    else if (event.payload === 'next') void player.next()
    else if (event.payload === 'previous') void player.previous()
    else if (event.payload === 'ready') publish()
  })
  return () => { stop(); unlisten() }
}
