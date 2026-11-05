import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { watch } from 'vue'
import type { PlaybackSource, PlaybackState, RawLyrics, Track } from '../src/types'
import type { ParsedLyrics } from '../src/lib/lyrics'

const api = vi.hoisted(() => ({ source: vi.fn(), lyrics: vi.fn(), record: vi.fn(), settings: vi.fn(), saveSettings: vi.fn(), playback: vi.fn(), savePlayback: vi.fn(), lyricOffset: vi.fn(), saveLyricOffset: vi.fn() }))
vi.mock('../src/lib/api', () => ({ api, desktop: true, errorMessage: (e: { message: string }) => e.message }))
const parseInWorker = vi.hoisted(() => vi.fn())
vi.mock('../src/lib/lyric-client', () => ({ parseInWorker }))

class TestAudio extends EventTarget {
  static latest: TestAudio
  static instances: TestAudio[] = []
  src = ''
  currentTime = 0
  volume = 0.7
  duration = 120
  preload = ''
  error = null
  constructor() { super(); TestAudio.latest = this; TestAudio.instances.push(this) }
  pause() { this.dispatchEvent(new Event('pause')) }
  load() { this.currentTime = 0 }
  removeAttribute(name: string) { if (name === 'src') this.src = '' }
  getAttribute(name: string) { return name === 'src' ? this.src : null }
  play() { this.dispatchEvent(new Event('loadedmetadata')); this.dispatchEvent(new Event('playing')); return Promise.resolve() }
}
const track = (id: number): Track => ({ id, mid: `mid${id}`, mediaMid: `media${id}`, title: `Track ${id}`, artist: 'Artist', album: 'Album', cover: '', duration: 120, vip: false })
const rawLyrics: RawLyrics = { word: '', line: '[00:40.00]Restored lyric', translation: '', romanization: '', warnings: [] }
const parsedLyrics: ParsedLyrics = { lines: [{ startTime: 40000, endTime: 50000,
  words: [{ word: 'Restored lyric', startTime: 40000, endTime: 50000 }],
  translatedLyric: '', romanLyric: '', isBG: false, isDuet: false,
}], mode: 'line', warnings: [] }
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(r => { resolve = r })
  return { promise, resolve }
}
const savedPlayback = (): PlaybackState => ({
  queue: [track(1), track(2), track(3)], currentMid: 'mid2', nextMid: 'mid3',
  position: 42.5, repeat: 'all', shuffle: true,
})

