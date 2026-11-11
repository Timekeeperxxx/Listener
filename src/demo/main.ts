import { createApp, watch } from 'vue'
import { createPinia } from 'pinia'
import type { LyricLine } from '@applemusic-like-lyrics/core'
import App from '../App.vue'
import { useLibraryStore } from '../stores/library'
import { usePlayerStore } from '../stores/player'
import { useSavedCatalogStore } from '../stores/saved-catalog'
import { useSearchStore } from '../stores/search'
import { recommendationDay } from '../stores/discovery'
import { api } from '../lib/api'
import { publicCatalogApi } from './catalog-api'
import type { ArtistRef, CatalogAlbum, Library, Playlist, SavedCatalog, Track } from '../types'
import data from './data.json'
import links from './catalog-links.json'
import '../style.css'
import '../redesign.css'

// This separate HTML entry is an explicit UI demo. It never loads the desktop
// entry, connects an account, requests QQ audio, or writes user data.
// Artist and album pages fetch live public QQ metadata, independently of demo playlists.
const images = import.meta.glob<string>('./assets/*.jpg', { eager: true, query: '?url', import: 'default' })
const image = (file: string) => images[`./assets/${file}`]
const tracks: Track[] = data.albums.flatMap(album => album.tracks.map(track => ({ ...track, cover: image(track.cover),
  artists: [{ mid: links.artists[album.artist as keyof typeof links.artists], name: album.artist, image: image(data.artists.find(a => a.name === album.artist)!.image) }],
  albumMid: links.albums[album.name as keyof typeof links.albums],
})))
const pinia = createPinia()
const library = useLibraryStore(pinia)
const player = usePlayerStore(pinia)
const chosen = tracks.find(track => track.title === '晴天') ?? tracks[0]!
const favoriteTracks = tracks.filter((_, index) => index % 3 === 0)
const playlistTracks = new Map<string, Track[]>([
  ['liked', favoriteTracks],
  ['evening', tracks.filter(track => ['周杰伦', '孙燕姿'].includes(track.artist)).slice(0, 18)],
  ['city', tracks.filter(track => track.artist === 'The Weeknd')],
  ['familiar', tracks.filter(track => ['王菲', '陈奕迅'].includes(track.artist))],
  ['piano', tracks.filter(track => track.artist === 'Charlie Puth')],
])
let playlists: Playlist[] = [
  { id: 'liked', name: '我喜欢的音乐', cover: '', count: favoriteTracks.length, kind: 'liked' },
  { id: 'evening', name: '把时间交给旋律', cover: tracks.find(track => track.album === '七里香')?.cover ?? tracks[0]!.cover, count: playlistTracks.get('evening')!.length, kind: 'created' },
  { id: 'city', name: '城市入夜以后', cover: tracks.find(track => track.album === 'After Hours')!.cover, count: playlistTracks.get('city')!.length, kind: 'created' },
  { id: 'familiar', name: '熟悉的声音', cover: tracks.find(track => track.artist === '王菲')!.cover, count: playlistTracks.get('familiar')!.length, kind: 'collected' },
  { id: 'piano', name: '耳机里的小世界', cover: tracks.find(track => track.album === 'Voicenotes')!.cover, count: playlistTracks.get('piano')!.length, kind: 'created' },
]
const recent = [chosen, ...data.artists.slice(1).map(artist => tracks.find(track => track.artist === artist.name)!) ]
const libraryResult = (): Library => ({ playlists: playlists.map(item => ({ ...item })), warnings: [] })
const unavailable = async () => { throw new Error('网页演示不连接真实 QQ 账户，请在 macOS 应用中使用此功能。') }
let demoSaved: SavedCatalog = { artists: [], albums: [] }
let demoSearchHistory: string[] = []
const demoOffsets = new Map<string, number>()
Object.assign(api, {
  ...publicCatalogApi,
  albumMeta: async (mid: string) => (await publicCatalogApi.albumDetail(mid)).album,
  dailyRecommendation: async () => ({ day: recommendationDay(), playlist: { id: 'demo-daily', name: '每日30首（演示）', cover: tracks[0]!.cover, kind: 'public', count: Math.min(30, tracks.length) }, tracks: tracks.slice(0, 30) }),
  recommendationRadio: async () => tracks.slice(0, 10),
  savedCatalog: async () => demoSaved,
  saveArtist: async (artist: ArtistRef, saved: boolean) => { demoSaved = { ...demoSaved, artists: saved ? [artist, ...demoSaved.artists.filter(a => a.mid !== artist.mid)] : demoSaved.artists.filter(a => a.mid !== artist.mid) }; return demoSaved },
  saveAlbum: async (album: CatalogAlbum, saved: boolean) => { demoSaved = { ...demoSaved, albums: saved ? [album, ...demoSaved.albums.filter(a => a.mid !== album.mid)] : demoSaved.albums.filter(a => a.mid !== album.mid) }; return demoSaved },
  searchHistory: async () => [...demoSearchHistory],
  recordSearch: async (keyword: string) => { demoSearchHistory = [keyword, ...demoSearchHistory.filter(t => t !== keyword)].slice(0, 20) },
  clearSearchHistory: async () => { demoSearchHistory = [] },
  lyricOffset: async (mid: string) => demoOffsets.get(mid) ?? 0,
  saveLyricOffset: async (mid: string, value: number) => { demoOffsets.set(mid, value); player.trackOffset = value },
  storageUsage: unavailable,
  clearLyricCache: unavailable,
  checkUpdate: unavailable,
  installUpdate: unavailable,
  library: async () => libraryResult(),
  history: async () => recent,
  playlist: async (id: string, offset: number) => {
    if (id === 'demo-daily') return { tracks: tracks.slice(offset, 30), total: Math.min(30, tracks.length), nextOffset: Math.min(30, tracks.length), hasMore: false }
    const list = playlistTracks.get(id)
    if (!list) return publicCatalogApi.publicPlaylist(id, offset)
    return { tracks: list.slice(offset, offset + 30), total: list.length, nextOffset: Math.min(offset + 30, list.length), hasMore: offset + 30 < list.length }
  },
  mutatePlaylist: async (action: string, id: string | null, name: string | null, track: Track | null) => {
    if (action === 'create') { const key = `demo-${Date.now()}`; playlists.push({ id: key, name: name!, count: 0, cover: '', kind: 'created' }); playlistTracks.set(key, []) }
    else {
      const selected = playlists.find(item => item.id === id)
      if (!selected) throw new Error('这张演示歌单不存在。')
      if (action === 'rename') selected.name = name!
      if (action === 'delete') { playlists = playlists.filter(item => item.id !== id); playlistTracks.delete(id!) }
      if (action === 'add') { const list = playlistTracks.get(id!)!; if (!list.some(item => item.mid === track!.mid)) list.unshift(track!); selected.count = list.length }
      if (action === 'remove') { playlistTracks.set(id!, playlistTracks.get(id!)!.filter(item => item.mid !== track!.mid)); selected.count = playlistTracks.get(id!)!.length }
    }
    return libraryResult()
  },
  downloads: async () => [],
  download: unavailable,
  openLogin: unavailable,
  logout: unavailable,
})
library.profile = { uin: 'html-demo', nickname: 'Listener', avatar: image(data.artists[0]!.image), renewable: true }
library.playlists = libraryResult().playlists
library.libraryState = 'ready'
library.recentTracks = recent
library.recentState = 'ready'
library.likedTracks = favoriteTracks
library.likedMids = new Set(favoriteTracks.map(track => track.mid))
library.likedState = 'ready'
void useSavedCatalogStore(pinia).load()
void useSearchStore(pinia).loadHistory()

