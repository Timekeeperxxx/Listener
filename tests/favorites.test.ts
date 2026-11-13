import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Library, Playlist, PlaylistPage, Track } from '../src/types'
const api = vi.hoisted(() => ({ playlist: vi.fn(), mutatePlaylist: vi.fn(), logout: vi.fn(), library: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, native: true, errorMessage: (cause: { message: string }) => cause.message }))
import { useLibraryStore } from '../src/stores/library'
const song = (id: number): Track => ({ id, mid: `mid${id}`, mediaMid: '', title: `Song ${id}`, artist: 'Artist', album: '', cover: '', duration: 60, vip: false })
const liked = (count: number): Playlist => ({ id: 'liked', name: '我喜欢', kind: 'liked', count, cover: '', dirId: 201 })
const libraryResult = (count: number): Library => ({ playlists: [liked(count)], warnings: [] })
const page = (tracks: Track[]): PlaylistPage => ({ tracks, total: tracks.length, nextOffset: tracks.length, hasMore: false })
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done }); return { promise, resolve } }
function readyStore(ids: string[] = []) {
  const store = useLibraryStore()
  store.profile = { uin: '123', nickname: 'Test', avatar: '' }
  store.likedState = 'ready'
  store.likedMids = new Set(ids)
  return store
}
describe('visible favorite controls share confirmed state', () => {
  beforeEach(() => { setActivePinia(createPinia()); vi.resetAllMocks(); api.logout.mockResolvedValue(undefined) })
  it('shares a paginated favorite read and reuses it when more buttons mount', async () => {
    api.playlist.mockResolvedValueOnce({ ...page([song(1)]), hasMore: true, nextOffset: 100 }).mockResolvedValueOnce(page([song(2)]))
    const store = useLibraryStore()
    await Promise.all([store.loadLiked(), store.loadLiked()])
    await store.loadLiked()
    expect(api.playlist).toHaveBeenCalledTimes(2)
    expect([...store.likedMids]).toEqual(['mid1', 'mid2'])
    expect(store.likedTracks.map(track => track.mid)).toEqual(['mid1', 'mid2'])
  })
  it('waits for confirmation before filling hearts and ignores a second click while saving', async () => {
    const result = deferred<Library>()
    api.mutatePlaylist.mockReturnValue(result.promise)
    const store = readyStore()
    const saving = store.toggleLike(song(1))
    await store.toggleLike(song(1))
    expect(store.likedMids.has('mid1')).toBe(false)
    expect(store.likingMid).toBe('mid1')
    expect(api.mutatePlaylist).toHaveBeenCalledTimes(1)
    result.resolve(libraryResult(1))
    await saving
    expect(store.likedMids.has('mid1')).toBe(true)
    expect(store.likedTracks.map(track => track.mid)).toEqual(['mid1'])
    expect(store.likingMid).toBeNull()
  })
  it('a failed favorite write stays visible and leaves the heart state unconfirmed', async () => {
    api.mutatePlaylist.mockRejectedValue({ message: 'QQ 拒绝写入' })
    const store = readyStore()
    await store.toggleLike(song(1))
    expect(store.likedMids.has('mid1')).toBe(false)
    expect(store.likedState).toBe('error')
    expect(store.favoriteError).toBe('QQ 拒绝写入')
    expect(store.likingMid).toBeNull()
  })
  it('unliking a visible song updates the liked list without blanking and reloading it', async () => {
    api.playlist.mockResolvedValue(page([song(1), song(2)]))
    api.mutatePlaylist.mockResolvedValue(libraryResult(1))
    const store = readyStore(['mid1', 'mid2'])
    await store.openPlaylist(liked(2))
    await store.toggleLike(song(1))
    expect(store.tracks.map(track => track.mid)).toEqual(['mid2'])
    expect(store.total).toBe(1)
    expect(store.state).toBe('ready')
    expect(store.hasMore).toBe(false)
    expect(api.playlist).toHaveBeenCalledTimes(1)
  })
  it('a favorite read started before the confirmed write cannot erase the new heart state', async () => {
    const mutation = deferred<Library>(), reading = deferred<PlaylistPage>()
    api.mutatePlaylist.mockReturnValue(mutation.promise)
    api.playlist.mockReturnValue(reading.promise)
    const store = readyStore()
    const saving = store.toggleLike(song(1))
    const refresh = store.loadLiked(true)
    mutation.resolve(libraryResult(1)); await saving
    reading.resolve(page([])); await refresh
    expect(store.likedMids.has('mid1')).toBe(true)
    expect(store.likedState).toBe('ready')
    expect(store.likedTracks.map(track => track.mid)).toEqual(['mid1'])
  })
  it('favorites arriving after logout cannot restore the old account state', async () => {
    const reading = deferred<PlaylistPage>()
    api.playlist.mockReturnValue(reading.promise)
    const store = readyStore()
    const loading = store.loadLiked(true)
    await store.logout()
    reading.resolve(page([song(1)])); await loading
    expect(store.likedState).toBe('idle')
    expect(store.likedMids.size).toBe(0)
    expect(store.likedTracks).toEqual([])
  })
})