describe('playback request ownership', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.stubGlobal('Audio',TestAudio)
    vi.stubGlobal('navigator',{})
    vi.stubGlobal('requestAnimationFrame',() => 1)
    vi.stubGlobal('cancelAnimationFrame',vi.fn())
    vi.resetAllMocks()
    TestAudio.instances = []
    api.source.mockResolvedValue({ url: 'https://example.test/audio.mp3', quality: '320 kbps' })
    api.lyrics.mockImplementation(() => new Promise(() => {}))
    parseInWorker.mockResolvedValue(parsedLyrics)
    api.lyricOffset.mockResolvedValue(0)
    api.saveLyricOffset.mockResolvedValue(undefined)
    api.record.mockResolvedValue(undefined)
    api.playback.mockResolvedValue(null)
    api.savePlayback.mockResolvedValue(undefined)
    api.saveSettings.mockResolvedValue(undefined)
  })
  it('a slow earlier source cannot replace the song selected later', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const first = deferred<PlaybackSource>(), second = deferred<PlaybackSource>()
    api.source.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const player = usePlayerStore()
    const firstPlay = player.play(track(1),[track(1),track(2)])
    const secondPlay = player.play(track(2))
    second.resolve({ url: 'https://example.test/two.mp3', quality: '320 kbps' })
    await secondPlay
    first.resolve({ url: 'https://example.test/one.mp3', quality: '128 kbps' })
    await firstPlay
    expect(TestAudio.latest.src).toContain('two.mp3')
    expect(player.current?.id).toBe(2)
    expect(player.quality).toBe('320 kbps')
    expect(api.record).toHaveBeenCalledTimes(1)
  })
  it('logout/reset invalidates a playback address still in flight', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const source = deferred<PlaybackSource>()
    api.source.mockReturnValue(source.promise)
    const player = usePlayerStore()
    const pending = player.play(track(1))
    player.reset()
    source.resolve({ url: 'https://example.test/one.mp3', quality: '320 kbps' })
    await pending
    expect(player.current).toBeNull()
    expect(player.sourceState).toBe('idle')
    expect(TestAudio.latest.src).toBe('')
    expect(api.record).not.toHaveBeenCalled()
  })
  it('lyrics follow the live audio clock while ordinary UI updates are throttled', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    let scheduled!: FrameRequestCallback
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { scheduled = callback; return 1 })
    const now = vi.spyOn(performance, 'now').mockReturnValue(0)
    const player = usePlayerStore()
    await player.play(track(1))
    const updates = vi.fn()
    const stop = watch(() => player.time, updates, { flush: 'sync' })
    try {
      player.settings.lyricOffset = 125
      for (let frame = 1; frame <= 60; frame++) {
        TestAudio.latest.currentTime = frame / 60
        scheduled(frame * 1000 / 60)
        expect(player.readLyricTime()).toBe(Math.round(frame * 1000 / 60 + 125))
      }
      expect(updates.mock.calls.length).toBeGreaterThanOrEqual(8)
      expect(updates.mock.calls.length).toBeLessThanOrEqual(10)
      player.pause()
      expect(player.time).toBe(1000)
      player.seek(42.25)
      expect(player.time).toBe(42250)
      expect(player.readLyricTime()).toBe(42375)
      player.setVisualActive(false)
      expect(cancelAnimationFrame).toHaveBeenCalledWith(1)
    } finally { stop(); player.reset(); now.mockRestore() }
  })
  it('the lyric clock keeps the restored paused position before audio is loaded', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.playback.mockResolvedValue(savedPlayback())
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(TestAudio.latest.src).toBe('')
    expect(player.readLyricTime()).toBe(42500)
    player.settings.lyricOffset = -50000
    expect(player.readLyricTime()).toBe(0)
    player.reset()
    expect(player.readLyricTime()).toBe(0)
  })
  it('denied playback stays an error and is not written as a played song', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.source.mockRejectedValue({ message: '需要对应会员权益' })
    const player = usePlayerStore()
    await player.play(track(1))
    expect(player.playing).toBe(false)
    expect(player.sourceState).toBe('error')
    expect(player.playbackError).toContain('会员')
    expect(api.record).not.toHaveBeenCalled()
  })

  it('adding to an empty queue selects a paused song without requesting audio', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    player.enqueue(track(1))
    player.enqueue(track(1))
    expect(player.queue.map(t => t.id)).toEqual([1])
    expect(player.current?.id).toBe(1)
    expect(player.playing).toBe(false)
    expect(api.source).not.toHaveBeenCalled()
    await player.toggle()
    expect(player.playing).toBe(true)
  })

  it('play-next moves an existing song and takes priority over shuffle', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2), track(3)])
    player.setShuffle(true)
    player.enqueue(track(3), true)
    expect(player.queue.map(t => t.id)).toEqual([1, 3, 2])
    expect(player.nextMid).toBe('mid3')
    await player.next()
    expect(player.current?.id).toBe(3)
    expect(player.nextMid).toBeNull()
  })

  it('editing and clearing the queue preserves the current audio and progress', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2), track(2), track(3)])
    TestAudio.latest.currentTime = 37
    TestAudio.latest.dispatchEvent(new Event('timeupdate'))
    player.moveInQueue('mid3', -1)
    expect(player.queue.map(t => t.id)).toEqual([1, 3, 2])
    player.moveInQueue('mid1', -1)
    player.moveInQueue('missing', 1)
    player.removeFromQueue('mid1')
    player.removeFromQueue('mid2')
    expect(player.queue.map(t => t.id)).toEqual([1, 3])
    player.clearQueue()
    expect(player.queue.map(t => t.id)).toEqual([1])
    expect(player.time).toBe(37000)
    expect(player.playing).toBe(true)
    expect(api.source).toHaveBeenCalledTimes(1)
  })

  it('removing a requested next song cannot leave a dangling next selection', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2), track(3)])
    player.enqueue(track(2), true)
    player.removeFromQueue('mid2')
    expect(player.nextMid).toBeNull()
    await player.next()
    expect(player.current?.id).toBe(3)
  })

  it('restores paused and requests a fresh source before resuming at the saved time', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.playback.mockResolvedValue(savedPlayback())
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    player.setVisualActive(false)
    expect(player.time).toBe(42500)
    expect(player.current?.id).toBe(2)
    expect(player.nextMid).toBe('mid3')
    expect(player.repeat).toBe('all')
    expect(player.shuffle).toBe(true)
    expect(player.restored).toBe(true)
    expect(player.sourceState).toBe('idle')
    expect(api.source).not.toHaveBeenCalled()
    expect(api.record).not.toHaveBeenCalled()
    expect(api.savePlayback).not.toHaveBeenCalled()
    await player.toggle()
    expect(api.source).toHaveBeenCalledWith(track(2))
    expect(TestAudio.latest.currentTime).toBe(42.5)
    expect(player.playing).toBe(true)
    await player.flushPlayback()
    expect(api.savePlayback).toHaveBeenLastCalledWith('account-a', savedPlayback())
  })

  it('loads restored lyrics while paused without fetching audio or recording a play', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const pending = deferred<RawLyrics>()
    api.playback.mockResolvedValue(savedPlayback())
    api.lyrics.mockReturnValue(pending.promise)
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(api.lyrics).toHaveBeenCalledWith(track(2))
    expect(player.lyricState).toBe('loading')
    // Closing must not wait for a network lyric request to finish.
    await player.flushPlayback()
    expect(api.savePlayback).toHaveBeenLastCalledWith('account-a', savedPlayback())
    pending.resolve(rawLyrics)
    await vi.waitFor(() => expect(player.lyricState).toBe('ready'))
    expect(parseInWorker).toHaveBeenCalledWith(rawLyrics, track(2).duration)
    expect(player.lyricLines).toEqual(parsedLyrics.lines)
    expect(player.readLyricTime()).toBe(42500)
    expect(player.playing).toBe(false)
    expect(player.restored).toBe(true)
    expect(TestAudio.latest.src).toBe('')
    expect(api.source).not.toHaveBeenCalled()
    expect(api.record).not.toHaveBeenCalled()
  })

  it('reports restored lyric failure separately from saved playback recovery', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.playback.mockResolvedValue(savedPlayback())
    api.lyrics.mockRejectedValue({ message: '歌词网络请求失败' })
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(api.lyrics).toHaveBeenCalledWith(track(2))
    await vi.waitFor(() => expect(player.lyricState).toBe('error'))
    expect(player.lyricError).toBe('歌词网络请求失败')
    expect(player.notice).toBe('')
    expect(player.current?.mid).toBe('mid2')
    expect(player.restored).toBe(true)
    await expect(player.flushPlayback()).resolves.toBeUndefined()
  })

  it('a restored lyric response cannot replace lyrics after selecting another song', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const pending = deferred<RawLyrics>()
    api.playback.mockResolvedValue(savedPlayback())
    api.lyrics.mockReturnValueOnce(pending.promise).mockResolvedValueOnce(rawLyrics)
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(api.lyrics).toHaveBeenCalledWith(track(2))
    await player.play(track(3))
    await vi.waitFor(() => expect(player.lyricState).toBe('ready'))
    pending.resolve({ ...rawLyrics, line: '[00:01.00]Old lyrics' })
    await Promise.resolve()
    expect(player.current?.mid).toBe('mid3')
    expect(player.lyricLines).toEqual(parsedLyrics.lines)
    expect(parseInWorker).toHaveBeenCalledTimes(1)
    expect(parseInWorker).toHaveBeenCalledWith(rawLyrics, track(3).duration)
  })

  it('a restored lyric worker cannot refill the player after account reset', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const pending = deferred<ParsedLyrics>()
    api.playback.mockResolvedValue(savedPlayback())
    api.lyrics.mockResolvedValue(rawLyrics)
    parseInWorker.mockReturnValueOnce(pending.promise)
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(api.lyrics).toHaveBeenCalledWith(track(2))
    await vi.waitFor(() => expect(parseInWorker).toHaveBeenCalledOnce())
    player.reset()
    pending.resolve(parsedLyrics)
    await Promise.resolve()
    expect(player.current).toBeNull()
    expect(player.lyricLines).toEqual([])
    expect(player.lyricState).toBe('idle')
  })

  it('does not request lyrics when a saved queue has no current song', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.playback.mockResolvedValue({ ...savedPlayback(), currentMid: null, position: 0 })
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(player.current).toBeNull()
    expect(api.lyrics).not.toHaveBeenCalled()
  })

  it('a delayed restore cannot overwrite newly added songs', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const saved = deferred<PlaybackState | null>()
    api.playback.mockReturnValue(saved.promise)
    const player = usePlayerStore()
    const restoring = player.restorePlayback('account-a')
    player.enqueue(track(4))
    saved.resolve(savedPlayback())
    await restoring
    expect(player.queue.map(t => t.id)).toEqual([4])
    expect(player.current?.id).toBe(4)
  })

  it('a delayed restore cannot restart a player reset during logout', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const saved = deferred<PlaybackState | null>()
    api.playback.mockReturnValue(saved.promise)
    const player = usePlayerStore()
    const restoring = player.restorePlayback('account-a')
    player.reset()
    saved.resolve(savedPlayback())
    await restoring
    expect(player.queue).toEqual([])
    expect(player.current).toBeNull()
  })

  it('writes queue snapshots in order and retains the latest edit', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const firstSave = deferred<void>()
    api.savePlayback.mockReturnValueOnce(firstSave.promise)
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    player.enqueue(track(1))
    player.enqueue(track(2))
    await Promise.resolve()
    expect(api.savePlayback).toHaveBeenCalledTimes(1)
    firstSave.resolve(undefined)
    await player.flushPlayback()
    expect(api.savePlayback).toHaveBeenLastCalledWith('account-a', {
      queue: [track(1), track(2)], currentMid: 'mid1', nextMid: null,
      position: 0, repeat: 'off', shuffle: false,
    })
  })

  it('save failures remain visible and reject flush instead of reporting success', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    api.savePlayback.mockRejectedValue({ message: '磁盘已满' })
    player.enqueue(track(1))
    await expect(player.flushPlayback()).rejects.toEqual({ message: '磁盘已满' })
    expect(player.notice).toContain('磁盘已满')
    api.savePlayback.mockResolvedValue(undefined)
    await player.flushPlayback()
    expect(player.current?.id).toBe(1)
  })

  it('a failed restore is visible and does not overwrite saved data with an empty queue', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.playback.mockRejectedValue({ message: '本地资料库损坏' })
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    expect(player.notice).toContain('恢复失败')
    await expect(player.flushPlayback()).rejects.toThrow('本地资料库损坏')
    expect(api.savePlayback).not.toHaveBeenCalled()
    expect(api.source).not.toHaveBeenCalled()
  })

  it('closing during restore waits for the saved queue instead of persisting an empty queue', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const saved = deferred<PlaybackState | null>()
    api.playback.mockReturnValue(saved.promise)
    const player = usePlayerStore()
    const restoring = player.restorePlayback('account-a')
    const closing = player.flushPlayback()
    expect(api.savePlayback).not.toHaveBeenCalled()
    saved.resolve(savedPlayback())
    await restoring
    await closing
    expect(api.savePlayback).toHaveBeenLastCalledWith('account-a', savedPlayback())
  })

  it('previous restarts the restored song before selecting another song', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.playback.mockResolvedValue(savedPlayback())
    const player = usePlayerStore()
    await player.restorePlayback('account-a')
    await player.previous()
    expect(player.time).toBe(0)
    expect(player.current?.id).toBe(2)
    expect(api.source).not.toHaveBeenCalled()
    await player.toggle()
    expect(TestAudio.latest.currentTime).toBe(0)
    expect(player.current?.id).toBe(2)
  })

  it('sleep timer pauses at the deadline and does not fire again', async () => {
    vi.useFakeTimers()
    try {
      const { usePlayerStore } = await import('../src/stores/player')
      const player = usePlayerStore()
      await player.play(track(1), [track(1), track(2)])
      player.setSleepTimer(1)
      await vi.advanceTimersByTimeAsync(60000)
      expect(player.playing).toBe(false)
      expect(player.sleepDeadline).toBeNull()
      expect(player.notice).toContain('定时停止')
      expect(api.source).toHaveBeenCalledTimes(1)
      player.reset()
    } finally { vi.useRealTimers() }
  })

  it('removing the only downloaded song releases local audio without losing its queue', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.source.mockResolvedValue({ url: 'asset://localhost/song.flac', quality: '无损 FLAC · 本地', local: true })
    const player = usePlayerStore()
    await player.play(track(1), [track(1)])
    player.releaseDownload('mid1')
    expect(player.playing).toBe(false)
    expect(player.sourceState).toBe('idle')
    expect(player.localSource).toBe(false)
    expect(TestAudio.latest.src).toBe('')
    expect(player.queue.map(item => item.id)).toEqual([1])
    expect(player.current?.id).toBe(1)
  })

  it('sleep after this song stops before repeat or the next song', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2)])
    player.cycleRepeat()
    player.setSleepTimer('track')
    TestAudio.latest.dispatchEvent(new Event('ended'))
    expect(player.playing).toBe(false)
    expect(player.current?.id).toBe(1)
    expect(player.sleepAfterTrack).toBe(false)
    expect(api.source).toHaveBeenCalledTimes(1)
  })

  it('preloads the planned next audio once and reuses its media buffer when switching', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2), track(3)])
    const active = TestAudio.latest
    active.currentTime = 105
    active.dispatchEvent(new Event('timeupdate'))
    await vi.waitFor(() => expect(player.preloadState).toBe('ready'))
    const warm = TestAudio.latest
    expect(warm).not.toBe(active)
    warm.dispatchEvent(new Event('playing'))
    warm.dispatchEvent(new Event('timeupdate'))
    expect(player.current?.id).toBe(1)
    expect(player.time).toBe(105000)
    active.dispatchEvent(new Event('timeupdate'))
    expect(api.source).toHaveBeenCalledTimes(2)
    await player.next()
    expect(player.current?.id).toBe(2)
    expect(player.playing).toBe(true)
    expect(api.source).toHaveBeenCalledTimes(2)
    expect(TestAudio.instances).toHaveLength(2)
    expect(active.src).toBe('')
    player.reset()
  })

  it('invalidates an old preload after play-next changes or account reset', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2), track(3)])
    const pending = deferred<PlaybackSource>()
    api.source.mockReturnValueOnce(pending.promise)
    const active = TestAudio.latest
    active.currentTime = 105; active.dispatchEvent(new Event('timeupdate'))
    player.enqueue(track(3), true)
    expect(player.upcoming?.id).toBe(3)
    player.reset()
    pending.resolve({ url: 'https://example.test/stale.mp3', quality: '320 kbps' })
    await Promise.resolve(); await Promise.resolve()
    expect(player.preloadState).toBe('idle')
    expect(player.current).toBeNull()
    expect(TestAudio.instances).toHaveLength(1)
  })

  it('a failed buffered audio does not fail the current song and next requests a fresh source', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    await player.play(track(1), [track(1), track(2)])
    const active = TestAudio.latest
    active.currentTime = 105; active.dispatchEvent(new Event('timeupdate'))
    await vi.waitFor(() => expect(player.preloadState).toBe('ready'))
    TestAudio.latest.dispatchEvent(new Event('error'))
    expect(player.preloadState).toBe('error')
    expect(player.sourceState).toBe('ready')
    expect(player.playing).toBe(true)
    expect(player.playbackError).toBe('')
    await player.next()
    expect(api.source).toHaveBeenCalledTimes(3)
    expect(player.current?.id).toBe(2)
    player.reset()
  })

  it('applies per-song offsets to the lyric clock only after persistence succeeds', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    api.lyricOffset.mockResolvedValueOnce(150)
    const player = usePlayerStore()
    await player.play(track(1))
    TestAudio.latest.currentTime = 10
    player.settings.lyricOffset = 50
    expect(player.readLyricTime()).toBe(10200)
    api.saveLyricOffset.mockRejectedValueOnce(new Error('Database unavailable'))
    await player.updateTrackOffset(400)
    expect(player.trackOffset).toBe(150)
    expect(player.trackOffsetError).toContain('Database')
    await player.updateTrackOffset(-200)
    expect(api.saveLyricOffset).toHaveBeenLastCalledWith('mid1', -200)
    expect(player.readLyricTime()).toBe(9850)
    player.reset()
  })

  it('flushes pending preference edits before closing instead of losing a debounced save', async () => {
    const { usePlayerStore } = await import('../src/stores/player')
    const player = usePlayerStore()
    player.updateSettings({ lyrics: { ...player.settings.lyrics, fontScale: 120, showTranslation: false } })
    await player.flushPlayback()
    expect(api.saveSettings).toHaveBeenCalledTimes(1)
    expect(api.saveSettings.mock.calls[0]![0].lyrics).toMatchObject({ fontScale: 120, showTranslation: false })
    player.reset()
  })

  it('a sleep deadline cancels a source request still loading', async () => {
    vi.useFakeTimers()
    try {
      const { usePlayerStore } = await import('../src/stores/player')
      const source = deferred<PlaybackSource>()
      api.source.mockReturnValue(source.promise)
      const player = usePlayerStore()
      const starting = player.play(track(1))
      player.setSleepTimer(1)
      await vi.advanceTimersByTimeAsync(60000)
      source.resolve({ url: 'https://example.test/late.mp3', quality: '320 kbps' })
      await starting
      expect(player.playing).toBe(false)
      expect(player.sourceState).toBe('idle')
      expect(TestAudio.latest.src).toBe('')
      player.reset()
    } finally { vi.useRealTimers() }
  })
})
