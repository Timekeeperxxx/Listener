export interface Profile {
  uin: string
  nickname: string
  avatar: string
  offline?: boolean
  renewable?: boolean
}

export interface Track {
  id: number
  mid: string
  mediaMid: string
  title: string
  artist: string
  album: string
  cover: string
  duration: number
  vip: boolean
  artists?: ArtistRef[]
  albumMid?: string
}

export interface ArtistRef { mid: string; name: string; image: string }
export interface CatalogAlbum {
  mid: string
  name: string
  artists: ArtistRef[]
  credit: string
  cover: string
  publishDate: string
  albumType: string
}
export interface ArtistDetail {
  artist: ArtistRef
  description: string
  totalSongs: number
  totalAlbums: number
}
export interface AlbumDetail {
  album: CatalogAlbum
  description: string
  language: string
  genre: string
  company: string
  tracks: Track[]
}
export interface CatalogPage<T> { items: T[]; total: number; nextOffset: number; hasMore: boolean }

export interface Playlist {
  id: string
  name: string
  cover: string
  count: number
  kind: 'liked' | 'created' | 'collected' | 'public'
  dirId?: number
  creator?: string
}

export interface PlaylistPage {
  tracks: Track[]
  total: number
  nextOffset: number
  hasMore: boolean
}

export interface DailyRecommendation {
  playlist: Playlist
  tracks: Track[]
  /** Calendar day in Asia/Shanghai, counted from the Unix epoch. */
  day: number
}

export interface Library {
  playlists: Playlist[]
  warnings: string[]
}

export interface PlaybackSource {
  url: string
  quality: string
  local?: boolean
}

export interface RawLyrics {
  word: string
  line: string
  translation: string
  romanization: string
  warnings: string[]
}

export interface AppSettings {
  volume: number
  animatedBackground: boolean
  lyricOffset: number
  audioQuality: 'auto' | 'standard' | 'high' | 'lossless' | 'hires'
  lyrics: LyricSettings
  smartMix: boolean
  preloadNext: boolean
  closeToTray: boolean
  shortcuts: ShortcutSettings
  cacheLimitMb: number
}
export interface LyricSettings { fontScale: number; lineHeight: number; showTranslation: boolean; showRomanization: boolean }
export interface ShortcutSettings { enabled: boolean; toggle: string; next: string; previous: string; main: string }
export interface SavedCatalog { artists: ArtistRef[]; albums: CatalogAlbum[] }
export interface StorageUsage { cacheBytes: number; cacheCount: number; downloadBytes: number; downloadCount: number; databaseBytes: number }
export interface UpdateInfo { version: string; currentVersion: string; notes: string }
export interface DesktopStatus { shortcutError: string }

export interface Download {
  track: Track
  quality: string
  filename: string
  bytes: number
}

export interface PlaybackState {
  queue: Track[]
  currentMid: string | null
  nextMid: string | null
  position: number
  repeat: 'off' | 'all' | 'one'
  shuffle: boolean
}

export interface AppError {
  code: string
  message: string
}

export type LoginPoll =
  | { status: 'waiting' }
  | { status: 'scanned' }
  | { status: 'expired' }
  | { status: 'refused' }
  | { status: 'closed' }
  | { status: 'complete'; profile: Profile }

export interface LoginQr {
  id: string
  image: string
  expiresIn: number
}

export type LoadState = 'idle' | 'loading' | 'ready' | 'error'

export type HomeNavigation = 'recommend' | 'discover' | 'favorites'
export type HomeCategory = 'songs' | 'albums' | 'artists' | 'playlists'

export interface TrackMenuTarget { track: Track; tracks: Track[]; anchor: HTMLElement }
