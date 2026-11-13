import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'
import { api, errorMessage } from '../lib/api'
import type { AlbumDetail, ArtistDetail, ArtistRef, ArtistSongOrder, CatalogAlbum, CatalogPage, LoadState, Track } from '../types'

export interface CatalogList<T> extends CatalogPage<T> { state: LoadState; error: string }
const empty = <T>(): CatalogList<T> => ({ items: [], total: 0, nextOffset: 0, hasMore: false, state: 'idle', error: '' })

export const useCatalogStore = defineStore('catalog', () => {
  const artists = shallowRef(empty<ArtistRef>())
  const albums = shallowRef(empty<CatalogAlbum>())
  const artistQuery = ref('')
  const albumQuery = ref('')
  const artistArea = ref(-100)
  const albumArea = ref(1)
  const artist = shallowRef<ArtistDetail | null>(null)
  const album = shallowRef<AlbumDetail | null>(null)
  const songs = shallowRef(empty<Track>())
  const newestSongs = shallowRef(empty<Track>())
  const discography = shallowRef(empty<CatalogAlbum>())
  const artistMid = ref('')
  const albumMid = ref('')
  const artistState = ref<LoadState>('idle')
  const albumState = ref<LoadState>('idle')
  const artistError = ref('')
  const albumError = ref('')
  let artistsRequest = 0, albumsRequest = 0, artistRequest = 0, albumRequest = 0
  const artistSongList = (order: ArtistSongOrder) => order === 'popular' ? songs : newestSongs

  async function loadArtists(more = false) {
    if (more && (artists.value.state === 'loading' || !artists.value.hasMore)) return
    const request = more ? artistsRequest : ++artistsRequest
    const previous = more ? artists.value : empty<ArtistRef>()
    artists.value = { ...previous, state: 'loading', error: '' }
    try {
      const result = await api.catalogArtists(artistQuery.value, more ? previous.nextOffset : 0, artistArea.value)
      if (request !== artistsRequest) return
      artists.value = { ...result, items: [...previous.items, ...result.items], state: 'ready', error: '' }
    } catch (cause) {
      if (request === artistsRequest) artists.value = { ...previous, state: 'error', error: errorMessage(cause) }
    }
  }
  async function loadAlbums(more = false) {
    if (more && (albums.value.state === 'loading' || !albums.value.hasMore)) return
    const request = more ? albumsRequest : ++albumsRequest
    const previous = more ? albums.value : empty<CatalogAlbum>()
    albums.value = { ...previous, state: 'loading', error: '' }
    try {
      const result = await api.catalogAlbums(albumQuery.value, more ? previous.nextOffset : 0, albumArea.value)
      if (request !== albumsRequest) return
      albums.value = { ...result, items: [...previous.items, ...result.items], state: 'ready', error: '' }
    } catch (cause) {
      if (request === albumsRequest) albums.value = { ...previous, state: 'error', error: errorMessage(cause) }
    }
  }
  async function loadArtist(mid: string, force = false) {
    if (!force && artistMid.value === mid && artistState.value === 'ready') return
    const request = ++artistRequest
    artistMid.value = mid; artist.value = null; songs.value = empty(); newestSongs.value = empty(); discography.value = empty()
    artistState.value = 'loading'; artistError.value = ''
    try {
      const [detail, songPage, albumPage] = await Promise.all([api.artistDetail(mid), api.artistSongs(mid, 0, 'popular'), api.artistAlbums(mid, 0)])
      if (request !== artistRequest) return
      artist.value = detail
      songs.value = { ...songPage, state: 'ready', error: '' }
      discography.value = { ...albumPage, state: 'ready', error: '' }
      artistState.value = 'ready'
    } catch (cause) {
      if (request === artistRequest) { artistState.value = 'error'; artistError.value = errorMessage(cause) }
    }
  }
  async function loadArtistSongs(order: ArtistSongOrder = 'popular') {
    const list = artistSongList(order)
    const previous = list.value
    if (artistState.value !== 'ready' || previous.state === 'loading' || (previous.state === 'ready' && !previous.hasMore)) return
    const request = artistRequest
    list.value = { ...previous, state: 'loading', error: '' }
    try {
      const result = await api.artistSongs(artistMid.value, previous.nextOffset, order)
      if (request !== artistRequest) return
      list.value = { ...result, items: [...previous.items, ...result.items], state: 'ready', error: '' }
    } catch (cause) {
      if (request === artistRequest) list.value = { ...previous, state: 'error', error: errorMessage(cause) }
    }
  }
  async function loadArtistAlbums() {
    if (!discography.value.hasMore || discography.value.state === 'loading') return
    const request = artistRequest, previous = discography.value
    discography.value = { ...previous, state: 'loading', error: '' }
    try {
      const result = await api.artistAlbums(artistMid.value, previous.nextOffset)
      if (request !== artistRequest) return
      discography.value = { ...result, items: [...previous.items, ...result.items], state: 'ready', error: '' }
    } catch (cause) {
      if (request === artistRequest) discography.value = { ...previous, state: 'error', error: errorMessage(cause) }
    }
  }
  async function completeArtistSongs(order: ArtistSongOrder = 'popular'): Promise<Track[]> {
    const list = artistSongList(order)
    if (artistState.value !== 'ready' || list.value.state === 'loading') throw new Error('歌曲正在读取，请稍后再播放。')
    const request = artistRequest
    while (list.value.state !== 'ready' || list.value.hasMore) {
      await loadArtistSongs(order)
      if (request !== artistRequest) throw new Error('当前歌手已变化。')
      if (list.value.state === 'error') throw new Error(list.value.error)
    }
    return [...new Map(list.value.items.map(track => [track.mid, track])).values()]
  }
  async function loadAlbum(mid: string, force = false) {
    if (!force && albumMid.value === mid && albumState.value === 'ready') return
    const request = ++albumRequest
    albumMid.value = mid; album.value = null; albumState.value = 'loading'; albumError.value = ''
    try {
      const result = await api.albumDetail(mid)
      if (request !== albumRequest) return
      album.value = result; albumState.value = 'ready'
    } catch (cause) {
      if (request === albumRequest) { albumState.value = 'error'; albumError.value = errorMessage(cause) }
    }
  }
  function reset() {
    artistsRequest++; albumsRequest++; artistRequest++; albumRequest++
    artists.value = empty(); albums.value = empty(); songs.value = empty(); newestSongs.value = empty(); discography.value = empty()
    artist.value = null; album.value = null; artistMid.value = ''; albumMid.value = ''
    artistState.value = 'idle'; albumState.value = 'idle'; artistError.value = ''; albumError.value = ''
    artistQuery.value = ''; albumQuery.value = ''; artistArea.value = -100; albumArea.value = 1
  }
  return { artists, albums, artistQuery, albumQuery, artistArea, albumArea, artist, album, songs, newestSongs, discography,
    artistState, albumState, artistError, albumError, loadArtists, loadAlbums, loadArtist, loadAlbum, loadArtistSongs, loadArtistAlbums, completeArtistSongs, reset }
})
