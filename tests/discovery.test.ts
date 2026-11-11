import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { CatalogPage, DailyRecommendation, Playlist, Track } from '../src/types'
const api = vi.hoisted(() => ({ catalogAlbums: vi.fn(), searchPlaylists: vi.fn(), dailyRecommendation: vi.fn(), recommendationRadio: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, errorMessage: (cause: Error) => cause.message }))
import { recommendationDay, useDiscoveryStore } from '../src/stores/discovery'
const playlist = (id: string): Playlist => ({ id, name: id, cover: '', kind: 'public', count: 10 })
const page = (id: string, nextOffset = 20, hasMore = false): CatalogPage<Playlist> => ({ items: [playlist(id)], total: 100, nextOffset, hasMore })
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done }); return { promise, resolve } }
describe('public discovery state', () => {
  beforeEach(() => { setActivePinia(createPinia()); vi.resetAllMocks() })
  it('uses the server pagination cursor and keeps loaded playlists when the next page fails', async () => {
    api.searchPlaylists.mockResolvedValueOnce(page('first', 40, true)).mockRejectedValueOnce(new Error('QQ 暂时不可用'))
    const store = useDiscoveryStore()
    await store.loadPlaylists('华语流行')
    await store.loadPlaylists('华语流行', true)
    expect(api.searchPlaylists).toHaveBeenLastCalledWith('华语流行', 40)
    expect(store.playlists.items.map(item => item.id)).toEqual(['first'])
    expect(store.playlistState).toBe('error')
    expect(store.playlistError).toBe('QQ 暂时不可用')
  })
  it('does not replace a newly chosen category with an older category response', async () => {
    const old = deferred<CatalogPage<Playlist>>()
    api.searchPlaylists.mockReturnValueOnce(old.promise).mockResolvedValueOnce(page('new'))
    const store = useDiscoveryStore(), pending = store.loadPlaylists('华语流行')
    await store.loadPlaylists('纯音乐')
    old.resolve(page('old')); await pending
    expect(store.playlistQuery).toBe('纯音乐')
    expect(store.playlists.items[0]?.id).toBe('new')
  })
  it('invalidates public selections on account reset', async () => {
    const pendingPage = deferred<CatalogPage<Playlist>>()
    api.searchPlaylists.mockReturnValueOnce(pendingPage.promise)
    const store = useDiscoveryStore(), pending = store.loadPlaylists()
    store.reset(); pendingPage.resolve(page('old')); await pending
    expect(store.playlists.items).toEqual([])
    expect(store.playlistState).toBe('idle')
  })
  it('reports album request failures instead of substituting collection data', async () => {
    api.catalogAlbums.mockRejectedValueOnce(new Error('新碟请求失败'))
    const store = useDiscoveryStore()
    await store.loadAlbums()
    expect(store.albums).toEqual([])
    expect(store.albumState).toBe('error')
    expect(store.albumError).toBe('新碟请求失败')
  })
})

const song = (mid: string): Track => ({ id: 1, mid, mediaMid: mid, title: mid, artist: 'Artist', album: 'Album', cover: '', duration: 120, vip: false })
const daily = (id: string, day = recommendationDay()): DailyRecommendation => ({ playlist: playlist(id), tracks: [song(id)], day })
describe('account recommendations', () => {
  beforeEach(() => { setActivePinia(createPinia()); vi.resetAllMocks() })
  it('loads the account daily mix and supports explicit refresh without public search', async () => {
    api.dailyRecommendation.mockResolvedValueOnce(daily('first')).mockResolvedValueOnce(daily('new'))
    const store = useDiscoveryStore()
    await store.loadDaily(); expect(api.dailyRecommendation).toHaveBeenLastCalledWith(false)
    await store.loadDaily(true); expect(api.dailyRecommendation).toHaveBeenLastCalledWith(true)
    expect(store.daily?.tracks[0]?.mid).toBe('new')
    expect(store.dailyState).toBe('ready')
    expect(api.searchPlaylists).not.toHaveBeenCalled()
  })
  it('does not treat an unavailable daily mix as successful public recommendations', async () => {
    api.dailyRecommendation.mockRejectedValueOnce(new Error('QQ 音乐尚未生成每日推荐'))
    const store = useDiscoveryStore(); await store.loadDaily()
    expect(store.daily).toBeNull(); expect(store.dailyState).toBe('error')
    expect(store.dailyError).toBe('QQ 音乐尚未生成每日推荐')
    expect(api.searchPlaylists).not.toHaveBeenCalled()
  })
  it('rejects a daily response that belongs to yesterday', async () => {
    api.dailyRecommendation.mockResolvedValueOnce(daily('old', recommendationDay() - 1))
    const store = useDiscoveryStore(); await store.loadDaily()
    expect(store.daily).toBeNull(); expect(store.dailyState).toBe('error')
    expect(store.dailyError).toContain('跨日')
  })
  it('ignores both daily and radio replies after an account change', async () => {
    const oldDaily = deferred<DailyRecommendation>(), oldRadio = deferred<Track[]>()
    api.dailyRecommendation.mockReturnValueOnce(oldDaily.promise).mockResolvedValueOnce(daily('account-b'))
    api.recommendationRadio.mockReturnValueOnce(oldRadio.promise).mockResolvedValueOnce([song('b-radio')])
    const store = useDiscoveryStore(), pendingDaily = store.loadDaily(), pendingRadio = store.loadRadio()
    store.reset(); await store.loadDaily(); await store.loadRadio()
    oldDaily.resolve(daily('account-a')); oldRadio.resolve([song('a-radio')]); await Promise.all([pendingDaily, pendingRadio])
    expect(store.daily?.playlist.id).toBe('account-b'); expect(store.radio[0]?.mid).toBe('b-radio')
  })
  it('does not let an older refresh replace a newer daily mix', async () => {
    const old = deferred<DailyRecommendation>()
    api.dailyRecommendation.mockReturnValueOnce(old.promise).mockResolvedValueOnce(daily('new'))
    const store = useDiscoveryStore(), pending = store.loadDaily()
    await store.loadDaily(true); old.resolve(daily('old')); await pending
    expect(store.daily?.playlist.id).toBe('new')
  })
  it('keeps the last radio batch and visibly reports a failed refresh', async () => {
    api.recommendationRadio.mockResolvedValueOnce([song('first')]).mockRejectedValueOnce(new Error('猜你喜欢加载失败'))
    const store = useDiscoveryStore(); await store.loadRadio(); await store.loadRadio()
    expect(store.radio[0]?.mid).toBe('first'); expect(store.radioState).toBe('error')
    expect(store.radioError).toBe('猜你喜欢加载失败')
  })
  it('clears personal data and errors on reset', async () => {
    api.dailyRecommendation.mockResolvedValueOnce(daily('account-a'))
    api.recommendationRadio.mockRejectedValueOnce(new Error('failed'))
    const store = useDiscoveryStore(); await store.loadDaily(); await store.loadRadio(); store.reset()
    expect(store.daily).toBeNull(); expect(store.radio).toEqual([])
    expect(store.dailyError).toBe(''); expect(store.radioError).toBe('')
    expect(store.dailyState).toBe('idle'); expect(store.radioState).toBe('idle')
  })
})
