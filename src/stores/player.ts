import { defineStore } from 'pinia'
import { computed, ref, shallowRef, watch } from 'vue'
import { api, desktop, errorMessage } from '../lib/api'
import { parseInWorker } from '../lib/lyric-client'
import type { ParsedLyrics } from '../lib/lyrics'
import { defaultSettings } from '../lib/preferences'
import type { QqAutoMix, QqMixPlan } from '../lib/qq-automix'
import type { LyricLine } from '@applemusic-like-lyrics/core'
import type { AppSettings, LoadState, PlaybackSource, PlaybackState, ShortcutSettings, Track } from '../types'

type LyricsRequest = Promise<ParsedLyrics | { error: string }>

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
  const mixState = ref<LoadState>('idle'), mixError = ref(''), mixMode = ref('')
  let mixer: QqAutoMix | null = null
  let startingBlend: HTMLAudioElement | null = null
  let prepareTask: Promise<void> | undefined
  async function getMixer(request: number) {
    if (!mixer) {
      const { QqAutoMix } = await import('../lib/qq-automix')
      if (request !== generation) throw new Error('播放请求已变化')
      mixer ??= new QqAutoMix()
    }
    return mixer
  }
  function releaseMedia(media: HTMLAudioElement) {
    if (mixer?.release(media)) return
    media.pause(); media.removeAttribute('src'); media.load()
  }
  function stopBlend() {
    mixer?.finish(); mixMode.value = ''
  }
  let audio = new Audio()
  audio.preload = 'metadata'
  let generation = 0
  let frame = 0
  let lastUiFrame = 0
  let recorded = false
  let seekTarget: { seconds: number; until: number } | null = null
  let prepared: {
    mid: string; media: HTMLAudioElement; source: PlaybackSource; request: number
    lyrics?: LyricsRequest
    plan: QqMixPlan | null
  } | null = null
  let preparingFor = ''
  let preparation = 0
  function cancelPreparation() {
    preparation++
    if (prepared) releaseMedia(prepared.media)
    prepareTask = undefined
    startingBlend = null
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
  function prepareNext() {
    if (prepareTask) return prepareTask
    const track = upcoming.value, from = current.value, outgoing = audio
    if (!playing.value || buffering.value || !from || !track || repeat.value === 'one' || preparingFor || !outgoing.getAttribute('src')) return
    const graph = settings.value.smartMix && mixer?.has(outgoing) ? mixer : null
    if (mixMode.value || (!graph && !settings.value.preloadNext)) return
    const orderedAlbum = from.albumMid && from.albumMid === track.albumMid && !shuffle.value && !nextMid.value
      && queue.value[currentIndex.value + 1]?.mid === track.mid
    const request = generation, version = ++preparation
    preparingFor = track.mid
    preloadState.value = 'loading'
    preloadError.value = ''
    prepareTask = prepare(track, from, graph, Boolean(graph && !orderedAlbum), request, version)
    return prepareTask
  }

  async function prepare(track: Track, from: Track, graph: QqAutoMix | null, requestPlan: boolean, request: number, version: number) {
    let media: HTMLAudioElement | undefined
    try {
      const mixRequest = requestPlan ? api.qqMixPlan(from, track).then(
        plan => ({ plan }),
        cause => ({ error: errorMessage(cause) }),
      ) : null
      const source = await api.source(track)
      if (request !== generation || version !== preparation) return
      media = new Audio()
      media.preload = 'auto'
      media.muted = true
      bindAudio(media)
      if (graph) graph.prepare(media, source.url)
      else {
        media.src = source.url
        media.load()
      }
      const warm: NonNullable<typeof prepared> = { mid: track.mid, media, source, request, plan: null }
      prepared = warm
      preloadState.value = 'ready'
      if (!mixRequest) return
      const result = await mixRequest
      if (prepared !== warm) return
      if ('error' in result) {
        reportMixError(`QQ 自动衔接不可用：${result.error}`)
        return
      }
      const { plan } = result
      if (plan.out + plan.duration > duration.value + 0.25
        || plan.input + plan.duration * plan.rate > track.duration + 1) {
        reportMixError('QQ 返回的衔接点超出音频时长。')
        return
      }
      if (plan.input > 0) media.currentTime = plan.input
      warm.plan = plan
      warm.lyrics = api.lyrics(track).then(raw => parseInWorker(raw, track.duration)).catch(cause => ({ error: errorMessage(cause) }))
      mixState.value = 'ready'
      mixError.value = ''
    } catch (cause) {
      if (media) releaseMedia(media)
      if (request !== generation || version !== preparation) return
      preloadState.value = 'error'
      preloadError.value = errorMessage(cause)
      if (graph) reportMixError(`QQ 自动衔接准备失败：${preloadError.value}`)
    } finally {
      if (version === preparation) prepareTask = undefined
    }
  }
  function reportMixError(message: string) {
    mixError.value = message
    mixState.value = 'error'
    notice.value = message
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
    stopBlend()
    audio.pause()
    cancelPreparation()
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

  function activeSeekTarget() {
    if (!seekTarget) return null
    if (performance.now() >= seekTarget.until || Math.abs(audio.currentTime - seekTarget.seconds) < 0.08) {
      seekTarget = null
      return null
    }
    return seekTarget.seconds * 1000
  }
  function syncTime() {
    const target = activeSeekTarget()
    if (target !== null) { time.value = Math.round(target); return }
    if (audio.getAttribute('src')) time.value = Math.round(audio.currentTime * 1000)
  }
  // Lyrics read the media clock inside their own animation frame. Keeping that
  // clock out of Vue avoids rendering the whole player for every lyric frame.
  function readLyricTime() {
    const target = activeSeekTarget()
    const position = target ?? (audio.getAttribute('src') ? audio.currentTime * 1000 : time.value)
    return Math.max(0, Math.round(position + settings.value.lyricOffset + trackOffset.value))
  }
  function tick(now: number) {
    checkNaturalBlend()
    if (now - lastUiFrame >= 100) {
      syncTime()
      lastUiFrame = now
    }
    if (playing.value && visualActive.value) frame = requestAnimationFrame(tick)
  }
  function checkNaturalBlend() {
    const track = upcoming.value, plan = prepared?.plan
    if (startingBlend || sleepAfterTrack.value || !track || !plan || !playing.value) return
    const elapsed = audio.currentTime - plan.out
    if (elapsed >= 0 && elapsed < Math.min(1, plan.duration * 0.35)) void blendTo(track, plan)
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
    if (element === audio) onPlaying()
  })
  element.addEventListener('pause', () => {
    if (element !== audio) return;
    playing.value = false
    const graph = mixer
    if (graph?.context.state === 'running') void graph.context.suspend().catch(cause => { if (graph === mixer) mixError.value = errorMessage(cause) })
    updateFrame()
    if (current.value) void persistPlayback()
  })
  element.addEventListener('seeked', () => {
    if (element !== audio) return;
    seekTarget = null
    syncTime()
  })
  element.addEventListener('timeupdate', () => {
    if (element !== audio) return;
    syncTime()
    void prepareNext()
    checkNaturalBlend()
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
        cancelPreparation()
        preloadState.value = 'error'; preloadError.value = '下一首音频预加载失败'
        if (settings.value.smartMix) reportMixError(preloadError.value)
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
  async function loadLyrics(track: Track, request: number, cached?: LyricsRequest) {
    lyricState.value = 'loading'
    lyricError.value = ''
    try {
      let result
      if (cached) result = await cached
      else {
        const raw = await api.lyrics(track)
        if (request !== generation) return
        result = await parseInWorker(raw, track.duration)
      }
      if (request !== generation) return
      if ('error' in result) throw new Error(result.error)
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

  function onPlaying() {
    playing.value = true
    buffering.value = false
    playbackError.value = ''
    updateFrame()
    if (current.value && !recorded) {
      recorded = true
      api.record(current.value).catch(cause => { notice.value = `播放记录保存失败：${errorMessage(cause)}` })
    }
  }

  function selectTrack(track: Track, items: Track[] | undefined, startAt: number, request: number, lyrics?: LyricsRequest) {
    seekTarget = null
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
    void loadLyrics(track, request, lyrics)
    void loadTrackOffset(track, request)
  }

  function ready(track: Track, source: PlaybackSource) {
    quality.value = source.quality
    localSource.value = source.local === true
    sourceState.value = 'ready'
    if ('mediaSession' in navigator) {
      navigator.mediaSession.metadata = new MediaMetadata({ title: track.title, artist: track.artist, album: track.album,
        artwork: track.cover ? [{ src: track.cover, sizes: '500x500', type: 'image/jpeg' }] : [] })
    }
  }

  async function blendTo(track: Track, plan: QqMixPlan) {
    const warm = prepared, graph = mixer, outgoing = audio
    if (!settings.value.smartMix || !graph || !warm || warm.mid !== track.mid || warm.request !== generation || !playing.value || outgoing.paused || outgoing.ended || startingBlend) return false
    const media = warm.media
    startingBlend = media
    try {
      if (plan.out + plan.duration > duration.value + 0.25
        || plan.input + plan.duration * plan.rate > (Number.isFinite(media.duration) ? media.duration : track.duration) + 0.25) {
        throw new Error('QQ 返回的衔接点超出音频时长。')
      }
      // Do not change the incoming element's playbackRate: WebKit reconfigures
      // its pitch-preserving audio pipeline and stalls for ~100 ms at the cut.
      // Keep QQ's cut points and filter/gain curves, but crossfade at normal speed.
      const available = (Number.isFinite(media.duration) ? media.duration : track.duration) - plan.input
      const crossfadeDuration = Math.min(plan.duration, available)
      if (!(crossfadeDuration > 0.25)) throw new Error('下一首剩余音频不足，无法自动衔接。')
      const crossfadePlan: QqMixPlan = { ...plan, rate: 1, duration: crossfadeDuration }
      graph.silence(media)
      media.playbackRate = 1
      const elapsed = () => Math.max(0, outgoing.currentTime - plan.out)
      const entry = plan.input + elapsed()
      if (Math.abs(entry - media.currentTime) > 0.065) media.currentTime = entry
      media.muted = false
      media.volume = settings.value.volume
      await media.play()
      if (prepared !== warm) return false
      const played = elapsed()
      if (sleepAfterTrack.value || outgoing.ended || played >= Math.min(1, crossfadePlan.duration * 0.35)) {
        cancelPreparation()
        return false
      }
      const aligned = plan.input + played
      if (Math.abs(aligned - media.currentTime) > 0.065) media.currentTime = aligned
      graph.fade(outgoing, media, crossfadePlan, played, () => {
        mixMode.value = ''
        void prepareNext()
      })
      prepared = null
      cancelPreparation()
      audio = media
      mixMode.value = 'QQ 自动衔接'
      selectTrack(track, undefined, media.currentTime, ++generation, warm.lyrics)
      resumePosition = 0
      if (Number.isFinite(media.duration)) duration.value = media.duration
      onPlaying()
      ready(track, warm.source)
      return true
    } catch (cause) {
      if (prepared === warm) {
        cancelPreparation()
        reportMixError(`QQ 自动衔接失败：${errorMessage(cause)}`)
      }
      return false
    } finally {
      if (startingBlend === media) startingBlend = null
    }
  }

  async function play(track: Track, items?: Track[], startAt = 0) {
    const warm = prepared && prepared.mid === track.mid && prepared.request === generation && !startingBlend ? prepared : null
    if (warm) prepared = null
    cancelPreparation()
    const request = ++generation
    stopBlend()
    releaseMedia(audio)
    if (warm) { audio = warm.media; audio.muted = false }
    selectTrack(track, items, startAt, request, warm?.lyrics)
    try {
      const source = warm ? warm.source : await api.source(track)
      if (request !== generation) return
      quality.value = source.quality
      localSource.value = source.local === true
      if (!warm && settings.value.smartMix) {
        mixState.value = 'loading'
        mixError.value = ''
        const graph = await getMixer(request)
        if (request !== generation) return
        // A MediaElementAudioSource cannot be rebound after graph destruction.
        audio = new Audio()
        bindAudio(audio)
        graph.prepare(audio, source.url)
        mixState.value = 'ready'
      } else if (!warm) {
        audio.src = source.url
      } else {
        if (Number.isFinite(audio.duration)) duration.value = audio.duration
        audio.currentTime = Math.min(startAt, duration.value)
        resumePosition = 0
      }
      audio.volume = settings.value.volume
      if (mixer?.has(audio)) await mixer.resume()
      await audio.play()
      if (request !== generation) return
      ready(track, source)
    } catch (cause) {
      if (request !== generation) return
      stopBlend(); releaseMedia(audio)
      if (settings.value.smartMix) { mixState.value = 'error'; mixError.value = errorMessage(cause) }
      playbackError.value = errorMessage(cause)
      sourceState.value = 'error'
      buffering.value = false
    }
  }

  async function toggle() {
    if (!current.value) return
    if (playing.value) pause()
    else if (sourceState.value === 'error' || sourceState.value === 'idle') await play(current.value, undefined, time.value / 1000)
    else if (sourceState.value === 'ready') {
      try { if (mixer?.has(audio)) await mixer.resume(); await audio.play() } catch (cause) { playbackError.value = errorMessage(cause) }
    }
  }

  function seek(seconds: number) {
    if (!audio.getAttribute('src') || !Number.isFinite(seconds)) return
    if (startingBlend) cancelPreparation()
    stopBlend()
    const target = Math.max(0, Math.min(seconds, duration.value))
    // WebKit applies currentTime asynchronously. Keep the UI and lyric clock on
    // the requested position until the media reports the seeked position.
    seekTarget = { seconds: target, until: performance.now() + 1_200 }
    audio.currentTime = target
    time.value = Math.round(target * 1000)
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
    const track = queue.value[index]
    await play(track)
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
    const smartChanged = values.smartMix !== undefined && values.smartMix !== settings.value.smartMix
    settings.value = { ...settings.value, ...values }
    if (smartChanged) {
      cancelPreparation(); stopBlend()
      if (!settings.value.smartMix && mixer) {
        const old = mixer; mixer = null
        generation++; sourceState.value = 'idle'; playing.value = false; buffering.value = false
        void old.dispose().catch(cause => { mixError.value = errorMessage(cause) })
        audio = new Audio(); bindAudio(audio)
      }
      mixState.value = 'idle'; mixError.value = ''
      notice.value = '自动衔接设置将在下一次播放时应用。'
    }
    audio.volume = settings.value.volume
    clearTimeout(saveTimer)
    if (desktop) saveTimer = setTimeout(() => {
      saveTimer = undefined
      void saveSettings()
    }, 250)
  }

  function reset() {
    seekTarget = null
    setSleepTimer(null)
    stopBlend()
    if (mixer) { const old = mixer; mixer = null; void old.dispose().catch(cause => { mixError.value = errorMessage(cause) }); audio = new Audio(); bindAudio(audio) }
    mixState.value = 'idle'; mixError.value = ''
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
    navigator.mediaSession.setActionHandler('pause', () => { if (playing.value) pause() })
    navigator.mediaSession.setActionHandler('previoustrack', () => { void previous() })
    navigator.mediaSession.setActionHandler('nexttrack', () => { void next() })
    navigator.mediaSession.setActionHandler('seekto', event => { if (event.seekTime !== undefined) seek(event.seekTime) })
  }

  return { current, queue, playing, buffering, time, duration, quality, sourceState, playbackError, lyricState,
    lyricError, lyricWarnings, lyricLines, lyricMode, lyricTime, notice, repeat, shuffle, settings, progress,
    trackOffset, trackOffsetError, updateTrackOffset, updateShortcuts, preloadState, preloadError, upcoming,
    visualActive, currentIndex, nextMid, restored, sleepDeadline, sleepAfterTrack, sleepRemaining, setSleepTimer, checkSleepTimer, pause,
    mixState, mixError, mixMode, initialize, play, toggle, seek, next, previous, cycleRepeat,
    setShuffle, toggleShuffle, enqueue, removeFromQueue, moveInQueue, clearQueue, localSource, releaseDownload, restorePlayback, flushPlayback, updateSettings, reset,
    setVisualActive, readLyricTime, reloadLyrics: () => { if (current.value) void loadLyrics(current.value, generation) } }
})
