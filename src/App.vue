<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { AudioLines, ArrowDown, ArrowRight, ArrowUp, Check, ChevronDown, ChevronLeft, Clock3,
  Download, Heart, ListMusic, LoaderCircle, LogOut, Music2, Pause, Play, Plus,
  QrCode, RefreshCw, Repeat, Repeat1, Search, Shuffle, SkipBack, SkipForward,
  SlidersHorizontal, Volume2, VolumeX, X } from '@lucide/vue'
import CoverArt from './components/CoverArt.vue'
import FavoriteButton from './components/FavoriteButton.vue'
import TrackList from './components/TrackList.vue'
import DownloadsView from './components/DownloadsView.vue'
import PlaylistDialog from './components/PlaylistDialog.vue'
import TrackActions from './components/TrackActions.vue'
import PlayerDock from './components/PlayerDock.vue'
import CatalogView from './components/CatalogView.vue'
import SearchView from './components/SearchView.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import { useSearchStore } from './stores/search'
import { useSavedCatalogStore } from './stores/saved-catalog'
import { resolveArtist, resolveAlbum } from './lib/catalog'
import { useCatalogStore } from './stores/catalog'
import { useDownloadsStore } from './stores/downloads'
import { bindCompanions } from './lib/companion'
import { filterTracks, type TrackSort } from './lib/track-filter'
import { useLibraryStore } from './stores/library'
import { usePlayerStore } from './stores/player'
import { api, desktop, errorMessage } from './lib/api'
import { formatTime } from './lib/format'
import type { Playlist, Track } from './types'

const LyricsView = defineAsyncComponent(() => import('./components/LyricsView.vue'))
const props = defineProps<{ demo?: boolean }>()
const library = useLibraryStore()
const player = usePlayerStore()
const downloads = useDownloadsStore()
const catalog = useCatalogStore()
const searchStore = useSearchStore()
const savedCatalog = useSavedCatalogStore()
const playlistDialog = ref<InstanceType<typeof PlaylistDialog>>()
const trackActions = ref<InstanceType<typeof TrackActions>>()
const trackFilter = ref('')
const trackSort = ref<TrackSort>('default')
const sortDescending = ref(false)
const filterLoading = ref(false)
const filtering = computed(() => !!trackFilter.value.trim() || trackSort.value !== 'default')
const displayedTracks = computed(() => filterTracks(library.tracks, trackFilter.value, trackSort.value, sortDescending.value))
const loginApp = computed(() => library.loginKind === 'wx' ? '微信' : library.loginKind === 'mobile' ? '手机 QQ 音乐' : '手机 QQ')
let unlistenDownload: UnlistenFn | undefined
let stopCompanions: (() => void) | undefined
let renewalTimer: ReturnType<typeof setInterval> | undefined
const scene = ref<'library' | 'lyrics'>('library')
const searchInput = ref<HTMLInputElement>()
const contentScroll = ref<HTMLElement>()
const searchText = ref('')
const settingsDialog = ref<HTMLDialogElement>()
const settingsPanel = ref<InstanceType<typeof SettingsPanel>>()
let unlistenDesktop: UnlistenFn | undefined
let unlistenFocus: UnlistenFn | undefined
let quitting = false
const queueDialog = ref<HTMLDialogElement>()
const loginDialog = ref<HTMLDialogElement>()
const busyAll = ref(false)
const playlistFilter = ref<'all' | 'created' | 'collected'>('all')
const showTranslation = computed({ get: () => player.settings.lyrics.showTranslation, set: value => player.updateSettings({ lyrics: { ...player.settings.lyrics, showTranslation: value } }) })
const showRomanization = computed({ get: () => player.settings.lyrics.showRomanization, set: value => player.updateSettings({ lyrics: { ...player.settings.lyrics, showRomanization: value } }) })
const uiError = ref('')
const windowVisible = ref(true)
const windowMinimized = ref(false)
let unlistenResize: UnlistenFn | undefined
let unlistenClose: UnlistenFn | undefined
let visibilityVersion = 0

