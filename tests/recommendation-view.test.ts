// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, reactive, type App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import type { HomeCategory, HomeNavigation, Track } from '../src/types'
const api = vi.hoisted(() => ({ dailyRecommendation: vi.fn(), recommendationRadio: vi.fn(), desktopSongRecommendation: vi.fn(), searchPlaylists: vi.fn(), recommendedPlaylists: vi.fn(), featuredCharts: vi.fn(), history: vi.fn(), mutatePlaylist: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, native: false, errorMessage: (cause: Error) => cause.message }))
vi.mock('../src/stores/player', () => ({ usePlayerStore: () => player }))
import HomeView from '../src/components/HomeView.vue'
import { useLibraryStore } from '../src/stores/library'
import { recommendationDay } from '../src/stores/discovery'
const player = reactive({ current: null as Track | null, playing: false, toggle: vi.fn() })
const track = (mid: string): Track => ({ id: 1, mid, mediaMid: mid, title: mid, artist: 'Artist', album: 'Album', cover: '', duration: 180, vip: false })
const dailyTracks = [track('daily-1'), track('daily-2')]
const radioTracks = Array.from({ length: 10 }, (_, i) => track(`radio-${i + 1}`))
const blindBoxTracks = Array.from({ length: 10 }, (_, i) => track(`blind-box-${i + 1}`))
const recommendedTracks = blindBoxTracks.flatMap((song, index) => [song, radioTracks[index]])
const recentTracks = Array.from({ length: 9 }, (_, i) => track(`recent-${i + 1}`))
const onPlay = vi.fn(), onPlaylist = vi.fn()
const view = reactive<{ navigation: HomeNavigation; category: HomeCategory }>({ navigation: 'recommend', category: 'songs' })
let app: App
const flush = async () => { await Promise.resolve(); await nextTick(); await Promise.resolve(); await nextTick() }
beforeEach(async () => {
  vi.useFakeTimers(); vi.setSystemTime(new Date('2026-11-06T15:59:40Z')); vi.resetAllMocks()
  const pinia = createPinia(); setActivePinia(pinia)
  const library = useLibraryStore()
  library.profile = { uin: 'account-a', nickname: 'A', avatar: '' }
  library.likedState = 'ready'
  library.libraryState = 'ready'
  view.navigation = 'recommend'
  view.category = 'songs'
  api.history.mockResolvedValue(recentTracks)
  api.dailyRecommendation.mockImplementation(async () => ({ day: recommendationDay(), playlist: { id: '123456', name: '每日30首', count: dailyTracks.length, kind: 'public', cover: '' }, tracks: dailyTracks }))
  api.recommendationRadio.mockResolvedValue(radioTracks)
  api.desktopSongRecommendation.mockResolvedValue({ title: '音乐盲盒', tracks: blindBoxTracks })
  api.recommendedPlaylists.mockResolvedValue([{ id: '234567', name: 'QQ 推荐歌单', cover: '', count: 30, kind: 'public' }])
  api.featuredCharts.mockResolvedValue([{ playlist: { id: 'chart:26', name: '热歌榜', cover: '', count: 100, kind: 'chart' }, preview: [{ title: '榜首歌曲', artist: 'Artist' }] }])
  const root = document.createElement('div'); document.body.append(root)
  app = createApp({ render: () => h(HomeView, { navigation: view.navigation, category: view.category, keyword: '', busy: false, onPlay, onPlaylist,
    'onUpdate:category': category => { view.category = category },
  }) }); app.use(pinia); app.mount(root)
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
  it('shows both sources together and plays from the combined queue', async () => {
    const rows = document.querySelectorAll<HTMLElement>('.recommendation-radio .track-row')
    expect([...rows].map(row => row.querySelector('.track-title')?.textContent?.trim())).toEqual(['blind-box-1', 'radio-1', 'blind-box-2', 'radio-2'])
    const row = rows[1]
    row.click(); await flush()
    expect(row.getAttribute('aria-selected')).toBe('true'); expect(onPlay).not.toHaveBeenCalled()
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); await flush()
    expect(onPlay).toHaveBeenCalledWith(radioTracks[0], recommendedTracks)
  })
  it('pages through combined recommendations while playing from the complete queue', async () => {
    document.querySelector<HTMLButtonElement>('[aria-label="下一组推荐歌曲"]')!.click(); await flush()
    const first = document.querySelector<HTMLElement>('.recommendation-radio .track-row')!
    expect(first.querySelector('.track-title')?.textContent).toContain('blind-box-3')
    first.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); await flush()
    expect(onPlay).toHaveBeenCalledWith(blindBoxTracks[2], recommendedTracks)
  })
  it('opens the complete history in its own favorites tab and plays in place', async () => {
    expect(document.querySelector('.listening-overview .recommendation-radio')).toBeTruthy()
    view.navigation = 'favorites'; await flush()
    expect(document.querySelector('.recent-tab')).toBeNull()
    expect(api.history).not.toHaveBeenCalled()
    const tab = [...document.querySelectorAll<HTMLButtonElement>('.favorite-tabs button')].find(button => button.textContent?.trim() === '最近播放')!
    tab.click(); await flush()
    expect(tab.getAttribute('aria-pressed')).toBe('true')
    const rows = document.querySelectorAll<HTMLElement>('.recent-tab .track-row')
    expect(rows).toHaveLength(recentTracks.length)
    rows[0].click(); await flush()
    expect(onPlay).not.toHaveBeenCalled()
    rows[0].dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); await flush()
    expect(onPlay).toHaveBeenCalledWith(recentTracks[0], recentTracks)
    expect(onPlaylist).not.toHaveBeenCalled()
    expect(useLibraryStore().view).toBe('library')
  })
  it('opens recommended playlists and charts by their distinct upstream IDs', async () => {
    document.querySelector<HTMLButtonElement>('[aria-label="查看歌单 QQ 推荐歌单"]')!.click()
    document.querySelector<HTMLButtonElement>('[aria-label="查看排行榜 热歌榜"]')!.click()
    expect(onPlaylist.mock.calls.map(([playlist]) => [playlist.id, playlist.kind])).toEqual([['234567', 'public'], ['chart:26', 'chart']])
    expect(document.querySelector('.recommended-charts')?.textContent).toContain('榜首歌曲')
  })
  it('collects and removes a recommended playlist without opening its details', async () => {
    api.mutatePlaylist.mockResolvedValueOnce({ playlists: [{ id: '234567', name: 'QQ 推荐歌单', count: 30, cover: '', kind: 'collected' }], warnings: [] })
    api.mutatePlaylist.mockResolvedValueOnce({ playlists: [], warnings: [] })
    const button = document.querySelector<HTMLButtonElement>('[aria-label="收藏歌单 QQ 推荐歌单"]')!
    button.click(); await flush()
    expect(api.mutatePlaylist).toHaveBeenNthCalledWith(1, 'collect', '234567', null, null)
    expect(button.getAttribute('aria-pressed')).toBe('true')
    button.click(); await flush()
    expect(api.mutatePlaylist).toHaveBeenNthCalledWith(2, 'uncollect', '234567', null, null)
    expect(button.getAttribute('aria-pressed')).toBe('false')
    expect(onPlaylist).not.toHaveBeenCalled()
  })
  it('refreshes both song sources and deduplicates tracks across them', async () => {
    const shared = track('shared-song'), radioOnly = track('new-radio')
    api.desktopSongRecommendation.mockResolvedValueOnce({ title: '音乐盲盒', tracks: [shared] })
    api.recommendationRadio.mockResolvedValueOnce([shared, radioOnly])
    const refresh = document.querySelector<HTMLButtonElement>('[aria-label="刷新为你推荐"]')!
    refresh.click(); await flush()
    expect(document.querySelectorAll('.recommendation-radio .track-row')).toHaveLength(2)
    document.querySelector<HTMLButtonElement>('[aria-label="播放为你推荐"]')!.click()
    expect(onPlay).toHaveBeenCalledWith(shared, [shared, radioOnly])
    expect(api.recommendationRadio).toHaveBeenCalledTimes(2)
    expect(api.desktopSongRecommendation).toHaveBeenCalledTimes(2)
    expect(api.dailyRecommendation).toHaveBeenCalledTimes(1)
  })
  it('refreshes daily recommendations across Shanghai midnight and removes its clock on unmount', async () => {
    await vi.advanceTimersByTimeAsync(60_000); await flush()
    expect(api.dailyRecommendation).toHaveBeenCalledTimes(2)
    expect(api.recommendationRadio).toHaveBeenCalledTimes(1)
    expect(vi.getTimerCount()).toBe(1)
    app.unmount(); expect(vi.getTimerCount()).toBe(0)
  })
})
