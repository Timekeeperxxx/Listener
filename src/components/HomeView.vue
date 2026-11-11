<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowUpRight, AudioLines, Download, Heart, ListMusic, LoaderCircle, Pause, Play, Plus, QrCode, RefreshCw, Search, Shuffle, SlidersHorizontal, X } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import { useCatalogStore } from '../stores/catalog'
import { useSavedCatalogStore } from '../stores/saved-catalog'
import { recommendationDay, useDiscoveryStore } from '../stores/discovery'
import { usePlayerStore } from '../stores/player'
import { filterTracks } from '../lib/track-filter'
import type { HomeNavigation, HomeCategory, Playlist, Track, TrackMenuTarget } from '../types'
import CoverArt from './CoverArt.vue'
import AlbumGrid from './AlbumGrid.vue'
import PlaylistGrid from './PlaylistGrid.vue'
import TrackList from './TrackList.vue'
import CatalogPager from './CatalogPager.vue'

const props = defineProps<{ navigation: HomeNavigation; category: HomeCategory; keyword: string; busy: boolean }>()
const emit = defineEmits<{
  'update:category': [category: HomeCategory]; browse: [category: HomeCategory]; search: [query: string]; downloads: []
  playlist: [playlist: Playlist]; playPlaylist: [playlist: Playlist]; history: []; createPlaylist: []
  album: [mid: string]; artist: [mid: string]; trackArtist: [name: string, track: Track]; trackAlbum: [track: Track]
  play: [track: Track, tracks: Track[]]; shuffle: [tracks: Track[]]; playNext: [track: Track]; enqueue: [track: Track]; actions: [target: TrackMenuTarget]
}>()
const library = useLibraryStore(), catalog = useCatalogStore(), saved = useSavedCatalogStore(), discovery = useDiscoveryStore(), player = usePlayerStore()
const category = computed({ get: () => props.category, set: value => emit('update:category', value) })
const playlistFilter = ref<'all' | 'created' | 'collected'>('all')
const reverse = ref(false), selectedRecent = ref(''), songQuery = ref(''), directoryQuery = ref('')
const searchTerm = computed(() => props.keyword.trim().toLocaleLowerCase())
const searching = computed(() => props.navigation === 'favorites' && !!searchTerm.value)
const matches = (text: string) => text.toLocaleLowerCase().includes(searchTerm.value)
const likedPlaylist = computed(() => library.playlists.find(item => item.kind === 'liked'))
const personalPlaylists = computed(() => library.playlists.filter(item => item.kind !== 'liked'))
const ownPlaylists = computed(() => personalPlaylists.value.filter(item => matches(item.name) && (searching.value || playlistFilter.value === 'all' || item.kind === playlistFilter.value)))
const likedTracks = computed(() => {
  const items = filterTracks(library.likedTracks, props.keyword, 'default', false)
  return reverse.value ? [...items].reverse() : items
})
const savedAlbums = computed(() => saved.items.albums.filter(item => matches(`${item.name} ${item.credit}`)))
const savedArtists = computed(() => saved.items.artists.filter(item => matches(item.name)))
const recent = computed(() => library.recentTracks.slice(0, 6))
const spotlight = computed(() => discovery.daily?.playlist)
const categories = computed(() => [
  { id: 'songs' as const, name: props.navigation === 'favorites' ? '喜欢的歌曲' : '歌曲', count: props.navigation === 'favorites' ? likedPlaylist.value?.count : undefined },
  { id: 'albums' as const, name: '专辑', count: props.navigation === 'favorites' ? saved.items.albums.length : undefined },
  { id: 'artists' as const, name: '歌手', count: props.navigation === 'favorites' ? saved.items.artists.length : undefined },
  { id: 'playlists' as const, name: '歌单', count: props.navigation === 'favorites' ? personalPlaylists.value.length : undefined },
])
const favoriteEmpty = computed(() => !likedTracks.value.length && !savedAlbums.value.length && !savedArtists.value.length && !ownPlaylists.value.length)
const favoriteReady = computed(() => library.likedState === 'ready' && saved.state === 'ready' && library.libraryState === 'ready')
const regions = computed(() => category.value === 'artists' ? [{ id: -100, name: '全部地区' }, { id: 200, name: '内地' }, { id: 2, name: '港台' }, { id: 5, name: '欧美' }, { id: 4, name: '日本' }, { id: 3, name: '韩国' }] : [{ id: 1, name: '内地' }, { id: 2, name: '港台' }, { id: 3, name: '欧美' }, { id: 5, name: '日本' }, { id: 4, name: '韩国' }])
const directory = computed(() => category.value === 'artists' ? catalog.artists : catalog.albums)
function loadContent() {
  if (!library.profile) return
  if (props.navigation === 'favorites') { if (library.likedState === 'idle') void library.loadLiked(); return }
  if (props.navigation === 'recommend') {
    if (discovery.dailyState === 'idle' || (discovery.daily && discovery.daily.day !== recommendationDay())) void discovery.loadDaily()
    if (!library.profile.offline && discovery.radioState === 'idle') void discovery.loadRadio()
    return
  }
  if (library.profile.offline) return
  if (props.navigation === 'discover') {
    if (category.value === 'albums' && catalog.albums.state === 'idle') void catalog.loadAlbums()
    if (category.value === 'artists' && catalog.artists.state === 'idle') void catalog.loadArtists()
    if (category.value === 'playlists' && discovery.playlistState === 'idle') void discovery.loadPlaylists()
  }
}
function refreshPage() {
  if (props.navigation === 'recommend') { void discovery.loadDaily(true); void discovery.loadRadio(); void library.loadRecent() }
  else if (props.navigation === 'favorites') void library.refresh()
  else if (category.value === 'artists') void catalog.loadArtists()
  else if (category.value === 'albums') void catalog.loadAlbums()
  else void discovery.loadPlaylists()
}
function checkDailyDate() {
  if (props.navigation === 'recommend' && library.profile && discovery.daily && discovery.daily.day !== recommendationDay()) void discovery.loadDaily()
}
let dailyClock: ReturnType<typeof setInterval>
onMounted(() => { dailyClock = setInterval(checkDailyDate, 60_000); document.addEventListener('visibilitychange', checkDailyDate) })
onUnmounted(() => { clearInterval(dailyClock); document.removeEventListener('visibilitychange', checkDailyDate) })
watch(() => props.navigation, () => { playlistFilter.value = 'all'; reverse.value = false; directoryQuery.value = '' })
watch([() => props.navigation, category, () => library.profile?.uin], loadContent, { immediate: true, flush: 'post' })
function chooseCategory(value: HomeCategory) { category.value = value; directoryQuery.value = value === 'artists' ? catalog.artistQuery : catalog.albumQuery }
function searchDirectory() {
  if (category.value === 'artists') { catalog.artistQuery = directoryQuery.value.trim(); void catalog.loadArtists() }
  else { catalog.albumQuery = directoryQuery.value.trim(); void catalog.loadAlbums() }
}
function changeRegion(event: Event) {
  const area = Number((event.target as HTMLSelectElement).value)
  if (category.value === 'artists') { catalog.artistArea = area; void catalog.loadArtists() }
  else { catalog.albumArea = area; void catalog.loadAlbums() }
}
function toggleRecent(track: Track) { if (player.current?.mid === track.mid) void player.toggle(); else emit('play', track, library.recentTracks) }
function showSongs() { if (songQuery.value.trim()) emit('search', songQuery.value.trim()) }
watch(category, value => { directoryQuery.value = value === 'artists' ? catalog.artistQuery : catalog.albumQuery }, { immediate: true })
</script>