const likedPlaylist = computed(() => library.playlists.find(p => p.kind === 'liked'))
const visiblePlaylists = computed(() => playlistFilter.value === 'all' ? library.playlists : library.playlists.filter(p => p.kind === playlistFilter.value))
const recentPreview = computed(() => library.recentTracks.slice(0, 6))
const catalogView = computed(() => ['artists', 'albums', 'artist', 'album'].includes(library.view))
function openCatalog(view: 'artists' | 'albums' | 'artist' | 'album', id = '') { setScene('library'); library.showCatalog(view, id) }
function goBack() { if (catalogView.value) library.backCatalog(); else goLibrary() }
let catalogLinkRequest = 0
watch([() => library.view, () => library.profile?.uin], () => { catalogLinkRequest++ })
async function openTrackArtist(name: string, track: Track) {
  const request = ++catalogLinkRequest
  uiError.value = ''
  try {
    const artist = await resolveArtist(track, name)
    if (request === catalogLinkRequest) openCatalog('artist', artist.mid)
  } catch (cause) { if (request === catalogLinkRequest) uiError.value = errorMessage(cause) }
}
async function openTrackAlbum(track: Track) {
  const request = ++catalogLinkRequest
  uiError.value = ''
  try {
    const mid = await resolveAlbum(track)
    if (request === catalogLinkRequest) openCatalog('album', mid)
  } catch (cause) { if (request === catalogLinkRequest) uiError.value = errorMessage(cause) }
}

function playCatalog(track: Track, tracks: Track[]) { void player.play(track, tracks) }
function shuffleCatalog(tracks: Track[]) { if (tracks.length) { player.setShuffle(true); void player.play(tracks[Math.floor(Math.random() * tracks.length)], tracks) } }
const trackTitle = computed(() => library.view === 'playlist' ? library.selected?.name : library.view === 'search' ? `“${library.query}”` : '最近播放')
const notices = computed(() => [uiError.value, library.loginError, library.favoriteError, player.notice, savedCatalog.error].filter(Boolean))
watch(() => library.profile?.uin, owner => {
  catalog.reset(); searchStore.reset(); savedCatalog.reset()
  if (owner) { void savedCatalog.load(); void searchStore.loadHistory(); void player.restorePlayback(owner); void library.loadRecent(); downloads.clear(); void downloads.load() }
  else { player.reset(); downloads.clear() }
})
watch(() => library.loginOpen, async open => {
  await nextTick()
  if (open) loginDialog.value?.showModal()
  else loginDialog.value?.close()
})

watch([() => library.view, () => library.selected?.id], () => { trackFilter.value = ''; trackSort.value = 'default'; sortDescending.value = false })
watch([() => library.view, () => library.artistMid, () => library.albumId], () => {
  if (contentScroll.value) contentScroll.value.scrollTop = 0
}, { flush: 'post' })
watch([trackFilter, trackSort, () => library.loadingMore], async () => {
  if (library.view !== 'playlist' || !filtering.value || library.loadingMore || filterLoading.value || !library.hasMore) return
  filterLoading.value = true
  try {
    while (library.view === 'playlist' && filtering.value && library.hasMore) {
      await library.loadMore()
      if (library.error) break
    }
  } finally { filterLoading.value = false }
})
function openDownloads() {
  if (!library.profile) { void library.connect(); return }
  setScene('library'); library.showDownloads(); void downloads.load()
}
function showSettings() { void settingsPanel.value?.refresh(); settingsDialog.value?.showModal() }
async function quitApp() {
  if (quitting) return
  quitting = true
  try { await player.flushPlayback(); await api.quit() }
  catch (cause) { uiError.value = `退出失败：${errorMessage(cause)}`; quitting = false }
}
async function showCompanion(kind: 'mini' | 'desktop-lyrics') {
  try { await api.showCompanion(kind); settingsDialog.value?.close() }
  catch (cause) { uiError.value = errorMessage(cause) }
}
function updateSleep(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  if (value === 'active') return
  player.setSleepTimer(value === 'off' ? null : value === 'track' ? 'track' : Number(value))
}
async function observeVisibility() {
  const version = ++visibilityVersion
  windowVisible.value = !document.hidden
  if (desktop) {
    windowMinimized.value = await getCurrentWindow().isMinimized()
    windowVisible.value = windowVisible.value && await getCurrentWindow().isVisible()
    if (version !== visibilityVersion) return
  }
  player.setVisualActive(windowVisible.value && !windowMinimized.value && scene.value === 'lyrics')
}

