import { defineStore } from 'pinia'
import { computed, ref, shallowRef, watch } from 'vue'
import { api, desktop, errorMessage } from '../lib/api'
import { parseInWorker } from '../lib/lyric-client'
import { defaultSettings } from '../lib/preferences'
import type { LyricLine } from '@applemusic-like-lyrics/core'
import type { AppSettings, LoadState, PlaybackSource, PlaybackState, ShortcutSettings, Track } from '../types'

export const usePlayerStore = defineStore('player', () => {
  const current = shallowRef<Track | null>(null)
  const queue = shallowRef<Track[]>([])
  const playing = ref(false)
  const buffering = ref(false)
  const time = ref(0)
  const duration = ref(0)
  const quality = ref('')
  const localSource = ref(false)
  const sourceState = ref<LoadState>('idle')
  const playbackError = ref('')
  const lyricState = ref<LoadState>('idle')
  const lyricError = ref('')
  const lyricWarnings = ref<string[]>([])
  const lyricLines = shallowRef<LyricLine[]>([])
  const lyricMode = ref<'word' | 'line' | 'none'>('none')
  const notice = ref('')
  const repeat = ref<'off' | 'all' | 'one'>('off')
  const shuffle = ref(false)
  const nextMid = ref<string | null>(null)
  const restored = ref(false)
  const sleepDeadline = ref<number | null>(null)
  const sleepAfterTrack = ref(false)
  const sleepNow = ref(Date.now())
  const sleepRemaining = computed(() => sleepDeadline.value === null ? 0 : Math.max(0, Math.ceil((sleepDeadline.value - sleepNow.value) / 60000)))
  const visualActive = ref(true)
  const settings = ref<AppSettings>(defaultSettings())
  const trackOffset = ref(0)
  const trackOffsetError = ref('')
  const preloadState = ref<LoadState>('idle')
  const preloadError = ref('')
  const upcoming = shallowRef<Track | null>(null)
  let audio = new Audio()
  audio.preload = 'metadata'
  let generation = 0
  let frame = 0
  let lastUiFrame = 0
  let recorded = false
  let prepared: { mid: string; media: HTMLAudioElement; source: PlaybackSource; request: number } | null = null
  let preparingFor = ''
  let preparation = 0
  function cancelPreparation() {
    preparation++
    if (prepared) { prepared.media.removeAttribute('src'); prepared.media.load() }
    prepared = null; preparingFor = ''; preloadState.value = 'idle'; preloadError.value = ''
  }
  function planNext() {
    cancelPreparation()
    const index = currentIndex.value
    if (index < 0 || !queue.value.length || repeat.value === 'one') { upcoming.value = null; return }
    if (nextMid.value) { upcoming.value = queue.value.find(track => track.mid === nextMid.value)!; return }
    if (shuffle.value && queue.value.length > 1) {
      upcoming.value = queue.value[(index + 1 + Math.floor(Math.random() * (queue.value.length - 1))) % queue.value.length]
    } else upcoming.value = index + 1 < queue.value.length ? queue.value[index + 1] : repeat.value === 'all' ? queue.value[0] : null
  }
  async function prepareNext() {
    const track = upcoming.value
    if (!settings.value.preloadNext || !playing.value || !track || repeat.value === 'one' || preparingFor || !audio.getAttribute('src') || duration.value - audio.currentTime > 20) return
    const request = generation, version = ++preparation
    preparingFor = track.mid; preloadState.value = 'loading'; preloadError.value = ''
    try {
      const source = await api.source(track)
      if (request !== generation || version !== preparation) return
      const media = new Audio(); media.preload = 'auto'; media.muted = true
      bindAudio(media)
      media.src = source.url; media.load()
      prepared = { mid: track.mid, media, source, request }; preloadState.value = 'ready'
    } catch (cause) {
      if (request === generation && version === preparation) { preloadState.value = 'error'; preloadError.value = errorMessage(cause) }
    }
  }
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let settingsSave = Promise.resolve()
  let playbackOwner: string | null = null
  let revision = 0
  let checkpoint = 0
  let resumePosition = 0
  let saveChain = Promise.resolve()
  let restorePromise = Promise.resolve()
  let restoreFailure = ''
  let sleepTimer: ReturnType<typeof setInterval> | undefined

  function setSleepTimer(value: number | 'track' | null) {
    clearInterval(sleepTimer)
    sleepDeadline.value = null
    sleepAfterTrack.value = value === 'track'
    if (typeof value === 'number') {
      if (!Number.isFinite(value) || value < 1 || value > 240) throw new Error('定时停止时间需要在 1–240 分钟内。')
      sleepNow.value = Date.now()
      sleepDeadline.value = sleepNow.value + value * 60000
      sleepTimer = setInterval(checkSleepTimer, 1000)
    }
  }
  function pause() {
    if (sourceState.value === 'loading') {
      generation++
      sourceState.value = 'idle'
      buffering.value = false
      lyricState.value = 'idle'
    }
    audio.pause()
  }
  function checkSleepTimer() {
    sleepNow.value = Date.now()
    if (sleepDeadline.value !== null && sleepNow.value >= sleepDeadline.value) {
      pause()
      setSleepTimer(null)
      notice.value = '已按定时停止播放。'
    }
  }

  const lyricTime = computed(() => Math.max(0, Math.round(time.value + settings.value.lyricOffset + trackOffset.value)))
  const progress = computed(() => duration.value > 0 ? time.value / (duration.value * 1000) * 100 : 0)
  const currentIndex = computed(() => queue.value.findIndex(t => t.mid === current.value?.mid))

  function syncTime() {
    if (audio.getAttribute('src')) time.value = Math.round(audio.currentTime * 1000)
  }
  // Lyrics read the media clock inside their own animation frame. Keeping that
  // clock out of Vue avoids rendering the whole player for every lyric frame.
  function readLyricTime() {
    const position = audio.getAttribute('src') ? audio.currentTime * 1000 : time.value
    return Math.max(0, Math.round(position + settings.value.lyricOffset + trackOffset.value))
  }
  function tick(now: number) {
    if (now - lastUiFrame >= 100) {
      syncTime()
      lastUiFrame = now
    }
    if (playing.value && visualActive.value) frame = requestAnimationFrame(tick)
  }
  function updateFrame() {
    cancelAnimationFrame(frame)
    syncTime()
    lastUiFrame = performance.now()
    if (playing.value && visualActive.value) frame = requestAnimationFrame(tick)
  }
  function setVisualActive(value: boolean) { visualActive.value = value; updateFrame() }

  function persistPlayback(): Promise<void> {
    revision++
    restoreFailure = ''
    if (!desktop || !playbackOwner) return Promise.resolve()
    const owner = playbackOwner
    const playback: PlaybackState = {
      queue: [...queue.value], currentMid: current.value ? current.value.mid : null,
      nextMid: nextMid.value, position: time.value / 1000, repeat: repeat.value, shuffle: shuffle.value,
    }
    const save = () => api.savePlayback(owner, playback)
    // Each new snapshot may be saved even if the previous write failed. The rejected
    // promise is preserved for callers, and background failures remain visible.
    saveChain = saveChain.then(save, save)
    void saveChain.catch(cause => {
      if (owner === playbackOwner) notice.value = `播放状态保存失败：${errorMessage(cause)}`
    })
    return saveChain
  }

  async function flushPlayback(): Promise<void> {
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = undefined; saveSettings() }
    await settingsSave
    await restorePromise
    if (restoreFailure) throw new Error(restoreFailure)
    if (audio.getAttribute('src')) syncTime()
    await persistPlayback()
  }

  function restorePlayback(owner: string): Promise<void> {
    reset()
    playbackOwner = owner
    const request = generation
    const version = revision
    restorePromise = (async () => {
      try {
        const playback = await api.playback(owner)
        if (request !== generation || version !== revision || owner !== playbackOwner || !playback) return
        queue.value = playback.queue
        current.value = playback.currentMid === null ? null : playback.queue.find(track => track.mid === playback.currentMid)!
        nextMid.value = playback.nextMid
        time.value = Math.round(playback.position * 1000)
        checkpoint = time.value
        duration.value = current.value ? current.value.duration : 0
        repeat.value = playback.repeat
        shuffle.value = playback.shuffle
        restored.value = current.value !== null
        if (current.value) { void loadLyrics(current.value, request); void loadTrackOffset(current.value, request) }
      } catch (cause) {
        if (request === generation && version === revision && owner === playbackOwner) {
          restoreFailure = errorMessage(cause)
          notice.value = `播放状态恢复失败：${restoreFailure}`
        }
      }
    })()
    return restorePromise
  }

  function bindAudio(element: HTMLAudioElement) {
  element.addEventListener('playing', () => {
    if (element !== audio) return;
    playing.value = true
    buffering.value = false
    playbackError.value = ''
    updateFrame()
    if (current.value && !recorded) {
      recorded = true
      api.record(current.value).catch(cause => { notice.value = `播放记录保存失败：${errorMessage(cause)}` })
    }
  })
  element.addEventListener('pause', () => {
    if (element !== audio) return;
    playing.value = false
    updateFrame()
    if (current.value) void persistPlayback()
  })
  element.addEventListener('timeupdate', () => {
    if (element !== audio) return;
    syncTime()
    void prepareNext()
    if (Math.abs(time.value - checkpoint) >= 5000) {
      checkpoint = time.value
      void persistPlayback()
    }
  })
  element.addEventListener('loadedmetadata', () => {
    if (element !== audio) return;
    if (Number.isFinite(audio.duration)) duration.value = audio.duration
    if (resumePosition > 0) {
      audio.currentTime = Math.min(resumePosition, duration.value)
      resumePosition = 0
      syncTime()
    }
  })
  element.addEventListener('durationchange', () => {
    if (element !== audio) return; if (Number.isFinite(audio.duration)) duration.value = audio.duration })
  element.addEventListener('waiting', () => {
    if (element !== audio) return; buffering.value = true })
  element.addEventListener('canplay', () => {
    if (element !== audio) return; buffering.value = false })
  element.addEventListener('error', () => {
    if (element !== audio) {
      if (prepared?.media === element) {
        prepared = null; element.removeAttribute('src'); element.load()
        preloadState.value = 'error'; preloadError.value = '下一首音频预加载失败'
      }
      return
    }
    if (!audio.getAttribute('src')) return
    const messages: Record<number, string> = {
      1: '音频加载已中断，请重试。', 2: '音频下载失败，请检查网络后重试。',
      3: '系统无法解码这首歌的音频。', 4: '播放地址不可用或已过期，请重试。',
    }
    playbackError.value = audio.error ? messages[audio.error.code] : '歌曲播放失败，请重试。'
    sourceState.value = 'error'
    playing.value = false
    buffering.value = false
    updateFrame()
  })
  element.addEventListener('ended', () => {
    if (element !== audio) return;
    if (sleepAfterTrack.value) { pause(); setSleepTimer(null); notice.value = '本首已播完，已停止播放。'; return }
    if (repeat.value === 'one') { seek(0); void toggle() }
    else void next(true)
  })

  }
  bindAudio(audio)
  watch([queue, current, nextMid, shuffle, repeat], planNext, { flush: 'sync' })
  watch([() => settings.value.preloadNext, () => settings.value.audioQuality], planNext)

  async function initialize() {
    if (!desktop) return
    try { settings.value = await api.settings(); audio.volume = settings.value.volume }
    catch (cause) { notice.value = errorMessage(cause) }
  }

  async function loadTrackOffset(track: Track, request: number) {
    trackOffset.value = 0; trackOffsetError.value = ''
    if (!desktop) return
    try { const value = await api.lyricOffset(track.mid); if (request === generation) trackOffset.value = value }
    catch (cause) { if (request === generation) trackOffsetError.value = errorMessage(cause) }
  }
  async function updateTrackOffset(value: number) {
    if (!current.value) return
    const track = current.value, request = generation
    try { await api.saveLyricOffset(track.mid, value); if (request === generation) { trackOffset.value = value; trackOffsetError.value = '' } }
    catch (cause) { if (request === generation) trackOffsetError.value = errorMessage(cause) }
  }
  async function updateShortcuts(value: ShortcutSettings) {
    clearTimeout(saveTimer)
    saveTimer = undefined
    await saveSettings({ ...settings.value, shortcuts: value })
    settings.value = { ...settings.value, shortcuts: value }
  }
  async function loadLyrics(track: Track, request: number) {
    lyricState.value = 'loading'
    lyricError.value = ''
    try {
      const raw = await api.lyrics(track)
      if (request !== generation) return
      const result = await parseInWorker(raw, track.duration)
      if (request !== generation) return
      lyricLines.value = result.lines
      lyricMode.value = result.mode
      lyricWarnings.value = result.warnings
      lyricState.value = 'ready'
    } catch (cause) {
      if (request !== generation) return
      lyricError.value = errorMessage(cause)
      lyricState.value = 'error'
    }
  }

  async function play(track: Track, items?: Track[], startAt = 0) {
    const warm = prepared && prepared.mid === track.mid && prepared.request === generation ? prepared : null
    if (warm) prepared = null
    cancelPreparation()
    const request = ++generation
    audio.pause()
    audio.removeAttribute('src')
    audio.load()
    if (warm) { audio = warm.media; audio.muted = false }
    playing.value = false
    if (items) {
      queue.value = [...new Map(items.map(item => [item.mid, item])).values()]
      nextMid.value = null
    }
    if (!queue.value.some(item => item.mid === track.mid)) queue.value = [...queue.value, track]
    if (nextMid.value === track.mid) nextMid.value = null
    current.value = track
    time.value = Math.round(startAt * 1000)
    checkpoint = time.value
    resumePosition = startAt
    restored.value = false
    duration.value = track.duration
    quality.value = ''
    localSource.value = false
    playbackError.value = ''
    sourceState.value = 'loading'
    buffering.value = true
    recorded = false
    lyricLines.value = []
    lyricMode.value = 'none'
    lyricWarnings.value = []
    void persistPlayback()
    void loadLyrics(track, request)
    void loadTrackOffset(track, request)
    try {
      const source = warm ? warm.source : await api.source(track)
      if (request !== generation) return
      quality.value = source.quality
      localSource.value = source.local === true
      if (!warm) audio.src = source.url
      else if (Number.isFinite(audio.duration)) { duration.value = audio.duration; audio.currentTime = Math.min(startAt, audio.duration); resumePosition = 0 }
      audio.volume = settings.value.volume
      await audio.play()
      if (request !== generation) return
      sourceState.value = 'ready'
      if ('mediaSession' in navigator) {
        navigator.mediaSession.metadata = new MediaMetadata({ title: track.title, artist: track.artist, album: track.album,
          artwork: track.cover ? [{ src: track.cover, sizes: '500x500', type: 'image/jpeg' }] : [] })
      }
    } catch (cause) {
      if (request !== generation) return
      playbackError.value = errorMessage(cause)
      sourceState.value = 'error'
      buffering.value = false
    }
  }

  async function toggle() {
    if (!current.value) return
    if (playing.value) audio.pause()
    else if (sourceState.value === 'error' || sourceState.value === 'idle') await play(current.value, undefined, time.value / 1000)
    else if (sourceState.value === 'ready') {
      try { await audio.play() } catch (cause) { playbackError.value = errorMessage(cause) }
    }
  }

  function seek(seconds: number) {
    if (!audio.getAttribute('src') || !Number.isFinite(seconds)) return
    audio.currentTime = Math.max(0, Math.min(seconds, duration.value))
    syncTime()
    checkpoint = time.value
    void persistPlayback()
  }

  async function next(automatic = false) {
    if (!queue.value.length) return
    let index = currentIndex.value + 1
    if (nextMid.value) index = queue.value.findIndex(track => track.mid === nextMid.value)
    else if (upcoming.value) index = queue.value.findIndex(track => track.mid === upcoming.value!.mid)
    if (index >= queue.value.length) {
      if (automatic && repeat.value === 'off') return
      index = 0
    }
    await play(queue.value[index])
  }
  async function previous() {
    if (time.value > 3000) {
      if (audio.getAttribute('src')) seek(0)
      else { time.value = 0; checkpoint = 0; void persistPlayback() }
      return
    }
    if (queue.value.length) await play(queue.value[(currentIndex.value - 1 + queue.value.length) % queue.value.length])
  }
  function cycleRepeat() {
    repeat.value = repeat.value === 'off' ? 'all' : repeat.value === 'all' ? 'one' : 'off'
    void persistPlayback()
  }
  function setShuffle(value: boolean) { shuffle.value = value; void persistPlayback() }
  function toggleShuffle() { setShuffle(!shuffle.value) }

  function enqueue(track: Track, next = false) {
    if (next && current.value?.mid === track.mid) {
      notice.value = '这首歌已是当前歌曲。'
      return
    }
    if (next) {
      const items = queue.value.filter(item => item.mid !== track.mid)
      const index = items.findIndex(item => item.mid === current.value?.mid)
      items.splice(index + 1, 0, track)
      queue.value = items
      nextMid.value = current.value ? track.mid : null
    } else {
      if (queue.value.some(item => item.mid === track.mid)) {
        notice.value = '这首歌已在播放队列中。'
        return
      }
      queue.value = [...queue.value, track]
    }
    if (!current.value) {
      current.value = track
      duration.value = track.duration
    }
    notice.value = next ? `已设为下一首：${track.title}` : `已加入播放队列：${track.title}`
    void persistPlayback()
  }

  function removeFromQueue(mid: string) {
    if (mid === current.value?.mid) return
    queue.value = queue.value.filter(track => track.mid !== mid)
    if (nextMid.value === mid) nextMid.value = null
    void persistPlayback()
  }

  function moveInQueue(mid: string, step: -1 | 1) {
    const index = queue.value.findIndex(track => track.mid === mid)
    const target = index + step
    if (index < 0 || target < 0 || target >= queue.value.length) return
    const items = [...queue.value]
    ;[items[index], items[target]] = [items[target], items[index]]
    queue.value = items
    nextMid.value = null
    void persistPlayback()
  }

  function clearQueue() {
    queue.value = current.value ? [current.value] : []
    nextMid.value = null
    void persistPlayback()
  }

  function releaseDownload(mid: string) {
    if (current.value?.mid !== mid || !localSource.value) return
    generation++
    cancelPreparation()
    audio.pause()
    audio.removeAttribute('src')
    audio.load()
    localSource.value = false
    quality.value = ''
    playing.value = false
    buffering.value = false
    sourceState.value = 'idle'
    void persistPlayback()
  }

  function saveSettings(snapshot = settings.value) {
    settingsSave = settingsSave.then(() => api.saveSettings(snapshot), () => api.saveSettings(snapshot))
    void settingsSave.catch(cause => { notice.value = `设置保存失败：${errorMessage(cause)}` })
    return settingsSave
  }
  function updateSettings(values: Partial<AppSettings>) {
    settings.value = { ...settings.value, ...values }
    audio.volume = settings.value.volume
    clearTimeout(saveTimer)
    if (desktop) saveTimer = setTimeout(() => {
      saveTimer = undefined
      void saveSettings()
    }, 250)
  }

  function reset() {
    setSleepTimer(null)
    generation++
    cancelPreparation()
    trackOffset.value = 0; trackOffsetError.value = ''
    playbackOwner = null
    restorePromise = Promise.resolve()
    restoreFailure = ''
    audio.pause()
    audio.removeAttribute('src')
    audio.load()
    cancelAnimationFrame(frame)
    current.value = null
    queue.value = []
    lyricLines.value = []
    time.value = 0
    duration.value = 0
    playing.value = false
    buffering.value = false
    sourceState.value = 'idle'
    playbackError.value = ''
    lyricState.value = 'idle'
    lyricError.value = ''
    lyricWarnings.value = []
    quality.value = ''
    localSource.value = false
    lyricMode.value = 'none'
    nextMid.value = null
    repeat.value = 'off'
    shuffle.value = false
    restored.value = false
    resumePosition = 0
    checkpoint = 0
    if ('mediaSession' in navigator) navigator.mediaSession.metadata = null
  }

  if ('mediaSession' in navigator) {
    navigator.mediaSession.setActionHandler('play', () => { if (!playing.value) void toggle() })
    navigator.mediaSession.setActionHandler('pause', () => { if (playing.value) audio.pause() })
    navigator.mediaSession.setActionHandler('previoustrack', () => { void previous() })
    navigator.mediaSession.setActionHandler('nexttrack', () => { void next() })
    navigator.mediaSession.setActionHandler('seekto', event => { if (event.seekTime !== undefined) seek(event.seekTime) })
  }

  return { current, queue, playing, buffering, time, duration, quality, sourceState, playbackError, lyricState,
    lyricError, lyricWarnings, lyricLines, lyricMode, lyricTime, notice, repeat, shuffle, settings, progress,
    trackOffset, trackOffsetError, updateTrackOffset, updateShortcuts, preloadState, preloadError, upcoming,
    visualActive, currentIndex, nextMid, restored, sleepDeadline, sleepAfterTrack, sleepRemaining, setSleepTimer, checkSleepTimer, pause,
    initialize, play, toggle, seek, next, previous, cycleRepeat,
    setShuffle, toggleShuffle, enqueue, removeFromQueue, moveInQueue, clearQueue, localSource, releaseDownload, restorePlayback, flushPlayback, updateSettings, reset,
    setVisualActive, readLyricTime, reloadLyrics: () => { if (current.value) void loadLyrics(current.value, generation) } }
})
