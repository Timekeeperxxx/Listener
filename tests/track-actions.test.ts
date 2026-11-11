// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, reactive, type App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import type { Download, Library, Track, TrackMenuTarget } from '../src/types'
const api = vi.hoisted(() => ({ mutatePlaylist: vi.fn(), download: vi.fn(), playlist: vi.fn(), library: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, desktop: false, errorMessage: (cause: Error) => cause.message }))
vi.mock('../src/stores/player', () => ({ usePlayerStore: () => player }))
import TrackActions from '../src/components/TrackActions.vue'
import { useLibraryStore } from '../src/stores/library'
import { useDownloadsStore } from '../src/stores/downloads'
const player = reactive({ current: null as Track | null, playing: false, play: vi.fn(), toggle: vi.fn(), enqueue: vi.fn() })
const song = (mid: string): Track => ({ id: 1, mid, mediaMid: mid, title: mid, artist: 'Artist', album: 'Album', cover: '', duration: 120, vip: false })
const list: Library = { playlists: [{ id: 'mine', name: '我的歌单', kind: 'created', count: 1, cover: '' }, { id: 'others', name: '收藏的歌单', kind: 'collected', count: 1, cover: '' }], warnings: [] }
function deferred<T>() { let resolve!: (value: T) => void, reject!: (cause: Error) => void; const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail }); return { promise, resolve, reject } }
let app: App, menu: { open: (target: TrackMenuTarget) => Promise<void>; close: (restore?: boolean) => void }, anchor: HTMLButtonElement
const flush = async () => { await Promise.resolve(); await nextTick(); await Promise.resolve(); await nextTick() }
const button = (name: string) => [...document.querySelectorAll<HTMLButtonElement>('.track-menu-item')].find(item => item.textContent!.trim() === name)!
const open = (track = song('first'), trigger = anchor) => menu.open({ track, tracks: [track, song('second')], anchor: trigger })
beforeEach(() => {
  vi.resetAllMocks()
  const pinia = createPinia(); setActivePinia(pinia)
  const library = useLibraryStore(); library.profile = { uin: 'account-a', nickname: 'A', avatar: '' }; library.playlists = list.playlists; library.libraryState = 'ready'; library.likedState = 'ready'
  player.current = null; player.playing = false
  anchor = document.createElement('button'); anchor.textContent = '更多'; document.body.append(anchor)
  const root = document.createElement('div'); document.body.append(root)
  app = createApp(TrackActions); app.use(pinia); menu = app.mount(root) as unknown as typeof menu
})
afterEach(() => { app.unmount(); document.body.replaceChildren() })
describe('non-modal song actions', () => {
  it('opens an accessible menu without a dialog and restores focus on Escape', async () => {
    await open()
    expect(document.querySelector('dialog')).toBeNull()
    expect(document.querySelector('[role="menu"]')).not.toBeNull()
    expect(anchor.getAttribute('aria-expanded')).toBe('true')
    expect(document.activeElement).toBe(button('播放'))
    button('播放').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flush()
    expect(document.querySelector('[role="menu"]')).toBeNull()
    expect(document.activeElement).toBe(anchor)
    expect(anchor.getAttribute('aria-expanded')).toBe('false')
  })
  it('passes the selected song to play-next without starting playback', async () => {
    const track = song('chosen'); await open(track)
    button('下一首播放').click(); await flush()
    expect(player.enqueue).toHaveBeenCalledExactlyOnceWith(track, true)
    expect(player.play).not.toHaveBeenCalled()
    expect(document.querySelector('[role="menu"]')).toBeNull()
  })
  it('does not intercept clicks on the rest of the page', async () => {
    const outside = document.createElement('button'), action = vi.fn(); outside.addEventListener('click', action); document.body.append(outside)
    await open(); outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })); outside.click(); await flush()
    expect(action).toHaveBeenCalledTimes(1)
    expect(document.querySelector('[role="menu"]')).toBeNull()
  })
  it('adds only to an owned playlist and does not close a newer song menu on completion', async () => {
    const pending = deferred<Library>(); api.mutatePlaylist.mockReturnValue(pending.promise)
    const first = song('first'); await open(first); button('添加到').click(); await flush()
    expect(button('我的歌单')).toBeDefined(); expect(button('收藏的歌单')).toBeUndefined()
    button('我的歌单').click(); await flush()
    const another = document.createElement('button'); document.body.append(another); await open(song('newer'), another)
    pending.resolve(list); await flush()
    expect(api.mutatePlaylist).toHaveBeenCalledExactlyOnceWith('add', 'mine', null, first)
    expect(document.querySelector('[role="menu"]')?.getAttribute('aria-label')).toBe('newer 的更多操作')
  })
  it('keeps a failed QQ write visible and does not display a success message', async () => {
    api.mutatePlaylist.mockRejectedValue(new Error('QQ 拒绝写入'))
    await open(); button('添加到').click(); await flush(); button('我的歌单').click(); await flush()
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('QQ 拒绝写入')
    expect(document.querySelector('[role="menu"]')).not.toBeNull()
    expect(document.querySelector('[role="status"]')).toBeNull()
  })
  it('shows a download failure outside the closed menu instead of claiming completion', async () => {
    const pending = deferred<Download>(); api.download.mockReturnValue(pending.promise)
    const track = song('download-me'); await open(track); button('下载').click(); await flush(); button('下载到此 Mac').click(); await flush()
    expect(api.download).toHaveBeenCalledExactlyOnceWith(track)
    expect(document.querySelector('[role="menu"]')).toBeNull()
    pending.reject(new Error('音源权限不足')); await flush()
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('音源权限不足')
    expect(useDownloadsStore().records).toEqual([])
  })
  it('does not announce an old account download after an account change', async () => {
    const pending = deferred<Download>(); api.download.mockReturnValue(pending.promise)
    const track = song('previous-account'); await open(track); button('下载').click(); await flush(); button('下载到此 Mac').click(); await flush()
    useLibraryStore().profile = { uin: 'account-b', nickname: 'B', avatar: '' }; useDownloadsStore().clear()
    pending.resolve({ track, bytes: 2048, quality: 'MP3', filename: 'song.mp3' }); await flush()
    expect(document.querySelector('.track-action-feedback')).toBeNull()
    expect(useDownloadsStore().records).toEqual([])
  })
})
