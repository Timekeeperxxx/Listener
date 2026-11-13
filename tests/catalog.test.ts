import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { ArtistDetail, ArtistRef, CatalogPage, Track } from '../src/types'
const api = vi.hoisted(() => ({ catalogArtists: vi.fn(), catalogAlbums: vi.fn(), artistDetail: vi.fn(), artistSongs: vi.fn(), artistAlbums: vi.fn(), albumDetail: vi.fn(), catalogTrack: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, errorMessage: (e: Error) => e.message }))
import { useCatalogStore } from '../src/stores/catalog'
import { artistNames, resolveAlbum, resolveArtist } from '../src/lib/catalog'
const ref: ArtistRef = { mid: 'qqartist', name: 'Outside Favorites', image: 'portrait.jpg' }
const song: Track = { id: 1, mid: 'qqsong', mediaMid: '', title: 'Song', artist: 'Outside Favorites', album: 'Album', cover: '', duration: 180, vip: false }
const page = <T>(items: T[], total = items.length, nextOffset = items.length): CatalogPage<T> => ({ items, total, nextOffset, hasMore: nextOffset < total })
const profile: ArtistDetail = { artist: ref, description: 'Real biography', totalSongs: 1013, totalAlbums: 43 }
function detail() { api.artistDetail.mockResolvedValue(profile); api.artistSongs.mockResolvedValue(page([song], 1013)); api.artistAlbums.mockResolvedValue(page([], 0)) }