<template>
  <section class="home-view">
    <div class="page-heading home-heading"><div><h1>{{ searching ? '搜索我的收藏' : navigation === 'recommend' ? '推荐' : navigation === 'discover' ? '发现' : '我的收藏' }}</h1><p v-if="navigation === 'favorites'">{{ searching ? `“${keyword}”` : `${likedPlaylist?.count ?? 0} 首歌曲 · ${saved.items.albums.length} 张专辑 · ${saved.items.artists.length} 位歌手` }}</p></div><div class="home-heading-actions"><button v-if="navigation === 'favorites'" class="icon-button" aria-label="下载与离线" @click="emit('downloads')"><Download :size="19"/></button><button v-if="library.profile" class="icon-button" aria-label="刷新当前页面" @click="refreshPage"><RefreshCw :size="16"/></button></div></div>
    <p v-for="warning in library.warnings" :key="warning" class="inline-warning" role="status">{{ warning }}</p>
    <section v-if="!library.profile" class="welcome-card"><QrCode :size="38"/><div><h2>登录 QQ 音乐</h2><button class="button primary" :disabled="library.sessionLoading || library.loginWaiting" @click="library.connect"><LoaderCircle v-if="library.sessionLoading" class="spinning" :size="16"/><QrCode v-else :size="16"/>{{ library.sessionLoading ? '正在连接账户' : '扫码登录' }}</button></div></section>

    <template v-else-if="navigation === 'recommend'">
      <div class="listening-overview">
        <section v-if="spotlight && discovery.daily" class="recommend-feature"><button class="feature-open" aria-label="查看每日30首" @click="emit('playlist', spotlight)"><span class="feature-label">QQ 音乐</span><h2>每日30首</h2><p>{{ spotlight.count }} 首歌曲</p><span class="feature-covers"><CoverArt v-for="track in discovery.daily.tracks.slice(0, 3)" :key="track.mid" :src="track.cover" :alt="track.album"/></span></button><button class="feature-play" :disabled="busy" aria-label="播放每日30首" @click="emit('play', discovery.daily.tracks[0], discovery.daily.tracks)"><LoaderCircle v-if="busy" class="spinning" :size="14"/><Play v-else :size="14" fill="currentColor"/>播放全部<ArrowUpRight :size="14"/></button></section>
        <section v-else class="recommend-feature recommendation-status" :aria-busy="discovery.dailyState === 'loading'"><h2>每日30首</h2><LoaderCircle v-if="discovery.dailyState === 'loading'" :size="24" class="spinning" aria-label="正在加载每日推荐"/><p v-if="discovery.dailyError" role="alert">{{ discovery.dailyError }}</p><button v-if="discovery.dailyState === 'error'" class="text-button" @click="discovery.loadDaily(true)">重试</button></section>
        <section class="recent-panel"><div class="section-heading compact"><h2>最近播放<span class="section-dot"></span></h2><button class="text-button" @click="emit('history')">查看全部<ArrowUpRight :size="14"/></button></div><div v-if="library.recentError" class="inline-error" role="alert">{{ library.recentError }}<button class="text-button" @click="library.loadRecent">重试</button></div><div v-else-if="library.recentState === 'loading' && !recent.length" class="recent-loading"><LoaderCircle :size="20" class="spinning"/></div><div v-else-if="recent.length" class="recent-grid"><div v-for="track in recent" :key="track.mid" class="recent-item" :class="{ current: player.current?.mid === track.mid, selected: selectedRecent === track.mid }" @click="selectedRecent = track.mid" @dblclick="emit('play', track, library.recentTracks)"><button class="recent-cover" :aria-label="`${player.current?.mid === track.mid && player.playing ? '暂停' : '播放'} ${track.title}`" @click.stop="toggleRecent(track)" @dblclick.stop><CoverArt :src="track.cover"/><span><Pause v-if="player.current?.mid === track.mid && player.playing" :size="14" fill="currentColor"/><Play v-else :size="14" fill="currentColor"/></span></button><button class="recent-text" @click.stop="selectedRecent = track.mid" @dblclick.stop="emit('play', track, library.recentTracks)" @keydown.enter.prevent.stop="emit('play', track, library.recentTracks)"><strong>{{ track.title }}</strong><small>{{ track.artist }}</small></button><AudioLines v-if="player.current?.mid === track.mid && player.playing" class="now-indicator" :size="16"/></div></div><div v-else class="recent-empty"><p>暂无播放记录</p></div></section>
      </div>
      <section class="home-section recommendation-radio" aria-label="猜你喜欢"><div class="section-heading"><h2>猜你喜欢</h2><button class="text-button" :disabled="discovery.radioState === 'loading' || library.profile.offline" @click="discovery.loadRadio"><RefreshCw :size="14" :class="{ spinning: discovery.radioState === 'loading' }"/>换一批</button></div><TrackList :tracks="discovery.radio" :current-mid="player.current?.mid" :playing="player.playing" @play="track => emit('play', track, discovery.radio)" @play-next="track => emit('playNext', track)" @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', target)" @artist="(name, track) => emit('trackArtist', name, track)" @album="track => emit('trackAlbum', track)"/><CatalogPager :state="discovery.radioState" :error="discovery.radioError" :has-more="false" :loaded="discovery.radio.length" :total="discovery.radio.length" @load="discovery.loadRadio"/><p v-if="library.profile.offline && !discovery.radio.length" class="inline-warning">连接网络后可获取猜你喜欢。</p></section>
    </template>

    <template v-else>
      <nav v-if="!searching" class="collection-tabs home-tabs" aria-label="音乐分类"><button v-for="item in categories" :key="item.id" :class="{ selected: category === item.id }" :aria-pressed="category === item.id" @click="chooseCategory(item.id)">{{ item.name }}<span v-if="item.count !== undefined">{{ item.count }}</span></button><button v-if="navigation === 'favorites' && category === 'songs'" class="home-sort" aria-label="反转歌曲顺序" :aria-pressed="reverse" @click="reverse = !reverse"><SlidersHorizontal :size="16"/></button></nav>
      <template v-if="navigation === 'favorites'">
        <div v-if="library.libraryError" class="inline-error" role="alert">{{ library.libraryError }}<button class="text-button" @click="library.refresh">重试</button></div>
        <div v-if="saved.error" class="inline-error" role="alert">{{ saved.error }}<button class="text-button" @click="saved.load">重试</button></div>
        <section v-if="category === 'songs' || searching" class="favorite-songs">
          <div v-if="searching && likedTracks.length" class="section-heading"><h2>喜欢的歌曲<span class="section-count">{{ likedTracks.length }}</span></h2></div>
          <div v-if="library.favoriteError" class="inline-error" role="alert">{{ library.favoriteError }}<button class="text-button" @click="library.loadLiked(true)">重试</button></div>
          <div v-if="library.likedState === 'loading' && !library.likedTracks.length" class="empty-state" role="status"><LoaderCircle :size="24" class="spinning"/><p>正在读取喜欢的歌曲</p></div>
          <template v-if="likedTracks.length"><div class="song-list-actions"><button class="button primary" @click="emit('play', likedTracks[0]!, likedTracks)"><Play :size="14" fill="currentColor"/>播放全部</button><button class="button secondary" @click="emit('shuffle', likedTracks)"><Shuffle :size="15"/>随机播放</button><small>{{ likedTracks.length }} 首歌曲</small></div><TrackList :tracks="likedTracks" :current-mid="player.current?.mid" :playing="player.playing" @play="track => emit('play', track, likedTracks)" @artist="(name, track) => emit('trackArtist', name, track)" @album="track => emit('trackAlbum', track)" @play-next="track => emit('playNext', track)" @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', target)"/></template>
          <div v-else-if="!searching && library.likedState === 'ready'" class="empty-state"><Heart :size="30"/><h3>还没有喜欢的歌曲</h3></div>
        </section>
        <section v-if="category === 'albums' || searching" class="saved-section"><div v-if="searching && savedAlbums.length" class="section-heading"><h2>专辑</h2></div><AlbumGrid :albums="savedAlbums" @album="mid => emit('album', mid)" @artist="mid => emit('artist', mid)"/><div v-if="!searching && saved.state === 'ready' && !savedAlbums.length" class="empty-state"><Heart :size="30"/><h3>还没有收藏的专辑</h3></div></section>
        <section v-if="category === 'artists' || searching" class="saved-section"><div v-if="searching && savedArtists.length" class="section-heading"><h2>歌手</h2></div><div class="artist-grid"><button v-for="artist in savedArtists" :key="artist.mid" class="artist-card" :aria-label="`查看歌手 ${artist.name}`" @click="emit('artist', artist.mid)"><CoverArt :src="artist.image" :alt="artist.name"/><strong>{{ artist.name }}</strong></button></div><div v-if="!searching && saved.state === 'ready' && !savedArtists.length" class="empty-state"><Heart :size="30"/><h3>还没有关注的歌手</h3></div></section>
        <section v-if="category === 'playlists' || searching" class="saved-section"><div v-if="searching && ownPlaylists.length" class="section-heading"><h2>歌单</h2></div><div v-if="!searching" class="personal-playlist-tools"><div class="playlist-filters" aria-label="歌单筛选"><button v-for="item in ([['all','全部'],['created','我创建的'],['collected','我收藏的']] as const)" :key="item[0]" :class="{ selected: playlistFilter === item[0] }" @click="playlistFilter = item[0]">{{ item[1] }}</button></div><button class="text-button" :disabled="library.profile.offline" @click="emit('createPlaylist')"><Plus :size="14"/>创建歌单</button></div><PlaylistGrid :playlists="ownPlaylists" @open="item => emit('playlist', item)"/><div v-if="!searching && library.libraryState === 'ready' && !ownPlaylists.length" class="empty-state"><ListMusic :size="30"/><h3>暂无歌单</h3></div></section>
        <div v-if="searching && favoriteReady && favoriteEmpty" class="empty-state"><Search :size="30"/><h3>收藏中没有匹配的内容</h3></div>
      </template>
      <template v-else-if="category === 'albums' || category === 'artists'">
        <div class="directory-toolbar"><form class="catalog-search" @submit.prevent="searchDirectory"><Search :size="15"/><input v-model="directoryQuery" :aria-label="category === 'artists' ? '搜索所有歌手' : '搜索所有专辑'" :placeholder="category === 'artists' ? '搜索歌手' : '搜索专辑或歌手'"/><button v-if="directoryQuery" type="button" aria-label="清除目录搜索" @click="directoryQuery = ''; searchDirectory()"><X :size="14"/></button></form><select :value="category === 'artists' ? catalog.artistArea : catalog.albumArea" aria-label="地区" @change="changeRegion"><option v-for="region in regions" :key="region.id" :value="region.id">{{ region.name }}</option></select></div>
        <AlbumGrid v-if="category === 'albums'" :albums="catalog.albums.items" @album="mid => emit('album', mid)" @artist="mid => emit('artist', mid)"/><div v-else class="artist-grid"><button v-for="artist in catalog.artists.items" :key="artist.mid" class="artist-card" :aria-label="`查看歌手 ${artist.name}`" @click="emit('artist', artist.mid)"><CoverArt :src="artist.image" :alt="artist.name"/><strong>{{ artist.name }}</strong></button></div><CatalogPager :state="directory.state" :error="directory.error" :has-more="directory.hasMore" :loaded="directory.items.length" :total="directory.total" @load="category === 'artists' ? catalog.loadArtists(!!catalog.artists.items.length) : catalog.loadAlbums(!!catalog.albums.items.length)"/><div v-if="directory.state === 'ready' && !directory.items.length" class="empty-state"><Search :size="30"/><h3>没有匹配的内容</h3></div>
      </template>
      <template v-else-if="category === 'playlists'"><div class="playlist-filters" aria-label="歌单分类"><button v-for="query in ['华语流行','欧美流行','华语经典','纯音乐']" :key="query" :class="{ selected: discovery.playlistQuery === query }" @click="discovery.loadPlaylists(query)">{{ query }}</button></div><PlaylistGrid :playlists="discovery.playlists.items" @open="item => emit('playlist', item)"/><CatalogPager :state="discovery.playlistState" :error="discovery.playlistError" :has-more="discovery.playlists.hasMore" :loaded="discovery.playlists.items.length" :total="discovery.playlists.total" @load="discovery.loadPlaylists(discovery.playlistQuery, discovery.playlists.items.length > 0)"/></template>
      <div v-else class="catalog-song-search"><form class="catalog-search" @submit.prevent="showSongs"><Search :size="18"/><input v-model="songQuery" aria-label="搜索全部歌曲" placeholder="搜索歌名、歌手或专辑"/><button class="text-button" type="submit">搜索</button></form></div>
    </template>
  </section>
</template>
