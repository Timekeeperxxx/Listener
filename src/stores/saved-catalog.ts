import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'
import { api, errorMessage } from '../lib/api'
import type { ArtistRef, CatalogAlbum, LoadState, SavedCatalog } from '../types'
export const useSavedCatalogStore = defineStore('saved-catalog', () => {
  const items = shallowRef<SavedCatalog>({ artists: [], albums: [] }), state = ref<LoadState>('idle'), error = ref(''), busy = ref('')
  let generation = 0
  async function load() {
    const request = ++generation; state.value = 'loading'; error.value = ''
    try { const data = await api.savedCatalog(); if (request === generation) { items.value = data; state.value = 'ready' } }
    catch (cause) { if (request === generation) { error.value = errorMessage(cause); state.value = 'error' } }
  }
  const hasArtist = (mid: string) => items.value.artists.some(a => a.mid === mid)
  const hasAlbum = (mid: string) => items.value.albums.some(a => a.mid === mid)
  async function toggleArtist(artist: ArtistRef) { await change(`artist:${artist.mid}`, () => api.saveArtist(artist, !hasArtist(artist.mid))) }
  async function toggleAlbum(album: CatalogAlbum) { await change(`album:${album.mid}`, () => api.saveAlbum(album, !hasAlbum(album.mid))) }
  async function change(key: string, mutate: () => Promise<SavedCatalog>) {
    if (busy.value || state.value !== 'ready') return
    const request = generation; busy.value = key; error.value = ''
    try { const data = await mutate(); if (request === generation) items.value = data }
    catch (cause) { if (request === generation) error.value = errorMessage(cause) }
    finally { if (request === generation) busy.value = '' }
  }
  function reset() { generation++; items.value = { artists: [], albums: [] }; state.value = 'idle'; error.value = ''; busy.value = '' }
  return { items, state, error, busy, load, hasArtist, hasAlbum, toggleArtist, toggleAlbum, reset }
})
