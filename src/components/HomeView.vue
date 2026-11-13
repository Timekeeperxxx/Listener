<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowUpRight, ChevronLeft, ChevronRight, Clock3, Download, Heart, ListMusic, LoaderCircle, Play, Plus, QrCode, RefreshCw, Search, Shuffle, SlidersHorizontal, X } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import { useCatalogStore } from '../stores/catalog'
import { useSavedCatalogStore } from '../stores/saved-catalog'
import { recommendationDay, useDiscoveryStore } from '../stores/discovery'
import { usePlayerStore } from '../stores/player'
import { filterTracks, type TrackSort } from '../lib/track-filter'
import type { HomeNavigation, HomeCategory, Playlist, Track, TrackMenuTarget } from '../types'
import CoverArt from './CoverArt.vue'
import AlbumGrid from './AlbumGrid.vue'
import PlaylistGrid from './PlaylistGrid.vue'
import TrackList from './TrackList.vue'
import CatalogPager from './CatalogPager.vue'

const props = defineProps<{ navigation: HomeNavigation; category: HomeCategory; keyword: string; busy: boolean }>()
const emit = defineEmits<{
  'update:category': [category: HomeCategory]; browse: [category: HomeCategory]; search: [query: string]; downloads: []
  playlist: [playlist: Playlist]; createPlaylist: []
  album: [mid: string]; artist: [mid: string]; trackArtist: [name: string, track: Track]; trackAlbum: [track: Track]
  play: [track: Track, tracks: Track[]]; shuffle: [tracks: Track[]]; playNext: [track: Track]; enqueue: [track: Track]; actions: [target: TrackMenuTarget]
}>()
const library = useLibraryStore(), catalog = useCatalogStore(), saved = useSavedCatalogStore(), discovery = useDiscoveryStore(), player = usePlayerStore()
const category = computed({ get: () => props.category, set: value => emit('update:category', value) })
const playlistFilter = ref<'all' | 'created' | 'collected'>('all')
const reverse = ref(false), songQuery = ref(''), directoryQuery = ref('')
const recentQuery = ref(''), recentSort = ref<TrackSort>('default'), recentDescending = ref(false)
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
const recentTracks = computed(() => filterTracks(library.recentTracks, recentQuery.value, recentSort.value, recentDescending.value))
const spotlight = computed(() => discovery.daily?.playlist)
const spotlightCovers = computed(() => [...new Set(discovery.daily?.tracks.map(track => track.cover).filter(Boolean))].slice(0, 3))
const pageSize = 4
const recommendedSongPage = ref(0), recommendedPlaylistPage = ref(0)
const recommendedTracks = computed(() => {
  const desktop = discovery.desktopSongs?.tracks ?? []
  const radio = discovery.radio
  const tracks = new Map<string, Track>()
  for (let index = 0; index < Math.max(desktop.length, radio.length); index++) {
    for (const track of [desktop[index], radio[index]]) {
      if (track && !tracks.has(track.mid)) tracks.set(track.mid, track)
    }
  }
  return [...tracks.values()]
})
const songLoading = computed(() => discovery.desktopSongsState === 'loading' || discovery.radioState === 'loading')
const songErrors = computed(() => [
  discovery.desktopSongsError && `首页推荐加载失败：${discovery.desktopSongsError}`,
  discovery.radioError && `猜你喜欢加载失败：${discovery.radioError}`,
].filter(Boolean))
const songPreview = computed(() => {
  const start = recommendedSongPage.value * pageSize
  return recommendedTracks.value.slice(start, start + pageSize)
})
const playlistPreview = computed(() => {
  const start = recommendedPlaylistPage.value * pageSize
  return discovery.recommendedPlaylists.slice(start, start + pageSize)
})
watch([() => discovery.desktopSongs, () => discovery.radio], () => { recommendedSongPage.value = 0 })
watch(() => discovery.recommendedPlaylists, () => { recommendedPlaylistPage.value = 0 })
const categories = computed<{ id: HomeCategory; name: string; count?: number }[]>(() => props.navigation === 'favorites' ? [
  { id: 'songs', name: '喜欢的歌曲', count: likedPlaylist.value?.count },
  { id: 'albums', name: '专辑', count: saved.items.albums.length },
  { id: 'artists', name: '歌手', count: saved.items.artists.length },
  { id: 'playlists', name: '歌单', count: personalPlaylists.value.length },
  { id: 'recent', name: '最近播放' },
] : [
  { id: 'songs', name: '歌曲' }, { id: 'albums', name: '专辑' }, { id: 'artists', name: '歌手' }, { id: 'playlists', name: '歌单' },
])
const favoriteEmpty = computed(() => !likedTracks.value.length && !savedAlbums.value.length && !savedArtists.value.length && !ownPlaylists.value.length)
const favoriteReady = computed(() => library.likedState === 'ready' && saved.state === 'ready' && library.libraryState === 'ready')
const regions = computed(() => category.value === 'artists' ? [{ id: -100, name: '全部地区' }, { id: 200, name: '内地' }, { id: 2, name: '港台' }, { id: 5, name: '欧美' }, { id: 4, name: '日本' }, { id: 3, name: '韩国' }] : [{ id: 1, name: '内地' }, { id: 2, name: '港台' }, { id: 3, name: '欧美' }, { id: 5, name: '日本' }, { id: 4, name: '韩国' }])
const directory = computed(() => category.value === 'artists' ? catalog.artists : catalog.albums)
function loadContent() {
  if (!library.profile) return
  if (props.navigation === 'favorites') {
    if (category.value === 'recent') void library.loadRecent()
    else if (library.likedState === 'idle') void library.loadLiked()
    return
  }
  if (props.navigation === 'recommend') {
    if (discovery.dailyState === 'idle' || (discovery.daily && discovery.daily.day !== recommendationDay())) void discovery.loadDaily()
    if (!library.profile.offline) {
      if (discovery.radioState === 'idle') void discovery.loadRadio()
      if (discovery.desktopSongsState === 'idle') void discovery.loadDesktopSongs()
      if (discovery.recommendedPlaylistState === 'idle') void discovery.loadRecommendedPlaylists()
      if (discovery.chartState === 'idle') void discovery.loadCharts()
    }
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
  if (props.navigation === 'recommend') {
    void discovery.loadDaily(true)
    if (!library.profile?.offline) {
      refreshRecommendedTracks()
      void discovery.loadRecommendedPlaylists()
      void discovery.loadCharts()
    }
  } else if (props.navigation === 'favorites') {
    void library.refresh()
    if (category.value === 'recent') void library.loadRecent()
  } else if (category.value === 'artists') void catalog.loadArtists()
  else if (category.value === 'albums') void catalog.loadAlbums()
  else void discovery.loadPlaylists()
}
function refreshRecommendedTracks() {
  void discovery.loadDesktopSongs()
  void discovery.loadRadio()
}
function checkDailyDate() {
  if (props.navigation === 'recommend' && library.profile && discovery.daily && discovery.daily.day !== recommendationDay()) void discovery.loadDaily()
}
let dailyClock: ReturnType<typeof setInterval>
onMounted(() => { dailyClock = setInterval(checkDailyDate, 60_000); document.addEventListener('visibilitychange', checkDailyDate) })
onUnmounted(() => { clearInterval(dailyClock); document.removeEventListener('visibilitychange', checkDailyDate) })
watch(() => props.navigation, () => { playlistFilter.value = 'all'; reverse.value = false; directoryQuery.value = '' })
watch([() => props.navigation, category, () => library.profile?.uin, () => library.profile?.offline], loadContent, { immediate: true, flush: 'post' })
function searchDirectory() {
  if (category.value === 'artists') { catalog.artistQuery = directoryQuery.value.trim(); void catalog.loadArtists() }
  else { catalog.albumQuery = directoryQuery.value.trim(); void catalog.loadAlbums() }
}
function changeRegion(event: Event) {
  const area = Number((event.target as HTMLSelectElement).value)
  if (category.value === 'artists') { catalog.artistArea = area; void catalog.loadArtists() }
  else { catalog.albumArea = area; void catalog.loadAlbums() }
}
function playRecent() {
  const tracks = recentTracks.value
  if (!tracks.length) return
  player.setShuffle(false)
  emit('play', tracks[0], tracks)
}
function showSongs() { if (songQuery.value.trim()) emit('search', songQuery.value.trim()) }
watch(category, value => { directoryQuery.value = value === 'artists' ? catalog.artistQuery : catalog.albumQuery }, { immediate: true })
</script>

<template>
  <section class="home-view" :class="{ 'recommendation-home': navigation === 'recommend' }">
    <div class="page-heading home-heading"><div><h1>{{ searching ? '搜索我的收藏' : navigation === 'recommend' ? '推荐' : navigation === 'discover' ? '发现' : '我的收藏' }}</h1><p v-if="navigation === 'favorites'">{{ searching ? `“${keyword}”` : `${likedPlaylist?.count ?? 0} 首歌曲 · ${saved.items.albums.length} 张专辑 · ${saved.items.artists.length} 位歌手` }}</p></div><div class="home-heading-actions"><button v-if="navigation === 'favorites'" class="icon-button" aria-label="下载与离线" @click="emit('downloads')"><Download :size="19"/></button><button v-if="library.profile" class="icon-button" aria-label="刷新当前页面" @click="refreshPage"><RefreshCw :size="16"/></button></div></div>
    <p v-for="warning in library.warnings" :key="warning" class="inline-warning" role="status">{{ warning }}</p>
    <section v-if="!library.profile" class="welcome-card"><QrCode :size="38"/><div><h2>登录 QQ 音乐</h2><button class="button primary" :disabled="library.sessionLoading || library.loginWaiting" @click="library.connect"><LoaderCircle v-if="library.sessionLoading" class="spinning" :size="16"/><QrCode v-else :size="16"/>{{ library.sessionLoading ? '正在连接账户' : '扫码登录' }}</button></div></section>

    <template v-else-if="navigation === 'recommend'">
      <div class="listening-overview">
        <section class="recommend-feature" aria-labelledby="daily-heading" :aria-busy="discovery.dailyState === 'loading'">
          <div class="recommend-feature-copy">
            <h2 id="daily-heading">每日 30 首</h2>
            <template v-if="spotlight && discovery.daily">
              <p v-if="spotlight.count !== 30">{{ spotlight.count }} 首歌曲</p>
              <div class="feature-actions">
                <button class="feature-play" :disabled="busy" aria-label="播放每日30首" @click="emit('play', discovery.daily.tracks[0], discovery.daily.tracks)">
                  <LoaderCircle v-if="busy" class="spinning" :size="15"/><Play v-else :size="15" fill="currentColor"/>播放全部
                </button>
                <button class="feature-open" aria-label="查看每日30首" @click="emit('playlist', spotlight)">查看歌单<ArrowUpRight :size="15"/></button>
              </div>
            </template>
            <LoaderCircle v-else-if="discovery.dailyState === 'loading'" :size="22" class="spinning" aria-label="正在加载每日推荐"/>
            <template v-else-if="discovery.dailyError">
              <p role="alert">{{ discovery.dailyError }}</p>
              <button class="feature-open" @click="discovery.loadDaily(true)">重试</button>
            </template>
          </div>
          <div v-if="spotlight" class="feature-covers" aria-hidden="true">
            <CoverArt v-for="cover in spotlightCovers" :key="cover" :src="cover"/>
          </div>
        </section>
        <section class="recommendation-radio" aria-labelledby="recommended-songs-heading">
          <div class="section-heading compact">
            <h2 id="recommended-songs-heading">为你推荐</h2>
            <div v-if="!library.profile.offline" class="recommendation-controls">
              <button v-if="recommendedTracks.length" class="icon-button recommendation-play" aria-label="播放为你推荐" title="播放为你推荐" @click="emit('play', recommendedTracks[0], recommendedTracks)"><Play :size="14" fill="currentColor"/></button>
              <button class="icon-button" aria-label="刷新为你推荐" title="刷新推荐歌曲" :disabled="songLoading" @click="refreshRecommendedTracks">
                <RefreshCw :size="14" :class="{ spinning: songLoading }"/>
              </button>
              <div v-if="recommendedTracks.length > pageSize" class="recommendation-paging" role="group" aria-label="推荐歌曲翻页">
                <button class="icon-button" aria-label="上一组推荐歌曲" :disabled="songLoading || recommendedSongPage === 0" @click="recommendedSongPage--"><ChevronLeft :size="16"/></button>
                <button class="icon-button" aria-label="下一组推荐歌曲" :disabled="songLoading || (recommendedSongPage + 1) * pageSize >= recommendedTracks.length" @click="recommendedSongPage++"><ChevronRight :size="16"/></button>
              </div>
            </div>
          </div>
          <p v-if="library.profile.offline" class="inline-warning">连接网络后可获取推荐歌曲。</p>
          <TrackList v-else-if="songPreview.length" :tracks="songPreview" compact cover-playback :current-mid="player.current?.mid" :playing="player.playing"
            @play="track => emit('play', track, recommendedTracks)" @play-next="track => emit('playNext', track)"
            @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', { ...target, tracks: recommendedTracks })"
            @artist="(name, track) => emit('trackArtist', name, track)" @album="track => emit('trackAlbum', track)"/>
          <p v-else-if="songLoading" class="recommendation-status" role="status">正在加载推荐歌曲…</p>
          <p v-for="error in library.profile.offline ? [] : songErrors" :key="error" class="catalog-error recommendation-status" role="alert">{{ error }}</p>
        </section>
      </div>
      <div v-if="!library.profile.offline" class="recommendation-grid">
        <section class="home-section recommended-playlists" aria-labelledby="recommended-playlists-heading">
          <div class="section-heading">
            <h2 id="recommended-playlists-heading">你的歌单补给站</h2>
            <div class="recommendation-controls">
              <button class="text-button" aria-label="刷新推荐歌单" :disabled="discovery.recommendedPlaylistState === 'loading'" @click="discovery.loadRecommendedPlaylists">
                <RefreshCw :size="14" :class="{ spinning: discovery.recommendedPlaylistState === 'loading' }"/>刷新
              </button>
              <div v-if="discovery.recommendedPlaylists.length > pageSize" class="recommendation-paging" role="group" aria-label="推荐歌单翻页">
                <button class="icon-button" aria-label="上一组推荐歌单" :disabled="recommendedPlaylistPage === 0" @click="recommendedPlaylistPage--"><ChevronLeft :size="16"/></button>
                <button class="icon-button" aria-label="下一组推荐歌单" :disabled="(recommendedPlaylistPage + 1) * pageSize >= discovery.recommendedPlaylists.length" @click="recommendedPlaylistPage++"><ChevronRight :size="16"/></button>
              </div>
            </div>
          </div>
          <PlaylistGrid :playlists="playlistPreview" compact @open="item => emit('playlist', item)"/>
          <CatalogPager v-if="!discovery.recommendedPlaylists.length || discovery.recommendedPlaylistError" :state="discovery.recommendedPlaylistState" :error="discovery.recommendedPlaylistError"
            :has-more="false" :loaded="discovery.recommendedPlaylists.length" :total="discovery.recommendedPlaylists.length"
            @load="discovery.loadRecommendedPlaylists"/>
        </section>
        <section class="home-section recommended-charts" aria-labelledby="recommended-charts-heading">
          <div class="section-heading"><h2 id="recommended-charts-heading">排行榜</h2></div>
          <div class="chart-cards">
            <button v-for="{ playlist, preview } in discovery.charts" :key="playlist.id" class="chart-card"
              :aria-label="`查看排行榜 ${playlist.name}`" @click="emit('playlist', playlist)">
              <span class="chart-card-heading">
                <CoverArt :src="playlist.cover"/>
                <strong>{{ playlist.name }}</strong>
              </span>
              <span class="chart-card-copy">
                <span v-for="(song, index) in preview" :key="index" class="chart-preview">
                  <span>{{ index + 1 }}</span>
                  <span :title="song.artist ? `${song.title} · ${song.artist}` : song.title">{{ song.title }}<small v-if="song.artist"> · {{ song.artist }}</small></span>
                </span>
              </span>
              <ArrowUpRight :size="15" class="chart-arrow"/>
            </button>
          </div>
          <CatalogPager :state="discovery.chartState" :error="discovery.chartError"
            :has-more="false" :loaded="discovery.charts.length" :total="discovery.charts.length" @load="discovery.loadCharts"/>
        </section>
      </div>
    </template>

    <template v-else>
      <nav v-if="!searching" class="collection-tabs home-tabs" :class="{ 'favorite-tabs': navigation === 'favorites' }"
        :aria-label="navigation === 'favorites' ? '收藏分类' : '音乐分类'">
        <button v-for="item in categories" :key="item.id" :class="{ selected: category === item.id }"
          :aria-pressed="category === item.id" @click="category = item.id">
          {{ item.name }}<span v-if="item.count !== undefined">{{ item.count }}</span>
        </button>
      </nav>
      <template v-if="navigation === 'favorites'">
        <div v-if="library.libraryError" class="inline-error" role="alert">{{ library.libraryError }}<button class="text-button" @click="library.refresh">重试</button></div>
        <div v-if="saved.error" class="inline-error" role="alert">{{ saved.error }}<button class="text-button" @click="saved.load">重试</button></div>
        <section v-if="category === 'songs' || searching" class="favorite-songs">
          <div v-if="searching && likedTracks.length" class="section-heading"><h2>喜欢的歌曲<span class="section-count">{{ likedTracks.length }}</span></h2></div>
          <div v-if="library.favoriteError" class="inline-error" role="alert">{{ library.favoriteError }}<button class="text-button" @click="library.loadLiked(true)">重试</button></div>
          <div v-if="library.likedState === 'loading' && !library.likedTracks.length" class="empty-state" role="status"><LoaderCircle :size="24" class="spinning"/><p>正在读取喜欢的歌曲</p></div>
          <template v-if="likedTracks.length"><div class="song-list-actions"><button class="button primary" @click="emit('play', likedTracks[0]!, likedTracks)"><Play :size="14" fill="currentColor"/>播放全部</button><button class="button secondary" @click="emit('shuffle', likedTracks)"><Shuffle :size="15"/>随机播放</button><small>{{ likedTracks.length }} 首歌曲</small><button v-if="!searching" class="icon-button favorite-sort" aria-label="反转歌曲顺序" :aria-pressed="reverse" @click="reverse = !reverse"><SlidersHorizontal :size="16"/></button></div><TrackList :tracks="likedTracks" :current-mid="player.current?.mid" :playing="player.playing" @play="track => emit('play', track, likedTracks)" @artist="(name, track) => emit('trackArtist', name, track)" @album="track => emit('trackAlbum', track)" @play-next="track => emit('playNext', track)" @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', target)"/></template>
          <div v-else-if="!searching && library.likedState === 'ready'" class="empty-state"><Heart :size="30"/><h3>还没有喜欢的歌曲</h3></div>
        </section>
        <section v-else-if="category === 'recent'" class="recent-tab" aria-label="最近播放记录">
          <div v-if="library.recentError" class="inline-error" role="alert">
            <span>{{ library.recentError }}</span><button class="text-button" @click="library.loadRecent">重试</button>
          </div>
          <div v-if="library.recentState === 'loading' && !library.recentTracks.length" class="empty-state" role="status">
            <LoaderCircle :size="24" class="spinning"/><p>正在读取最近播放</p>
          </div>
          <template v-if="library.recentTracks.length">
            <div class="track-tools recent-tools">
              <button class="button primary" :disabled="!recentTracks.length" @click="playRecent"><Play :size="14" fill="currentColor"/>播放全部</button>
              <label class="track-filter"><Search :size="15"/><input v-model="recentQuery" aria-label="查找播放记录" placeholder="查找播放记录"/></label>
              <select v-model="recentSort" aria-label="播放记录排序">
                <option value="default">最近播放顺序</option><option value="title">按歌名</option><option value="artist">按歌手</option>
                <option value="album">按专辑</option><option value="duration">按时长</option>
              </select>
              <button v-if="recentSort !== 'default'" class="text-button" :aria-label="recentDescending ? '切换升序' : '切换降序'"
                @click="recentDescending = !recentDescending">{{ recentDescending ? '降序' : '升序' }}</button>
              <span class="filter-count">{{ recentTracks.length }} 首</span>
            </div>
            <TrackList v-if="recentTracks.length" :tracks="recentTracks" :current-mid="player.current?.mid" :playing="player.playing"
              @play="track => emit('play', track, recentTracks)" @artist="(name, track) => emit('trackArtist', name, track)"
              @album="track => emit('trackAlbum', track)" @play-next="track => emit('playNext', track)"
              @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', target)"/>
            <p v-else class="empty-state">没有匹配的播放记录</p>
          </template>
          <div v-else-if="library.recentState === 'ready'" class="empty-state"><Clock3 :size="30"/><h3>还没有播放记录</h3></div>
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