function setScene(value: 'library' | 'lyrics') { scene.value = value; void observeVisibility() }
function goLibrary() { setScene('library'); library.showLibrary(); if (library.profile) void library.loadRecent() }
async function playLiked() {
  if (!likedPlaylist.value) return
  await openPlaylist(likedPlaylist.value)
  await playAll()
}
async function openPlaylist(playlist: Playlist) { setScene('library'); await library.openPlaylist(playlist) }
async function openLiked() {
  if (!library.profile) { await library.connect(); return }
  if (likedPlaylist.value) await openPlaylist(likedPlaylist.value)
}
async function openHistory() {
  if (!library.profile) { await library.connect(); return }
  setScene('library'); await library.history()
}
async function search() {
  if (!searchText.value.trim()) return
  if (!library.profile) { await library.connect(); return }
  setScene('library'); library.search(searchText.value)
}
async function playTrack(track: Track) { await player.play(track, displayedTracks.value) }
async function playAll(shuffled = false) {
  busyAll.value = true
  try {
    if (library.view === 'playlist') await library.completePlaylist()
    const tracks = displayedTracks.value
    if (!tracks.length) return
    player.setShuffle(shuffled)
    const track = tracks[shuffled ? Math.floor(Math.random() * tracks.length) : 0]
    await player.play(track, tracks)
  } catch (cause) { uiError.value = errorMessage(cause) }
  finally { busyAll.value = false }
}
async function logout() {
  try { await player.flushPlayback() }
  catch (cause) { uiError.value = `播放状态保存失败：${errorMessage(cause)}` }
  try {
    await library.logout()
    settingsDialog.value?.close()
    goLibrary()
  } catch (cause) { uiError.value = errorMessage(cause) }
}
function searchTerm(keyword: string) { searchText.value = keyword; void search() }
function focusSearch() { if (!searchText.value.trim() && library.profile) { setScene('library'); library.search('') } }
function dismissNotices() { uiError.value = ''; player.notice = ''; library.loginError = ''; library.favoriteError = ''; savedCatalog.error = '' }
function setVolume(event: Event) { player.updateSettings({ volume: Number((event.target as HTMLInputElement).value) }) }
function seek(event: Event) { player.seek(Number((event.target as HTMLInputElement).value)) }
function keyboard(event: KeyboardEvent) {
  const target = event.target as HTMLElement
  if (event.key === 'Escape' && scene.value === 'lyrics' && !target.closest('dialog')) { event.preventDefault(); setScene('library'); return }
  if ((event.metaKey || event.ctrlKey) && ['f','k'].includes(event.key.toLowerCase())) { event.preventDefault(); setScene('library'); void nextTick(() => searchInput.value?.focus()); return }
  if (target.closest('input,textarea,button,[contenteditable=true],dialog') || event.altKey || event.metaKey || event.ctrlKey) return
  if (event.code === 'Space') { event.preventDefault(); void player.toggle() }
  if (event.code === 'ArrowRight') { event.preventDefault(); player.seek(player.time / 1000 + 5) }
  if (event.code === 'ArrowLeft') { event.preventDefault(); player.seek(player.time / 1000 - 5) }
  if (event.key.toLowerCase() === 'l') setScene(scene.value === 'lyrics' ? 'library' : 'lyrics')
}

onMounted(async () => {
  document.addEventListener('keydown', keyboard)
  document.addEventListener('visibilitychange', observeVisibility)
  player.setVisualActive(false)
  if (desktop) {
    unlistenDownload = await listen<{ owner: string; mid: string; received: number; total: number | null }>('download-progress', event => { if (event.payload.owner === library.profile?.uin) downloads.progress(event.payload) })
    stopCompanions = await bindCompanions(player)
    unlistenDesktop = await listen<string>('desktop-action', event => {
      if (event.payload === 'toggle') void player.toggle()
      else if (event.payload === 'next') void player.next()
      else if (event.payload === 'previous') void player.previous()
      else if (event.payload === 'lyrics') setScene(scene.value === 'lyrics' ? 'library' : 'lyrics')
      else if (event.payload === 'quit') void quitApp()
    })
    renewalTimer = setInterval(() => { if (library.profile && !library.profile.offline) void api.renew().catch(cause => { uiError.value = `登录续期失败：${errorMessage(cause)}` }) }, 15 * 60 * 1000)
    const window = getCurrentWindow()
    unlistenFocus = await window.onFocusChanged(() => { void observeVisibility() })
    unlistenResize = await window.onResized(() => { void observeVisibility() })
    unlistenClose = await window.onCloseRequested(async event => {
      event.preventDefault()
      try {
        await player.flushPlayback()
        if (player.settings.closeToTray) { await api.hideMain(); player.setVisualActive(false) }
        else await api.quit()
      } catch (cause) { uiError.value = `窗口尚未关闭：${errorMessage(cause)}` }
    })
  }
  await Promise.all([library.initialize(), player.initialize()])
})
onUnmounted(() => {
  document.removeEventListener('keydown', keyboard)
  document.removeEventListener('visibilitychange', observeVisibility)
  unlistenResize?.()
  unlistenClose?.()
  unlistenDesktop?.()
  unlistenFocus?.()
  unlistenDownload?.()
  stopCompanions?.()
  clearInterval(renewalTimer)
})
</script>

