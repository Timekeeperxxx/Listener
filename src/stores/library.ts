import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'
import { api, desktop, errorMessage } from '../lib/api'
import type { LoadState, LoginQr, Playlist, Profile, Track } from '../types'

export const useLibraryStore = defineStore('library', () => {
  const profile = shallowRef<Profile | null>(null)
  const playlists = shallowRef<Playlist[]>([])
  const tracks = shallowRef<Track[]>([])
  const recentTracks = shallowRef<Track[]>([])
  const recentState = ref<LoadState>('idle')
  const recentError = ref('')
  const selected = shallowRef<Playlist | null>(null)
  const view = ref<'library' | 'playlist' | 'history' | 'search' | 'downloads' | 'artists' | 'albums' | 'artist' | 'album'>('library')
  const artistMid = ref('')
  const albumId = ref('')
  const catalogTrail: { view: typeof view.value; artist: string; album: string }[] = []
  function showCatalog(target: 'artists' | 'albums' | 'artist' | 'album', id = '') {
    if (view.value === target && (target === 'artist' ? artistMid.value === id : target === 'album' ? albumId.value === id : true)) return
    navigation++
    loadingMore.value = false
    catalogTrail.push({ view: view.value, artist: artistMid.value, album: albumId.value })
    view.value = target
    if (target === 'artist') artistMid.value = id
    if (target === 'album') albumId.value = id
    error.value = ''
  }
  function backCatalog() {
    navigation++
    const previous = catalogTrail.pop()
    view.value = previous ? previous.view : 'library'
    artistMid.value = previous ? previous.artist : ''
    albumId.value = previous ? previous.album : ''
  }
  const loginKind = ref<'qq' | 'wx' | 'mobile'>('qq')
  const mutationBusy = ref(false)
  const mutationError = ref('')
  const likedMids = shallowRef<Set<string>>(new Set())
  const likedState = ref<LoadState>('idle')
  const likingMid = ref<string | null>(null)
  const favoriteError = ref('')
  const state = ref<LoadState>('idle')
  const libraryState = ref<LoadState>('idle')
  const sessionLoading = ref(desktop)
  const error = ref('')
  const libraryError = ref('')
  const warnings = ref<string[]>([])
  const loginError = ref('')
  const loginWaiting = ref(false)
  const loginOpen = ref(false)
  const loginQr = shallowRef<LoginQr | null>(null)
  const loginStatus = ref<'loading' | 'waiting' | 'scanned' | 'expired' | 'refused' | 'error'>('loading')
  const total = ref(0)
  const offset = ref(0)
  const hasMore = ref(false)
  const loadingMore = ref(false)
  const query = ref('')
  const searchPage = ref(1)
  let navigation = 0
  let accountGeneration = 0
  let loginAttempt = 0
  let loginTimer: ReturnType<typeof setTimeout> | undefined
  let likedRequest: Promise<void> | null = null
  let likedVersion = 0

  async function initialize() {
    if (!desktop) return
    try {
      profile.value = await api.session()
      if (profile.value?.offline) showDownloads()
      else if (profile.value) await refresh()
    } catch (cause) { loginError.value = errorMessage(cause) }
    finally { sessionLoading.value = false }
  }

  async function refresh() {
    const generation = accountGeneration
    libraryState.value = 'loading'
    libraryError.value = ''
    try {
      const result = await api.library()
      if (generation !== accountGeneration) return
      playlists.value = result.playlists
      warnings.value = result.warnings
      libraryState.value = 'ready'
      void loadLiked(true)
    } catch (cause) {
      if (generation !== accountGeneration) return
      libraryError.value = errorMessage(cause)
      libraryState.value = 'error'
    }
  }

  function clearAccount() {
    accountGeneration++
    navigation++
    playlists.value = []
    tracks.value = []
    artistMid.value = ''
    albumId.value = ''
    catalogTrail.length = 0
    recentTracks.value = []
    recentState.value = 'idle'
    recentError.value = ''
    selected.value = null
    total.value = 0
    offset.value = 0
    hasMore.value = false
    loadingMore.value = false
    libraryState.value = 'idle'
    state.value = 'idle'
    error.value = ''
    warnings.value = []
    likedMids.value = new Set()
    likedState.value = 'idle'
    likedRequest = null
    likingMid.value = null
    favoriteError.value = ''
    mutationBusy.value = false
    mutationError.value = ''
    view.value = 'library'
  }

  async function connect() {
    const attempt = ++loginAttempt
    clearTimeout(loginTimer)
    loginError.value = ''
    loginOpen.value = true
    loginStatus.value = 'loading'
    loginQr.value = null
    loginWaiting.value = true
    try {
      const qr = await api.openLogin(loginKind.value)
      if (attempt !== loginAttempt) return
      loginQr.value = qr
      loginStatus.value = 'waiting'
      const poll = async () => {
        try {
          const result = await api.pollLogin(qr.id)
          if (attempt !== loginAttempt) return
          if (result.status === 'complete') {
            clearAccount()
            profile.value = result.profile
            loginWaiting.value = false
            loginOpen.value = false
            loginQr.value = null
            await refresh()
          } else if (result.status === 'closed') {
            loginWaiting.value = false
            loginOpen.value = false
          } else if (result.status === 'expired' || result.status === 'refused') {
            loginStatus.value = result.status
            loginWaiting.value = false
          } else {
            loginStatus.value = result.status
            loginTimer = setTimeout(poll, 1500)
          }
        } catch (cause) {
          if (attempt !== loginAttempt) return
          loginError.value = errorMessage(cause)
          loginWaiting.value = false
          loginStatus.value = 'error'
        }
      }
      loginTimer = setTimeout(poll, 1500)
    } catch (cause) {
      if (attempt !== loginAttempt) return
      loginError.value = errorMessage(cause)
      loginWaiting.value = false
      loginStatus.value = 'error'
    }
  }

  async function cancelLogin() {
    loginAttempt++
    clearTimeout(loginTimer)
    loginWaiting.value = false
    loginOpen.value = false
    loginQr.value = null
    try { await api.cancelLogin() } catch (cause) { loginError.value = errorMessage(cause) }
  }

  async function logout() {
    loginAttempt++
    clearTimeout(loginTimer)
    await api.logout()
    clearAccount()
    profile.value = null
    loginError.value = ''
    loginWaiting.value = false
    loginOpen.value = false
    loginQr.value = null
  }

  function showLibrary() { navigation++; catalogTrail.length = 0; view.value = 'library'; error.value = '' }
  function showDownloads() { navigation++; view.value = 'downloads'; error.value = '' }
  function setLoginKind(kind: 'qq' | 'wx' | 'mobile') { loginKind.value = kind; if (loginOpen.value) void connect() }

  function loadLiked(force = false): Promise<void> {
    if (likedRequest) return likedRequest
    if (!force && likedState.value === 'ready') return Promise.resolve()
    const generation = accountGeneration
    const version = likedVersion
    likedState.value = 'loading'
    favoriteError.value = ''
    likedRequest = (async () => {
      try {
        const mids = new Set<string>()
        let nextOffset = 0
        while (true) {
          const page = await api.playlist('liked', nextOffset)
          if (generation !== accountGeneration || version !== likedVersion) return
          page.tracks.forEach(track => mids.add(track.mid))
          if (!page.hasMore) break
          nextOffset = page.nextOffset
        }
        likedMids.value = mids
        likedState.value = 'ready'
      } catch (cause) {
        if (generation === accountGeneration && version === likedVersion) { favoriteError.value = errorMessage(cause); likedState.value = 'error' }
      } finally { if (generation === accountGeneration) likedRequest = null }
    })()
    return likedRequest
  }

  async function toggleLike(track: Track) {
    if (likingMid.value !== null || mutationBusy.value || !profile.value || profile.value.offline) return
    const generation = accountGeneration
    if (likedState.value !== 'ready') await loadLiked()
    if (generation !== accountGeneration || likedState.value !== 'ready') return
    const add = !likedMids.value.has(track.mid)
    likingMid.value = track.mid
    favoriteError.value = ''
    try {
      const success = await mutate(add ? 'add' : 'remove', 'liked', null, track)
      if (!success && generation === accountGeneration) {
        likedVersion++
        favoriteError.value = mutationError.value
        likedState.value = 'error'
      }
    } finally { if (generation === accountGeneration) likingMid.value = null }
  }

  async function mutate(action: 'create' | 'rename' | 'delete' | 'add' | 'remove', id: string | null, name: string | null, track: Track | null): Promise<boolean> {
    if (mutationBusy.value) return false
    const generation = accountGeneration
    const request = navigation
    mutationBusy.value = true
    mutationError.value = ''
    try {
      const result = await api.mutatePlaylist(action, id, name, track)
      if (generation !== accountGeneration) return false
      playlists.value = result.playlists
      warnings.value = result.warnings
      if (id === 'liked' && track) {
        likedVersion++
        const mids = new Set(likedMids.value)
        if (action === 'add') mids.add(track.mid)
        else mids.delete(track.mid)
        likedMids.value = mids
        likedState.value = 'ready'
      }
      if (request === navigation && view.value === 'playlist' && selected.value?.id === id) {
        const updated = result.playlists.find(playlist => playlist.id === id)
        if (updated && id === 'liked' && track) {
          if (action === 'remove') {
            const wasLoaded = tracks.value.some(item => item.mid === track.mid)
            tracks.value = tracks.value.filter(item => item.mid !== track.mid)
            if (wasLoaded) offset.value = Math.max(0, offset.value - 1)
          } else if (updated.count > selected.value.count && !tracks.value.some(item => item.mid === track.mid)) {
            tracks.value = [track, ...tracks.value]
            offset.value++
          }
          selected.value = updated
          total.value = updated.count
          hasMore.value = offset.value < updated.count
        }
        else if (updated) await openPlaylist(updated)
        else showLibrary()
      }
      return true
    } catch (cause) {
      if (generation === accountGeneration) mutationError.value = errorMessage(cause)
      return false
    } finally { if (generation === accountGeneration) mutationBusy.value = false }
  }

  async function loadRecent() {
    const generation = accountGeneration
    recentState.value = 'loading'
    recentError.value = ''
    try {
      const result = await api.history()
      if (generation !== accountGeneration) return
      recentTracks.value = result
      recentState.value = 'ready'
    } catch (cause) {
      if (generation !== accountGeneration) return
      recentError.value = errorMessage(cause)
      recentState.value = 'error'
    }
  }

  async function openPlaylist(playlist: Playlist) {
    const request = ++navigation
    view.value = 'playlist'
    selected.value = playlist
    tracks.value = []
    total.value = playlist.count
    offset.value = 0
    hasMore.value = false
    error.value = ''
    state.value = 'loading'
    loadingMore.value = false
    try {
      const page = await api.playlist(playlist.id, 0)
      if (request !== navigation) return
      tracks.value = page.tracks
      total.value = page.total
      offset.value = page.nextOffset
      hasMore.value = page.hasMore
      state.value = 'ready'
    } catch (cause) {
      if (request !== navigation) return
      error.value = errorMessage(cause)
      state.value = 'error'
    }
  }

  async function loadMore() {
    if (loadingMore.value || !hasMore.value) return
    const request = navigation
    loadingMore.value = true
    error.value = ''
    try {
      if (view.value === 'playlist' && selected.value) {
        const page = await api.playlist(selected.value.id, offset.value)
        if (request !== navigation) return
        const mids = new Set(tracks.value.map(t => t.mid))
        tracks.value = [...tracks.value, ...page.tracks.filter(t => !mids.has(t.mid))]
        offset.value = page.nextOffset
        hasMore.value = page.hasMore
        total.value = page.total
      } else if (view.value === 'search') {
        const page = await api.search(query.value, searchPage.value + 1)
        if (request !== navigation) return
        const mids = new Set(tracks.value.map(t => t.mid))
        tracks.value = [...tracks.value, ...page.filter(t => !mids.has(t.mid))]
        searchPage.value++
        hasMore.value = page.length === 30
      }
    } catch (cause) {
      if (request === navigation) error.value = errorMessage(cause)
    } finally { if (request === navigation) loadingMore.value = false }
  }

  async function completePlaylist(): Promise<Track[]> {
    if (loadingMore.value) throw new Error('歌曲正在加载，请稍后再播放全部。')
    const request = navigation
    while (hasMore.value) {
      await loadMore()
      if (request !== navigation) throw new Error('当前歌单已变化。')
      if (error.value) throw new Error(error.value)
    }
    return tracks.value
  }

  function search(keyword: string) {
    navigation++
    view.value = 'search'
    query.value = keyword.trim()
    tracks.value = []
    loadingMore.value = false
    error.value = ''
  }

  async function history() {
    const request = ++navigation
    view.value = 'history'
    state.value = 'loading'
    tracks.value = []
    error.value = ''
    hasMore.value = false
    loadingMore.value = false
    try {
      const result = await api.history()
      if (request !== navigation) return
      tracks.value = result
      recentTracks.value = result
      recentState.value = 'ready'
      recentError.value = ''
      total.value = result.length
      state.value = 'ready'
    } catch (cause) {
      if (request !== navigation) return
      error.value = errorMessage(cause)
      state.value = 'error'
    }
  }

  return { profile, playlists, tracks, recentTracks, recentState, recentError, selected, view, state, libraryState, sessionLoading, error,
    libraryError, warnings, loginError, loginWaiting, loginOpen, loginQr, loginStatus, total, hasMore, loadingMore, query,
    loginKind, setLoginKind, mutationBusy, mutationError, likedMids, likedState, likingMid, favoriteError, loadLiked, toggleLike, mutate,
    artistMid, albumId, showCatalog, backCatalog,
    initialize, refresh, connect, cancelLogin, logout, showLibrary, showDownloads, loadRecent, openPlaylist, loadMore, completePlaylist, search, history }
})
