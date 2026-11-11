import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core'
import type { DailyRecommendation } from '../types'
import type { QqMixPlan } from './qq-automix'
import type { DesktopStatus, SavedCatalog, StorageUsage, UpdateInfo, AlbumDetail, ArtistDetail, ArtistRef, CatalogAlbum, CatalogPage, AppSettings, Download, Library, LoginPoll, LoginQr, Playlist, PlaylistPage, PlaybackSource, PlaybackState, Profile, RawLyrics, Track } from '../types'

export const desktop = isTauri()

let renewal: Promise<void> | null = null
const renewableReads = new Set(['get_library', 'get_playlist', 'search_tracks', 'get_source', 'get_lyrics', 'get_catalog_artists', 'get_catalog_albums', 'get_artist_detail', 'get_artist_songs', 'get_artist_albums', 'get_album_detail', 'get_catalog_track', 'get_search_tracks', 'get_search_playlists', 'get_daily_recommendation', 'get_recommendation_radio'])
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject({ code: 'DESKTOP_REQUIRED', message: '请在 Listener macOS 应用中连接 QQ 音乐。' })
  try { return await invoke<T>(command, args) }
  catch (cause) {
    if (!renewableReads.has(command) || typeof cause !== 'object' || cause === null || !('code' in cause) || cause.code !== 'AUTH_EXPIRED') throw cause
    if (!renewal) renewal = invoke<void>('renew_session', { force: true }).finally(() => { renewal = null })
    await renewal
    return invoke<T>(command, args)
  }
}

export const api = {
  session: () => call<Profile | null>('get_session'),
  openLogin: (kind: 'qq' | 'wx' | 'mobile') => call<LoginQr>('open_login', { kind }),
  renew: () => call<void>('renew_session', { force: false }),
  pollLogin: (id: string) => call<LoginPoll>('poll_login', { id }),
  cancelLogin: () => call<void>('cancel_login'),
  logout: () => call<void>('logout'),
  library: () => call<Library>('get_library'),
  dailyRecommendation: (refresh: boolean) => call<DailyRecommendation>('get_daily_recommendation', { refresh }),
  recommendationRadio: () => call<Track[]>('get_recommendation_radio'),
  playlist: (id: string, offset: number) => call<PlaylistPage>('get_playlist', { id, offset }),
  search: (keyword: string, page: number) => call<Track[]>('search_tracks', { keyword, page }),
  catalogArtists: (keyword: string, offset: number, area: number) => call<CatalogPage<ArtistRef>>('get_catalog_artists', { keyword, offset, area }),
  catalogAlbums: (keyword: string, offset: number, area: number) => call<CatalogPage<CatalogAlbum>>('get_catalog_albums', { keyword, offset, area }),
  artistDetail: (mid: string) => call<ArtistDetail>('get_artist_detail', { mid }),
  artistSongs: (mid: string, offset: number) => call<CatalogPage<Track>>('get_artist_songs', { mid, offset }),
  artistAlbums: (mid: string, offset: number) => call<CatalogPage<CatalogAlbum>>('get_artist_albums', { mid, offset }),
  albumDetail: (mid: string) => call<AlbumDetail>('get_album_detail', { mid }),
  catalogTrack: (mid: string) => call<Track>('get_catalog_track', { mid }),
  searchTracks: (keyword: string, offset: number) => call<CatalogPage<Track>>('get_search_tracks', { keyword, offset }),
  searchPlaylists: (keyword: string, offset: number) => call<CatalogPage<Playlist>>('get_search_playlists', { keyword, offset }),
  searchHistory: () => call<string[]>('get_search_history'),
  recordSearch: (keyword: string) => call<void>('record_search', { keyword }),
  clearSearchHistory: () => call<void>('clear_search_history'),
  savedCatalog: () => call<SavedCatalog>('get_saved_catalog'),
  saveArtist: (artist: ArtistRef, saved: boolean) => call<SavedCatalog>('save_catalog_artist', { artist, saved }),
  saveAlbum: (album: CatalogAlbum, saved: boolean) => call<SavedCatalog>('save_catalog_album', { album, saved }),
  lyricOffset: (mid: string) => call<number>('get_lyric_offset', { mid }),
  saveLyricOffset: (mid: string, offset: number) => call<void>('save_lyric_offset', { mid, offset }),
  storageUsage: () => call<StorageUsage>('get_storage_usage'),
  clearLyricCache: () => call<void>('clear_lyric_cache'),
  desktopStatus: () => call<DesktopStatus>('get_desktop_status'),
  hideMain: () => call<void>('hide_main'),
  quit: () => call<void>('quit_app'),
  checkUpdate: () => call<UpdateInfo | null>('check_app_update'),
  installUpdate: () => call<void>('install_app_update'),
  qqMixPlan: (from: Track, to: Track) => call<QqMixPlan>('get_qq_mix_plan', { from, to }),
  source: async (track: Track) => {
    const source = await call<PlaybackSource>('get_source', { track })
    return source.local ? { ...source, url: convertFileSrc(source.url) } : source
  },
  lyrics: (track: Track) => call<RawLyrics>('get_lyrics', { track }),
  history: () => call<Track[]>('get_history'),
  record: (track: Track) => call<void>('record_play', { track }),
  mutatePlaylist: (action: 'create' | 'rename' | 'delete' | 'add' | 'remove', id: string | null, name: string | null, track: Track | null) => call<Library>('mutate_playlist', { action, id, name, track }),
  downloads: () => call<Download[]>('get_downloads'),
  download: (track: Track) => call<Download>('download_track', { track }),
  cancelDownload: (mid: string) => call<void>('cancel_download', { mid }),
  removeDownload: (mid: string) => call<void>('remove_download', { mid }),
  showCompanion: (kind: 'mini' | 'desktop-lyrics') => call<void>('show_companion', { kind }),
  playback: (owner: string) => call<PlaybackState | null>('get_playback', { owner }),
  savePlayback: (owner: string, playback: PlaybackState) => call<void>('save_playback', { owner, playback }),
  settings: () => call<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => call<void>('save_settings', { settings }),
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) return String(error.message)
  return String(error)
}