<template>
  <div class="app-shell" :class="{ 'lyrics-mode': scene === 'lyrics', 'web-demo': demo }">
    <div class="workspace">
      <header class="topbar" :inert="scene === 'lyrics'" :aria-hidden="scene === 'lyrics'" data-tauri-drag-region>
        <div class="window-navigation"><span v-if="demo" class="demo-traffic" aria-hidden="true"><i></i><i></i><i></i></span><button class="icon-button" :disabled="library.view === 'library'" aria-label="返回上一页" @click="goBack"><ChevronLeft :size="22" /></button></div>
        <nav class="top-nav" aria-label="音乐导航"><button :class="{ selected: library.view === 'library' || catalogView }" @click="goLibrary">音乐库</button><button :class="{ selected: library.view === 'playlist' && library.selected?.kind === 'liked' }" @click="openLiked">我喜欢</button><button :class="{ selected: library.view === 'history' }" @click="openHistory">最近播放</button><button :class="{ selected: library.view === 'downloads' }" @click="openDownloads">下载与离线</button></nav>
        <div class="topbar-actions"><span v-if="demo" class="demo-badge" title="示例资料；播放与歌词仅演示交互，不连接真实 QQ 账户">网页演示</span><form class="search-box" role="search" @submit.prevent="search"><Search :size="16" /><input ref="searchInput" v-model="searchText" placeholder="搜索" aria-label="搜索歌曲、歌手、专辑或歌单" maxlength="100" @focus="focusSearch" /><kbd>⌘ K</kbd></form><button class="account-menu" :aria-label="library.profile ? '账户与设置' : '登录 QQ 音乐'" :title="library.profile?.nickname || '登录 QQ 音乐'" @click="library.profile ? showSettings() : library.connect()"><img v-if="library.profile?.avatar" :src="library.profile.avatar" alt="" /><QrCode v-else :size="19" /></button></div>
      </header>


      <main :class="{ 'library-receded': scene === 'lyrics' }" :inert="scene === 'lyrics'" :aria-hidden="scene === 'lyrics'">

        <div ref="contentScroll" class="content-scroll">
          <div v-if="library.profile?.offline" class="inline-warning offline-banner">当前离线，只能播放已下载歌曲和查看本地记录。<button class="text-button" @click="library.initialize">重新连接</button></div>
          <section v-if="library.view === 'library'" class="library-view">
            <div class="page-heading library-heading"><div><h1>{{ library.profile ? `${library.profile.nickname} 的音乐库` : '音乐库' }}</h1></div></div>
            <nav v-if="library.profile" class="collection-tabs library-collection-tabs" aria-label="资料库分类"><button class="selected" @click="goLibrary">歌单</button><button @click="openCatalog('albums')">专辑</button><button @click="openCatalog('artists')">歌手</button></nav>
            <section v-if="!library.profile" class="welcome-card">
              <div class="welcome-icon"><Music2 :size="42" :stroke-width="1.5" /></div><div class="welcome-copy"><h2>登录 QQ 音乐</h2><button class="button primary" :disabled="library.loginWaiting || library.sessionLoading" @click="library.connect"><LoaderCircle v-if="library.loginWaiting || library.sessionLoading" :size="16" class="spinning" /><QrCode v-else :size="16" />{{ library.sessionLoading ? '正在连接账户' : library.loginWaiting ? '等待扫码登录' : '扫码登录' }}</button></div>
            </section>

            <div v-if="library.profile" class="library-overview" :class="{ 'without-favorites': !likedPlaylist }">
              <div v-if="likedPlaylist" class="favorites-card">
                <button class="favorites-open" @click="openLiked"><Heart :size="30" fill="currentColor" /><Heart :size="124" fill="currentColor" class="favorites-watermark" /><span><strong>我喜欢的音乐</strong><small>{{ likedPlaylist.count }} 首歌曲</small></span></button>
                <button class="favorites-play" :disabled="!likedPlaylist.count || busyAll" aria-label="播放我喜欢的音乐" title="播放我喜欢的音乐" @click="playLiked"><LoaderCircle v-if="busyAll" :size="21" class="spinning" /><Play v-else :size="21" fill="currentColor" /></button>
              </div>
              <section class="recent-panel" aria-labelledby="recent-heading">
                <div class="section-heading compact"><h2 id="recent-heading">最近播放</h2><button class="text-button" @click="openHistory">查看全部<ArrowRight :size="14" /></button></div>
                <div v-if="library.recentError" class="inline-error" role="alert"><span>{{ library.recentError }}</span><button class="text-button" @click="library.loadRecent">重试</button></div>
                <div v-else-if="library.recentState === 'loading' && !recentPreview.length" class="recent-loading" role="status"><LoaderCircle :size="20" class="spinning" />正在读取最近播放</div>
                <div v-else-if="recentPreview.length" class="recent-grid"><button v-for="track in recentPreview" :key="track.mid" class="recent-track" :class="{ current: player.current?.mid === track.mid }" :aria-label="`播放 ${track.title}，${track.artist}`" @click="player.play(track, library.recentTracks)"><CoverArt :src="track.cover" /><span class="recent-track-text"><strong>{{ track.title }}</strong><small>{{ track.artist }}</small></span><AudioLines v-if="player.current?.mid === track.mid && player.playing" :size="16" class="recent-playing" /></button></div>
                <div v-else class="recent-empty"><Clock3 :size="24" /><p>还没有播放记录</p><span>播放过的歌曲会出现在这里</span></div>
              </section>
            </div>
            <div v-if="library.profile" class="section-heading playlists-heading"><h2>我的歌单<span v-if="library.playlists.length" class="section-count">{{ library.playlists.length }}</span></h2><button class="text-button" :disabled="library.profile?.offline" @click="playlistDialog?.open('create')"><Plus :size="15" />创建歌单</button><button v-if="library.profile" class="text-button" :disabled="library.libraryState === 'loading'" @click="library.refresh"><RefreshCw :size="14" :class="{ spinning: library.libraryState === 'loading' }" />刷新歌单</button></div>
            <div v-if="library.profile" class="playlist-filters" role="group" aria-label="歌单筛选"><button :class="{ selected: playlistFilter === 'all' }" :aria-pressed="playlistFilter === 'all'" @click="playlistFilter = 'all'">全部</button><button :class="{ selected: playlistFilter === 'created' }" :aria-pressed="playlistFilter === 'created'" @click="playlistFilter = 'created'">我创建的</button><button :class="{ selected: playlistFilter === 'collected' }" :aria-pressed="playlistFilter === 'collected'" @click="playlistFilter = 'collected'">我收藏的</button></div>
            <div v-if="library.libraryError" class="inline-error" role="alert"><span>{{ library.libraryError }}</span><button class="text-button" @click="library.refresh">重试</button></div>
            <p v-for="warning in library.warnings" :key="warning" class="inline-warning">{{ warning }}</p>
            <div v-if="library.libraryState === 'loading' && !library.playlists.length" class="playlist-grid" aria-label="正在加载歌单"><div v-for="index in 4" :key="index" class="playlist-skeleton"><div></div><span></span><small></small></div></div>
            <div v-else-if="visiblePlaylists.length" class="playlist-grid"><button v-for="playlist in visiblePlaylists" :key="playlist.id" class="playlist-card" @click="openPlaylist(playlist)"><div class="playlist-art"><CoverArt :src="playlist.cover" :alt="playlist.name" :liked="playlist.kind === 'liked'" /><span class="playlist-open"><ArrowRight :size="21" /></span></div><strong>{{ playlist.name }}</strong><span>{{ playlist.count }} 首歌曲<span class="middle-dot">·</span>{{ playlist.kind === 'collected' ? '收藏的歌单' : playlist.kind === 'liked' ? '我喜欢' : '创建的歌单' }}</span></button></div>
            <div v-else-if="library.libraryState === 'ready'" class="empty-state"><ListMusic :size="32" :stroke-width="1.4" /><h3>{{ playlistFilter === 'collected' ? '还没有收藏的歌单' : '这里还没有歌单' }}</h3><p>在 QQ 音乐中添加歌单后，点击刷新同步。</p></div>
          </section>

          <CatalogView v-else-if="catalogView" :view="(library.view as 'artists' | 'albums' | 'artist' | 'album')" :artist-mid="library.artistMid" :album-id="library.albumId" :current-mid="player.current?.mid" :playing="player.playing" @library="goLibrary" @navigate="openCatalog" @artist="openTrackArtist" @album="openTrackAlbum" @play="playCatalog" @shuffle="shuffleCatalog" @play-next="track => player.enqueue(track, true)" @enqueue="player.enqueue" @actions="track => trackActions?.open(track)" />
          <DownloadsView v-else-if="library.view === 'downloads'" />
          <SearchView v-else-if="library.view === 'search'" :keyword="library.query" :current-mid="player.current?.mid" :playing="player.playing" @query="searchTerm" @artist="mid => openCatalog('artist', mid)" @album="mid => openCatalog('album', mid)" @playlist="openPlaylist" @track-artist="openTrackArtist" @track-album="openTrackAlbum" @play="playCatalog" @play-next="track => player.enqueue(track, true)" @enqueue="player.enqueue" @actions="track => trackActions?.open(track)" />
          <section v-else class="tracks-view">
            <div v-if="library.view === 'playlist' && library.selected" class="playlist-hero"><CoverArt :src="library.selected.cover" :liked="library.selected.kind === 'liked'" class="playlist-hero-cover" /><div class="playlist-hero-info"><span class="content-kind">{{ library.selected.kind === 'liked' ? '我喜欢' : library.selected.kind === 'collected' ? '收藏的歌单' : library.selected.kind === 'public' ? '歌单' : '创建的歌单' }}</span><h1>{{ trackTitle }}</h1><p>{{ library.selected.kind === 'public' ? library.selected.creator : library.profile?.nickname }}<span class="middle-dot">·</span>{{ library.total }} 首歌曲</p><div class="playlist-actions"><button class="button primary" :disabled="!library.tracks.length || busyAll || filterLoading" @click="playAll()"><LoaderCircle v-if="busyAll" :size="16" class="spinning" /><Play v-else :size="15" fill="currentColor" />播放全部</button><button class="button secondary" :disabled="!library.tracks.length || busyAll || filterLoading" @click="playAll(true)"><Shuffle :size="16" />随机播放</button><template v-if="library.selected.kind === 'created'"><button class="text-button" :disabled="library.profile?.offline" @click="playlistDialog?.open('rename', library.selected)">重命名</button><button class="text-button" :disabled="library.profile?.offline" @click="playlistDialog?.open('delete', library.selected)">删除歌单</button></template></div></div></div>
            <div v-else class="page-heading tracks-heading"><div><h1>{{ trackTitle }}</h1><p>{{ library.total }} 首歌曲</p></div><button v-if="library.view === 'history' && library.tracks.length" class="button primary" :disabled="busyAll" @click="playAll()"><Play :size="15" fill="currentColor" />播放全部</button></div>
            <div v-if="library.state === 'loading'" class="empty-state" role="status"><LoaderCircle :size="28" class="spinning" /><p>正在加载歌曲</p></div>
            <div v-if="library.error" class="inline-error" role="alert"><span>{{ library.error }}</span><button class="text-button" @click="library.view === 'playlist' && library.selected ? library.openPlaylist(library.selected) : library.history()">重试</button></div>
            <div v-if="library.tracks.length" class="track-tools"><label class="track-filter"><Search :size="15" /><input v-model="trackFilter" aria-label="在歌单中查找" :placeholder="library.view === 'playlist' ? '查找歌名、歌手或专辑' : '筛选已加载的歌曲'" /></label><select v-model="trackSort" aria-label="歌曲排序"><option value="default">原始顺序</option><option value="title">按歌名</option><option value="artist">按歌手</option><option value="album">按专辑</option><option value="duration">按时长</option></select><button class="text-button" :disabled="trackSort === 'default'" :aria-label="sortDescending ? '切换升序' : '切换降序'" @click="sortDescending = !sortDescending">{{ sortDescending ? '降序' : '升序' }}</button><span class="filter-count"><LoaderCircle v-if="filterLoading" :size="13" class="spinning" />{{ filterLoading ? '正在读取完整歌单' : `${displayedTracks.length} 首` }}</span></div>
            <TrackList v-if="library.tracks.length" :tracks="displayedTracks" :current-mid="player.current?.mid" :playing="player.playing" @artist="openTrackArtist" @album="openTrackAlbum" @play="playTrack" @play-next="track => player.enqueue(track, true)" @enqueue="player.enqueue" @actions="track => trackActions?.open(track)" />
            <p v-if="library.tracks.length && !displayedTracks.length && !filterLoading" class="empty-state">没有匹配的歌曲，试试其他歌名或歌手。</p>
            <div v-if="library.state === 'ready' && !library.tracks.length" class="empty-state"><Music2 :size="32" :stroke-width="1.4" /><h3>{{ library.view === 'history' ? '还没有播放记录' : '歌单中还没有歌曲' }}</h3><p>{{ library.view === 'history' ? '播放过的歌曲会出现在这里。' : '在 QQ 音乐中添加歌曲后刷新。' }}</p></div>
            <div v-if="library.hasMore" class="load-more"><button class="button secondary" :disabled="library.loadingMore" @click="library.loadMore"><LoaderCircle v-if="library.loadingMore" class="spinning" :size="15" />{{ library.loadingMore ? '正在加载' : '加载更多' }}</button><span v-if="library.view === 'playlist'">已加载 {{ library.tracks.length }} / {{ library.total }} 首</span></div>
          </section>
        </div>
      </main>
      <Transition name="lyrics-reveal">
        <div v-if="scene === 'lyrics'" class="lyrics-layer" aria-label="歌词页面">
      <header class="lyrics-toolbar" data-tauri-drag-region>
        <button class="lyric-back" aria-label="返回音乐资料库" title="返回（Esc）" @click="setScene('library')"><ChevronDown :size="21" /></button>
        <div class="lyric-options"><button :class="{ selected: showTranslation }" :aria-pressed="showTranslation" @click="showTranslation = !showTranslation">译文</button><button :class="{ selected: showRomanization }" :aria-pressed="showRomanization" @click="showRomanization = !showRomanization">音译</button><button class="icon-button" aria-label="设置" @click="showSettings()"><SlidersHorizontal :size="18" /></button></div>
      </header>
        <LyricsView :show-translation="showTranslation" :show-romanization="showRomanization" @artist="openTrackArtist"><template #controls><PlayerDock lyrics @lyrics="setScene('library')" @queue="queueDialog?.showModal()" /></template></LyricsView>
        </div>
      </Transition>

      <div v-if="player.playbackError" class="playback-error" role="alert"><span>{{ player.playbackError }}</span><button v-if="player.current" class="text-button" @click="player.play(player.current)">重试</button><button class="icon-button" aria-label="关闭播放错误" @click="player.playbackError = ''"><X :size="14" /></button></div>
      <PlayerDock v-if="scene === 'library'" @lyrics="setScene('lyrics')" @queue="queueDialog?.showModal()" />
    </div>

    <div v-if="notices.length" class="notice-toast" role="alert"><span>{{ notices[0] }}</span><button class="icon-button" aria-label="关闭提示" @click="dismissNotices"><X :size="16" /></button></div>

    <dialog ref="loginDialog" class="login-dialog" aria-labelledby="login-title" @cancel.prevent="library.cancelLogin">
      <button class="icon-button login-close" aria-label="取消扫码登录" @click="library.cancelLogin"><X :size="19" /></button>
      <span class="login-brand"><AudioLines :size="24" :stroke-width="1.8" /></span>
      <h2 id="login-title">登录 QQ 音乐</h2>
      <div class="login-methods" role="group" aria-label="扫码方式"><button v-for="method in ([['qq','QQ'],['wx','微信'],['mobile','QQ 音乐']] as const)" :key="method[0]" :class="{ selected: library.loginKind === method[0] }" :aria-pressed="library.loginKind === method[0]" @click="library.setLoginKind(method[0])">{{ method[1] }}</button></div><p class="login-instruction">打开 <strong>{{ loginApp }}</strong>，扫一扫</p>
      <div class="qr-frame" :class="{ 'qr-inactive': ['expired','refused','error'].includes(library.loginStatus) }">
        <img v-if="library.loginQr" :src="library.loginQr.image" :alt="`QQ 音乐登录二维码，请使用${loginApp}扫码`" />
        <div v-if="library.loginStatus === 'loading'" class="qr-loading"><LoaderCircle :size="28" class="spinning" /><span>正在获取二维码</span></div>
        <div v-if="['expired','refused','error'].includes(library.loginStatus)" class="qr-overlay"><RefreshCw :size="26" /><strong>{{ library.loginStatus === 'expired' ? '二维码已过期' : library.loginStatus === 'refused' ? '你已取消授权' : '二维码暂不可用' }}</strong><button class="button primary" @click="library.connect">刷新二维码</button></div>
      </div>
      <div class="qr-status" role="status"><template v-if="library.loginStatus === 'scanned'"><Check :size="15" /><span>已扫码，请在{{ loginApp }}上确认登录</span></template><template v-else-if="library.loginStatus === 'waiting'"><span class="live-dot pulsing"></span><span>等待扫码 · 二维码 3 分钟内有效</span></template><span v-else-if="library.loginStatus === 'error'">{{ library.loginError }}</span><span v-else>扫码后会自动同步你的音乐资料库</span></div>
      <div class="login-privacy">QQ 官方扫码授权<span>·</span>登录凭据仅存此 Mac</div>
    </dialog>

    <dialog ref="settingsDialog" class="settings-dialog" aria-labelledby="settings-title">
      <div class="dialog-heading"><h2 id="settings-title">设置</h2><button class="icon-button" aria-label="关闭设置" @click="settingsDialog?.close()"><X :size="19" /></button></div>
      <SettingsPanel ref="settingsPanel" :demo="demo" @logout="logout" @connect="settingsDialog?.close(); library.connect()" @companion="showCompanion" @downloads="settingsDialog?.close(); openDownloads()" />
    </dialog>

    <PlaylistDialog ref="playlistDialog" /><TrackActions ref="trackActions" />

    <dialog ref="queueDialog" class="queue-dialog" aria-labelledby="queue-title">
      <div class="dialog-heading"><div><h2 id="queue-title">播放队列</h2></div><button class="icon-button" aria-label="关闭待播列表" @click="queueDialog?.close()"><X :size="19" /></button></div>
      <div class="queue-toolbar"><span class="queue-count">{{ player.queue.length }} 首歌曲</span><button class="text-button" :disabled="player.queue.length <= (player.current ? 1 : 0)" title="清空待播，保留当前歌曲" @click="player.clearQueue">清空待播</button></div>
      <p v-if="player.restored" class="queue-hint" role="status">已恢复上次队列与进度，点击播放继续。</p>
      <div v-if="!player.queue.length" class="empty-state"><ListMusic :size="34" :stroke-width="1.2" /><p>在歌曲列表中添加，或播放一首歌。</p></div>
      <div class="queue-items">
        <div v-for="(track, index) in player.queue" :key="track.mid" class="queue-row" :class="{ current: player.current?.mid === track.mid }">
          <button class="queue-play" :aria-label="`播放 ${track.title}，${track.artist}`" @click="player.restored && player.current?.mid === track.mid ? player.toggle() : player.play(track)"><span class="queue-index"><AudioLines v-if="player.current?.mid === track.mid" :size="15" /><span v-else>{{ index + 1 }}</span></span><CoverArt :src="track.cover" /><span class="queue-track"><strong>{{ track.title }}</strong><small>{{ track.artist }}<span v-if="player.nextMid === track.mid" class="queue-next">下一首</span></small></span><span class="queue-duration">{{ formatTime(track.duration) }}</span></button>
          <div class="queue-actions"><button class="icon-button small" :disabled="index === 0" :aria-label="`上移 ${track.title}`" title="上移" @click="player.moveInQueue(track.mid, -1)"><ArrowUp :size="14" /></button><button class="icon-button small" :disabled="index === player.queue.length - 1" :aria-label="`下移 ${track.title}`" title="下移" @click="player.moveInQueue(track.mid, 1)"><ArrowDown :size="14" /></button><button class="icon-button small" :disabled="player.current?.mid === track.mid" :aria-label="`移除 ${track.title}`" :title="player.current?.mid === track.mid ? '保留当前歌曲' : '从队列移除'" @click="player.removeFromQueue(track.mid)"><X :size="14" /></button></div>
        </div>
      </div>

    </dialog>
  </div>
</template>
