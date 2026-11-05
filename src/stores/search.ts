import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'
import { api, errorMessage } from '../lib/api'
import type { ArtistRef, CatalogAlbum, LoadState, Playlist, Track } from '../types'
export type SearchKind = 'songs' | 'artists' | 'albums' | 'playlists'
interface Results { songs: Track[]; artists: ArtistRef[]; albums: CatalogAlbum[]; playlists: Playlist[]; total: number; nextOffset: number; hasMore: boolean }
const empty = (): Results => ({ songs: [], artists: [], albums: [], playlists: [], total: 0, nextOffset: 0, hasMore: false })
export const useSearchStore = defineStore('search', () => {
  const keyword = ref(''), kind = ref<SearchKind>('songs'), state = ref<LoadState>('idle'), error = ref('')
  const results = shallowRef(empty()), history = shallowRef<string[]>([]), historyError = ref('')
  let request = 0, historyRequest = 0, accountGeneration = 0
  async function loadHistory() {
    const version = ++historyRequest
    try { const items = await api.searchHistory(); if (version === historyRequest) { history.value = items; historyError.value = '' } }
    catch (cause) { if (version === historyRequest) historyError.value = errorMessage(cause) }
  }
  async function clearHistory() {
    const version = ++historyRequest
    try { await api.clearSearchHistory(); if (version === historyRequest) { history.value = []; historyError.value = '' } }
    catch (cause) { if (version === historyRequest) historyError.value = errorMessage(cause) }
  }
  async function search(query: string, target = kind.value, more = false) {
    if (more && (state.value === 'loading' || !results.value.hasMore)) return
    const version = ++request, term = query.trim(), previous = more ? results.value : empty()
    const remember = term && term !== keyword.value
    const owner = accountGeneration
    keyword.value = term; kind.value = target; results.value = previous; error.value = ''
    if (!term) { state.value = 'idle'; return }
    state.value = 'loading'
    if (remember) void api.recordSearch(term).then(() => { if (owner === accountGeneration) return loadHistory() }).catch(cause => { if (owner === accountGeneration) historyError.value = errorMessage(cause) })
    try {
      const offset = more ? previous.nextOffset : 0
      if (target === 'songs') { const page = await api.searchTracks(term, offset); if (version !== request) return; results.value = { ...previous, ...page, songs: [...previous.songs, ...page.items] } }
      else if (target === 'artists') { const page = await api.catalogArtists(term, offset, -100); if (version !== request) return; results.value = { ...previous, ...page, artists: [...previous.artists, ...page.items] } }
      else if (target === 'albums') { const page = await api.catalogAlbums(term, offset, 1); if (version !== request) return; results.value = { ...previous, ...page, albums: [...previous.albums, ...page.items] } }
      else { const page = await api.searchPlaylists(term, offset); if (version !== request) return; results.value = { ...previous, ...page, playlists: [...previous.playlists, ...page.items] } }
      state.value = 'ready'
    } catch (cause) { if (version === request) { error.value = errorMessage(cause); state.value = 'error' } }
  }
  function reset() { accountGeneration++; request++; historyRequest++; results.value = empty(); history.value = []; keyword.value = ''; kind.value = 'songs'; error.value = ''; historyError.value = ''; state.value = 'idle' }
  return { keyword, kind, state, error, results, history, historyError, loadHistory, clearHistory, search, reset }
})