describe('QQ catalog independent of the personal library', () => {
  beforeEach(() => { setActivePinia(createPinia()); vi.resetAllMocks() })
  it('browses online artists without reading favorites and sends a search to QQ', async () => {
    api.catalogArtists.mockResolvedValue(page([ref], 23782, 80))
    const catalog = useCatalogStore()
    await catalog.loadArtists()
    expect(api.catalogArtists).toHaveBeenCalledWith('', 0, -100)
    expect(catalog.artists.total).toBe(23782)
    catalog.artistQuery = 'A singer outside my library'
    await catalog.loadArtists()
    expect(api.catalogArtists).toHaveBeenLastCalledWith('A singer outside my library', 0, -100)
  })
  it('uses the server nextOffset even when search omits one unavailable item', async () => {
    api.catalogArtists.mockResolvedValueOnce(page(Array.from({ length: 19 }, (_, i) => ({ ...ref, mid: `a${i}` })), 47, 20)).mockResolvedValueOnce(page([ref], 47, 40))
    const catalog = useCatalogStore(); catalog.artistQuery = 'Artist'
    await catalog.loadArtists(); await catalog.loadArtists(true)
    expect(api.catalogArtists).toHaveBeenLastCalledWith('Artist', 20, -100)
    expect(catalog.artists.items).toHaveLength(20)
  })
  it('keeps an old search from overwriting the latest query', async () => {
    let resolve!: (value: CatalogPage<ArtistRef>) => void
    api.catalogArtists.mockReturnValueOnce(new Promise(r => { resolve = r })).mockResolvedValueOnce(page([{ ...ref, mid: 'latest' }]))
    const catalog = useCatalogStore(); const old = catalog.loadArtists()
    catalog.artistQuery = 'latest'; await catalog.loadArtists()
    resolve(page([ref])); await old
    expect(catalog.artists.items[0]?.mid).toBe('latest')
  })
  it('appends a singer’s full catalog with actual total and surfaces next-page failure', async () => {
    detail(); const catalog = useCatalogStore(); await catalog.loadArtist(ref.mid)
    expect(catalog.artist?.description).toBe('Real biography')
    expect(catalog.songs.total).toBe(1013)
    api.artistSongs.mockResolvedValueOnce(page([{ ...song, mid: 'page2' }], 1013, 2))
    await catalog.loadArtistSongs()
    expect(catalog.songs.items).toHaveLength(2)
    api.artistSongs.mockRejectedValueOnce(new Error('QQ unavailable'))
    await catalog.loadArtistSongs()
    expect(catalog.songs.state).toBe('error')
    expect(catalog.songs.error).toBe('QQ unavailable')
    expect(catalog.songs.items).toHaveLength(2)
    expect(catalog.songs.hasMore).toBe(true)
  })
  it('builds a full artist queue across all pages before returning any playback list', async () => {
    detail()
    api.artistSongs.mockReset().mockResolvedValueOnce(page([song], 3, 1))
      .mockResolvedValueOnce(page([{ ...song, mid: 'second' }], 3, 2))
      .mockResolvedValueOnce(page([{ ...song, mid: 'third' }], 3, 3))
    const catalog = useCatalogStore(); await catalog.loadArtist(ref.mid)
    const queue = await catalog.completeArtistSongs()
    expect(queue.map(t => t.mid)).toEqual(['qqsong', 'second', 'third'])
    expect(api.artistSongs.mock.calls.map(args => args[1])).toEqual([0, 1, 2])
  })
  it('paginates release order independently and keeps it in the playback queue', async () => {
    detail()
    const latest = { ...song, mid: 'latest' }
    const earlier = { ...song, mid: 'earlier' }
    api.artistSongs.mockImplementation((_mid, offset, order) => Promise.resolve(
      order === 'popular' ? page([song]) : offset === 0 ? page([latest], 2, 1) : page([earlier], 2, 2),
    ))
    const catalog = useCatalogStore()
    await catalog.loadArtist(ref.mid)
    await catalog.loadArtistSongs('newest')
    expect(await catalog.completeArtistSongs('newest')).toEqual([latest, earlier])
    expect(await catalog.completeArtistSongs('popular')).toEqual([song])
    expect(api.artistSongs.mock.calls).toEqual([[ref.mid, 0, 'popular'], [ref.mid, 0, 'newest'], [ref.mid, 1, 'newest']])
  })
  it('surfaces release-order failure and retries its first page without replacing the popular list', async () => {
    detail()
    const catalog = useCatalogStore()
    await catalog.loadArtist(ref.mid)
    api.artistSongs.mockRejectedValueOnce(new Error('排序请求失败'))
    await catalog.loadArtistSongs('newest')
    expect(catalog.newestSongs).toMatchObject({ state: 'error', error: '排序请求失败', items: [] })
    expect(catalog.songs.items).toEqual([song])
    const latest = { ...song, mid: 'latest' }
    api.artistSongs.mockResolvedValueOnce(page([latest]))
    await catalog.loadArtistSongs('newest')
    expect(api.artistSongs).toHaveBeenLastCalledWith(ref.mid, 0, 'newest')
    expect(catalog.newestSongs).toMatchObject({ state: 'ready', items: [latest] })
  })
  it('does not return a partial artist queue if the remaining catalog fails to load', async () => {
    detail(); const catalog = useCatalogStore(); await catalog.loadArtist(ref.mid)
    api.artistSongs.mockRejectedValueOnce(new Error('QQ unavailable'))
    await expect(catalog.completeArtistSongs()).rejects.toThrow('QQ unavailable')
    expect(catalog.songs.items).toEqual([song])
  })
  it('does not let an old artist page append songs to a newly opened artist', async () => {
    detail(); const catalog = useCatalogStore(); await catalog.loadArtist(ref.mid)
    let resolve!: (value: CatalogPage<Track>) => void
    api.artistSongs.mockReturnValueOnce(new Promise(r => { resolve = r })).mockResolvedValueOnce(page([{ ...song, mid: 'new-song' }]))
    const old = catalog.loadArtistSongs(); await catalog.loadArtist('newartist')
    resolve(page([{ ...song, mid: 'old-page' }])); await old
    expect(catalog.songs.items.map(t => t.mid)).toEqual(['new-song'])
  })
  it('rejects stale replies after account reset', async () => {
    let resolve!: (value: CatalogPage<ArtistRef>) => void
    api.catalogArtists.mockReturnValue(new Promise(r => { resolve = r }))
    const catalog = useCatalogStore(); const loading = catalog.loadArtists(); catalog.reset()
    resolve(page([ref])); await loading
    expect(catalog.artists.state).toBe('idle'); expect(catalog.artists.items).toEqual([])
  })
  it('keeps metadata request failure visible instead of showing a successful empty artist', async () => {
    detail(); api.artistDetail.mockRejectedValue(new Error('Biography unavailable'))
    const catalog = useCatalogStore(); await catalog.loadArtist(ref.mid)
    expect(catalog.artistState).toBe('error'); expect(catalog.artistError).toBe('Biography unavailable')
    expect(catalog.artist).toBeNull()
  })
  it('preserves distinct credits such as AC/DC and resolves stored tracks by exact song MID', async () => {
    expect(artistNames({ artist: 'AC/DC / Artist B / Artist B' })).toEqual(['AC/DC', 'Artist B'])
    api.catalogTrack.mockResolvedValue({ ...song, artists: [ref], albumMid: 'realalbum' })
    expect(await resolveArtist(song, ref.name)).toEqual(ref)
    expect(api.catalogTrack).toHaveBeenCalledWith('qqsong')
    expect(await resolveAlbum(song)).toBe('realalbum')
    await expect(resolveArtist(song, 'Unrelated artist')).rejects.toThrow('没有提供')
  })
  it('uses known QQ IDs directly and never guesses a same-named album', async () => {
    const known = { ...song, artists: [ref], albumMid: 'realalbum' }
    expect(await resolveArtist(known, ref.name)).toEqual(ref)
    expect(await resolveAlbum(known)).toBe('realalbum')
    expect(api.catalogTrack).not.toHaveBeenCalled()
  })
})