let anchorTime = 75000
let anchorWall = performance.now()
const readClock = () => Math.min(player.duration * 1000, anchorTime + (player.playing ? performance.now() - anchorWall : 0))
const sampleLines = ['晚风轻轻落在窗边', '把今天交给这一段旋律', '城市慢下来，灯光渐渐亮起', '让心事随着节拍轻轻摇摆', '此刻只想安静听一首歌', '时间在熟悉的声音里停留', '沿着音符走过每一条街', '把温柔留给下一个明天']
function layoutLyrics(duration: number): LyricLine[] {
  return Array.from({ length: Math.ceil(duration / 8) }, (_, index) => {
    const text = sampleLines[index % sampleLines.length]!
    const start = index * 8000
    const length = [...text].length
    return { startTime: start, endTime: start + 6800, words: [...text].map((word, i) => ({ word, startTime: start + Math.round(i * 6500 / length), endTime: start + Math.round((i + 1) * 6500 / length) })), translatedLyric: '', romanLyric: '', isBG: false, isDuet: false }
  })
}
function select(track: Track, items?: Track[], position = 0) {
  anchorTime = position * 1000; anchorWall = performance.now()
  player.current = track
  if (items) player.queue = [...new Map(items.map(item => [item.mid, item])).values()]
  if (!player.queue.some(item => item.mid === track.mid)) player.queue = [...player.queue, track]
  player.time = anchorTime; player.duration = track.duration; player.sourceState = 'ready'; player.buffering = false; player.playbackError = ''; player.restored = false
  player.lyricLines = layoutLyrics(track.duration); player.lyricState = 'ready'; player.lyricMode = 'word'
  player.lyricWarnings = ['网页歌词为原创排版示例。']
}
player.play = async (track, items, startAt = 0) => { select(track, items, startAt); player.playing = true }
player.pause = () => { anchorTime = readClock(); player.time = anchorTime; player.playing = false }
player.toggle = async () => { if (player.playing) player.pause(); else { anchorWall = performance.now(); player.playing = true } }
player.seek = seconds => { anchorTime = Math.max(0, Math.min(seconds, player.duration)) * 1000; anchorWall = performance.now(); player.time = anchorTime }
player.readLyricTime = () => Math.max(0, Math.round(readClock() + player.settings.lyricOffset))
player.next = async () => {
  const list = player.queue
  if (!list.length) return
  const pending = list.findIndex(track => track.mid === player.nextMid)
  const index = pending >= 0 ? pending : player.shuffle ? Math.floor(Math.random() * list.length) : (player.currentIndex + 1) % list.length
  player.nextMid = null
  await player.play(list[index]!)
}
player.previous = async () => { if (readClock() > 3000) player.seek(0); else if (player.queue.length) await player.play(player.queue[(player.currentIndex - 1 + player.queue.length) % player.queue.length]!) }
select(chosen, recent, 75)
const clock = setInterval(() => { if (player.playing) { player.time = readClock(); if (player.time >= player.duration * 1000) void player.next() } }, 100)
window.addEventListener('pagehide', () => clearInterval(clock), { once: true })

const route = new URLSearchParams(location.hash.slice(1))
const view = route.get('view')
if (view === 'artists' || view === 'albums') library.showCatalog(view)
if (view === 'artist') library.showCatalog('artist', route.get('artist') ?? '0025NhlN2yWrP4')
if (view === 'album') library.showCatalog('album', route.get('album') ?? '000MkMni19ClKG')
watch([() => library.view, () => library.artistMid, () => library.albumId], () => {
  const params = new URLSearchParams({ view: library.view })
  if (library.view === 'artist') params.set('artist', library.artistMid)
  if (library.view === 'album') params.set('album', library.albumId)
  history.replaceState(null, '', `${location.pathname}#${params}`)
})
createApp(App, { demo: true }).use(pinia).mount('#app')
