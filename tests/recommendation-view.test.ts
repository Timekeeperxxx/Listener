// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, reactive, type App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import type { Track } from '../src/types'
const api = vi.hoisted(() => ({ dailyRecommendation: vi.fn(), recommendationRadio: vi.fn(), searchPlaylists: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, desktop: false, errorMessage: (cause: Error) => cause.message }))
vi.mock('../src/stores/player', () => ({ usePlayerStore: () => player }))
import HomeView from '../src/components/HomeView.vue'
import { useLibraryStore } from '../src/stores/library'
import { recommendationDay } from '../src/stores/discovery'
const player = reactive({ current: null as Track | null, playing: false, toggle: vi.fn() })
const track = (mid: string): Track => ({ id: 1, mid, mediaMid: mid, title: mid, artist: 'Artist', album: 'Album', cover: '', duration: 180, vip: false })
const dailyTracks = [track('daily-1'), track('daily-2')], radioTracks = [track('radio-1'), track('radio-2')]
const onPlay = vi.fn(), onPlaylist = vi.fn()
let app: App
const flush = async () => { await Promise.resolve(); await nextTick(); await Promise.resolve(); await nextTick() }
beforeEach(async () => {
  vi.useFakeTimers(); vi.setSystemTime(new Date('2026-11-06T15:59:40Z')); vi.resetAllMocks()
  const pinia = createPinia(); setActivePinia(pinia)
  useLibraryStore().profile = { uin: 'account-a', nickname: 'A', avatar: '' }
  api.dailyRecommendation.mockImplementation(async () => ({ day: recommendationDay(), playlist: { id: '123456', name: '每日30首', count: dailyTracks.length, kind: 'public', cover: '' }, tracks: dailyTracks }))
  api.recommendationRadio.mockResolvedValue(radioTracks)
  const root = document.createElement('div'); document.body.append(root)
  app = createApp(HomeView, { navigation: 'recommend', category: 'songs', keyword: '', busy: false, onPlay, onPlaylist }); app.use(pinia); app.mount(root)
  await flush()
})
afterEach(() => { app.unmount(); document.body.replaceChildren(); vi.useRealTimers() })
describe('personal recommendation view', () => {
  it('uses personal endpoints and opens the actual daily playlist or plays its complete queue', async () => {
    expect(api.searchPlaylists).not.toHaveBeenCalled()
    document.querySelector<HTMLButtonElement>('[aria-label="查看每日30首"]')!.click(); await flush()
    expect(onPlaylist.mock.calls[0]?.[0].id).toBe('123456')
    document.querySelector<HTMLButtonElement>('[aria-label="播放每日30首"]')!.click(); await flush()
    expect(onPlay).toHaveBeenCalledWith(dailyTracks[0], dailyTracks)
  })
  it('selects a radio song on click and plays the full batch on double click', async () => {
    const row = document.querySelector<HTMLElement>('.track-row')!
    row.click(); await flush()
    expect(row.getAttribute('aria-selected')).toBe('true'); expect(onPlay).not.toHaveBeenCalled()
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); await flush()
    expect(onPlay).toHaveBeenCalledWith(radioTracks[0], radioTracks)
  })
  it('refreshes only the radio batch when choosing another batch', async () => {
    api.recommendationRadio.mockResolvedValueOnce([track('new-radio')])
    const refresh = [...document.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent?.includes('换一批'))!
    refresh.click(); await flush()
    expect(document.querySelector('.track-title')?.textContent).toContain('new-radio')
    expect(api.recommendationRadio).toHaveBeenCalledTimes(2); expect(api.dailyRecommendation).toHaveBeenCalledTimes(1)
  })
  it('refreshes daily recommendations across Shanghai midnight and removes its clock on unmount', async () => {
    await vi.advanceTimersByTimeAsync(60_000); await flush()
    expect(api.dailyRecommendation).toHaveBeenCalledTimes(2)
    expect(api.recommendationRadio).toHaveBeenCalledTimes(1)
    expect(vi.getTimerCount()).toBe(1)
    app.unmount(); expect(vi.getTimerCount()).toBe(0)
  })
})
