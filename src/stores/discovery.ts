import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'
import { api, errorMessage } from '../lib/api'
import type { CatalogAlbum, CatalogPage, Chart, DailyRecommendation, DesktopSongRecommendation, LoadState, Playlist, Track } from '../types'

export const recommendationDay = () => Math.floor((Date.now() / 1000 + 8 * 3600) / 86400)

// Public catalog selections use QQ's existing metadata APIs, independently of personal collections.
export const useDiscoveryStore = defineStore('discovery', () => {
  const albums = shallowRef<CatalogAlbum[]>([])
  const albumState = ref<LoadState>('idle'), albumError = ref('')
  const playlists = shallowRef<CatalogPage<Playlist>>({ items: [], total: 0, nextOffset: 0, hasMore: false })
  const playlistState = ref<LoadState>('idle'), playlistError = ref(''), playlistQuery = ref('华语流行')
  const daily = shallowRef<DailyRecommendation | null>(null), radio = shallowRef<Track[]>([])
  const dailyState = ref<LoadState>('idle'), dailyError = ref('')
  const radioState = ref<LoadState>('idle'), radioError = ref('')
  const desktopSongs = shallowRef<DesktopSongRecommendation | null>(null)
  const desktopSongsState = ref<LoadState>('idle'), desktopSongsError = ref('')
  const recommendedPlaylists = shallowRef<Playlist[]>([])
  const recommendedPlaylistState = ref<LoadState>('idle'), recommendedPlaylistError = ref('')
  const charts = shallowRef<Chart[]>([])
  const chartState = ref<LoadState>('idle'), chartError = ref('')
  let albumRequest = 0, playlistRequest = 0, dailyRequest = 0, radioRequest = 0
  let recommendedPlaylistRequest = 0, chartRequest = 0, desktopSongsRequest = 0

  async function loadDaily(refresh = false) {
    const request = ++dailyRequest
    daily.value = null; dailyState.value = 'loading'; dailyError.value = ''
    try {
      const result = await api.dailyRecommendation(refresh)
      if (request !== dailyRequest) return
      if (result.day !== recommendationDay()) throw new Error('每日推荐已跨日，请刷新。')
      daily.value = result; dailyState.value = 'ready'
    } catch (cause) { if (request === dailyRequest) { dailyError.value = errorMessage(cause); dailyState.value = 'error' } }
  }
  async function loadRadio() {
    const request = ++radioRequest
    radioState.value = 'loading'; radioError.value = ''
    try {
      const tracks = await api.recommendationRadio()
      if (request !== radioRequest) return
      radio.value = tracks; radioState.value = 'ready'
    } catch (cause) { if (request === radioRequest) { radioError.value = errorMessage(cause); radioState.value = 'error' } }
  }

  async function loadRecommendedPlaylists() {
    const request = ++recommendedPlaylistRequest
    recommendedPlaylistState.value = 'loading'
    recommendedPlaylistError.value = ''
    try {
      const items = await api.recommendedPlaylists()
      if (request !== recommendedPlaylistRequest) return
      recommendedPlaylists.value = items
      recommendedPlaylistState.value = 'ready'
    } catch (cause) {
      if (request !== recommendedPlaylistRequest) return
      recommendedPlaylistError.value = errorMessage(cause)
      recommendedPlaylistState.value = 'error'
    }
  }

  async function loadDesktopSongs() {
    const request = ++desktopSongsRequest
    desktopSongsState.value = 'loading'
    desktopSongsError.value = ''
    try {
      const result = await api.desktopSongRecommendation()
      if (request !== desktopSongsRequest) return
      desktopSongs.value = result
      desktopSongsState.value = 'ready'
    } catch (cause) {
      if (request !== desktopSongsRequest) return
      desktopSongsError.value = errorMessage(cause)
      desktopSongsState.value = 'error'
    }
  }

  async function loadCharts() {
    const request = ++chartRequest
    chartState.value = 'loading'
    chartError.value = ''
    try {
      const items = await api.featuredCharts()
      if (request !== chartRequest) return
      charts.value = items
      chartState.value = 'ready'
    } catch (cause) {
      if (request !== chartRequest) return
      chartError.value = errorMessage(cause)
      chartState.value = 'error'
    }
  }

  async function loadAlbums() {
    const request = ++albumRequest
    albumState.value = 'loading'; albumError.value = ''
    try {
      const page = await api.catalogAlbums('', 0, 1)
      if (request !== albumRequest) return
      albums.value = page.items.slice(0, 5); albumState.value = 'ready'
    } catch (cause) { if (request === albumRequest) { albumError.value = errorMessage(cause); albumState.value = 'error' } }
  }
  async function loadPlaylists(query = playlistQuery.value, more = false) {
    if (more && (playlistState.value === 'loading' || !playlists.value.hasMore)) return
    const request = ++playlistRequest
    const previous = more ? playlists.value : { items: [], total: 0, nextOffset: 0, hasMore: false }
    playlistQuery.value = query; playlists.value = previous; playlistState.value = 'loading'; playlistError.value = ''
    try {
      const page = await api.searchPlaylists(query, previous.nextOffset)
      if (request !== playlistRequest) return
      playlists.value = { ...page, items: [...previous.items, ...page.items] }; playlistState.value = 'ready'
    } catch (cause) { if (request === playlistRequest) { playlistError.value = errorMessage(cause); playlistState.value = 'error' } }
  }
  function reset() {
    albumRequest++; playlistRequest++; dailyRequest++; radioRequest++
    recommendedPlaylistRequest++; chartRequest++; desktopSongsRequest++
    desktopSongs.value = null; desktopSongsState.value = 'idle'; desktopSongsError.value = ''
    recommendedPlaylists.value = []; recommendedPlaylistState.value = 'idle'; recommendedPlaylistError.value = ''
    charts.value = []; chartState.value = 'idle'; chartError.value = ''
    daily.value = null; radio.value = []; dailyState.value = 'idle'; radioState.value = 'idle'; dailyError.value = ''; radioError.value = ''
    albums.value = []; albumState.value = 'idle'; albumError.value = ''
    playlists.value = { items: [], total: 0, nextOffset: 0, hasMore: false }
    playlistState.value = 'idle'; playlistError.value = ''; playlistQuery.value = '华语流行'
  }
  return { albums, albumState, albumError, playlists, playlistState, playlistError, playlistQuery,
    daily, dailyState, dailyError, radio, radioState, radioError,
    desktopSongs, desktopSongsState, desktopSongsError, loadDesktopSongs,
    recommendedPlaylists, recommendedPlaylistState, recommendedPlaylistError, charts, chartState, chartError,
    loadDaily, loadRadio, loadRecommendedPlaylists, loadCharts, loadAlbums, loadPlaylists, reset }
})
