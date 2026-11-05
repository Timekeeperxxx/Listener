import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Playlist, PlaylistPage, Track } from '../src/types'

const api = vi.hoisted(() => ({ playlist:vi.fn(),openLogin:vi.fn(),pollLogin:vi.fn(),cancelLogin:vi.fn(),history:vi.fn(),logout:vi.fn() }))
vi.mock('../src/lib/api',() => ({ api, desktop:true, errorMessage:(e: { message: string }) => e.message }))
const playlist = (id:string): Playlist => ({ id, name:id, cover:'', count:0, kind:'created' })
const page: PlaylistPage = { tracks:[],total:0,nextOffset:0,hasMore:false }
const track: Track = { id:1,mid:'recent',mediaMid:'recent',title:'Recent',artist:'Artist',album:'Album',cover:'',duration:60,vip:false }

describe('library navigation and login cancellation',() => {
  beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks() })
  it('catalog navigation preserves the selected playlist and returns through artist and album pages', async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    const library = useLibraryStore()
    library.view = 'playlist'
    library.selected = playlist('selected')
    library.tracks = [track]
    library.showCatalog('artist', 'Artist')
    library.showCatalog('artist', 'Artist')
    library.showCatalog('album', 'album-key')
    library.backCatalog()
    expect(library.view).toBe('artist')
    expect(library.artistMid).toBe('Artist')
    library.backCatalog()
    expect(library.view).toBe('playlist')
    expect(library.selected?.id).toBe('selected')
    expect(library.tracks).toEqual([track])
  })
  it('catalog navigation cancels a pending playlist response and logout clears catalog navigation', async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    let resolve!: (value: PlaylistPage) => void
    api.playlist.mockReturnValueOnce(new Promise(r => { resolve = r }))
    api.logout.mockResolvedValue(undefined)
    const library = useLibraryStore()
    const pending = library.openPlaylist(playlist('selected'))
    library.showCatalog('artists')
    resolve({ tracks: [track], total: 1, nextOffset: 1, hasMore: false })
    await pending
    expect(library.view).toBe('artists')
    await library.logout()
    expect(library.artistMid).toBe('')
  })
  it('a delayed playlist response cannot overwrite a newly selected playlist',async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    let resolve!: (value:PlaylistPage) => void
    api.playlist.mockReturnValueOnce(new Promise(r => { resolve = r })).mockResolvedValueOnce(page)
    const library = useLibraryStore()
    const first = library.openPlaylist(playlist('first'))
    await library.openPlaylist(playlist('second'))
    resolve({ ...page,total:100 })
    await first
    expect(library.selected?.id).toBe('second')
    expect(library.total).toBe(0)
  })
  it('a QR image returned after cancellation does not restart polling',async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    let resolve!: (value:unknown) => void
    api.openLogin.mockReturnValue(new Promise(r => { resolve = r }))
    api.cancelLogin.mockResolvedValue(undefined)
    const library = useLibraryStore()
    const connecting = library.connect()
    await library.cancelLogin()
    resolve({ id:'qr-1',image:'data:image/png;base64,test',expiresIn:180 })
    await connecting
    expect(library.loginOpen).toBe(false)
    expect(library.loginQr).toBeNull()
    expect(api.pollLogin).not.toHaveBeenCalled()
  })
  it('play-all cannot spin while a page request is already in flight',async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    const library = useLibraryStore()
    library.loadingMore = true
    library.hasMore = true
    await expect(library.completePlaylist()).rejects.toThrow('正在加载')
    expect(api.playlist).not.toHaveBeenCalled()
  })
  it('loading the library recent section preserves the selected playlist and its tracks',async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    const library = useLibraryStore()
    library.view = 'playlist'
    library.selected = playlist('selected')
    library.tracks = [track]
    api.history.mockResolvedValue([])
    await library.loadRecent()
    expect(library.view).toBe('playlist')
    expect(library.selected.id).toBe('selected')
    expect(library.tracks).toEqual([track])
    expect(library.recentState).toBe('ready')
  })
  it('a delayed recent response cannot repopulate a signed-out library',async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    const library = useLibraryStore()
    let resolve!: (value:Track[]) => void
    api.history.mockReturnValue(new Promise(r => { resolve = r }))
    api.logout.mockResolvedValue(undefined)
    const loading = library.loadRecent()
    await library.logout()
    resolve([track])
    await loading
    expect(library.recentTracks).toEqual([])
    expect(library.recentState).toBe('idle')
  })
  it('a recent-history failure remains visible without turning the music library into an error',async () => {
    const { useLibraryStore } = await import('../src/stores/library')
    const library = useLibraryStore()
    library.libraryState = 'ready'
    api.history.mockRejectedValue(new Error('history database unavailable'))
    await library.loadRecent()
    expect(library.recentState).toBe('error')
    expect(library.recentError).toBe('history database unavailable')
    expect(library.libraryState).toBe('ready')
  })
})
