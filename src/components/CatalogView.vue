<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ChevronRight, Disc3, Heart, LoaderCircle, Play, Search, Shuffle, Users, X } from '@lucide/vue'
import { errorMessage } from '../lib/api'
import { useCatalogStore } from '../stores/catalog'
import { useSavedCatalogStore } from '../stores/saved-catalog'
import type { ArtistSongOrder, Track, TrackMenuTarget } from '../types'
import CoverArt from './CoverArt.vue'
import TrackList from './TrackList.vue'
import AlbumGrid from './AlbumGrid.vue'
import CatalogPager from './CatalogPager.vue'

const props = defineProps<{
  view: 'artists' | 'albums' | 'artist' | 'album'
  artistMid: string
  albumId: string
  currentMid?: string
  playing: boolean
}>()
const emit = defineEmits<{
  library: []
  navigate: [view: 'artists' | 'albums' | 'artist' | 'album', id?: string]
  artist: [name: string, track: Track]
  album: [track: Track]
  play: [track: Track, tracks: Track[]]
  shuffle: [tracks: Track[]]
  playNext: [track: Track]
  enqueue: [track: Track]
  actions: [target: TrackMenuTarget]
}>()
const catalog = useCatalogStore()
const saved = useSavedCatalogStore()
const scope = ref<'browse' | 'saved'>('browse')
const preparingArtist = ref(false)
const playbackError = ref('')
const filter = ref('')
const artistTab = ref<'overview' | 'songs' | 'albums' | 'about'>('overview')
const artistSongOrder = ref<ArtistSongOrder>('popular')
let artistPlaybackRequest = 0
const artist = computed(() => catalog.artist)
const album = computed(() => catalog.album)
const savedArtists = computed(() => saved.items.artists.filter(a => a.name.toLowerCase().includes(filter.value.trim().toLowerCase())))
const savedAlbums = computed(() => saved.items.albums.filter(a => `${a.name} ${a.credit}`.toLowerCase().includes(filter.value.trim().toLowerCase())))
const shownArtists = computed(() => scope.value === 'saved' ? savedArtists.value : catalog.artists.items)
const shownAlbums = computed(() => scope.value === 'saved' ? savedAlbums.value : catalog.albums.items)
const directory = computed(() => scope.value === 'saved' ? { items: props.view === 'artists' ? savedArtists.value : savedAlbums.value, state: saved.state, error: saved.error, hasMore: false, total: props.view === 'artists' ? savedArtists.value.length : savedAlbums.value.length } : props.view === 'artists' ? catalog.artists : catalog.albums)
const orderedSongs = computed(() => artistSongOrder.value === 'popular' ? catalog.songs : catalog.newestSongs)
const artistTracks = computed(() => artistTab.value === 'overview' ? orderedSongs.value.items.slice(0, 5) : orderedSongs.value.items)
const albums = computed(() => artistTab.value === 'overview' ? catalog.discography.items.slice(0, 5) : catalog.discography.items)
const albumMinutes = computed(() => album.value ? Math.round(album.value.tracks.reduce((sum, track) => sum + track.duration, 0) / 60) : 0)
const artistAreas = [{ id: -100, name: '全部地区' }, { id: 200, name: '内地' }, { id: 2, name: '港台' }, { id: 5, name: '欧美' }, { id: 4, name: '日本' }, { id: 3, name: '韩国' }, { id: 6, name: '其他' }]
const albumAreas = [{ id: 1, name: '内地' }, { id: 2, name: '港台' }, { id: 3, name: '欧美' }, { id: 4, name: '韩国' }, { id: 5, name: '日本' }, { id: 6, name: '其他' }]
watch(() => [props.view, props.artistMid, props.albumId], () => {
  artistPlaybackRequest++
  preparingArtist.value = false
  playbackError.value = ''
  artistTab.value = 'overview'
  artistSongOrder.value = 'popular'
  if (props.view === 'artists') {
    filter.value = catalog.artistQuery
    if (catalog.artists.state === 'idle') void catalog.loadArtists()
  } else if (props.view === 'albums') {
    filter.value = catalog.albumQuery
    if (catalog.albums.state === 'idle') void catalog.loadAlbums()
  } else if (props.view === 'artist') void catalog.loadArtist(props.artistMid)
  else void catalog.loadAlbum(props.albumId)
}, { immediate: true })
watch(artistSongOrder, order => {
  playbackError.value = ''
  if (orderedSongs.value.state === 'idle') void catalog.loadArtistSongs(order)
})
onBeforeUnmount(() => { artistPlaybackRequest++ })
async function playArtist(shuffle = false) {
  const request = ++artistPlaybackRequest
  preparingArtist.value = true; playbackError.value = ''
  try {
    const tracks = await catalog.completeArtistSongs(artistSongOrder.value)
    if (request !== artistPlaybackRequest || !tracks.length) return
    if (shuffle) emit('shuffle', tracks)
    else emit('play', tracks[0]!, tracks)
  } catch (cause) {
    if (request === artistPlaybackRequest) playbackError.value = errorMessage(cause)
  } finally {
    if (request === artistPlaybackRequest) preparingArtist.value = false
  }
}
function search() {
  if (scope.value === 'saved') return
  if (props.view === 'artists') { catalog.artistQuery = filter.value.trim(); void catalog.loadArtists() }
  else { catalog.albumQuery = filter.value.trim(); void catalog.loadAlbums() }
}
function clearSearch() { filter.value = ''; search() }
function loadDirectory() { if (scope.value === 'saved') { void saved.load(); return }; if (props.view === 'artists') void catalog.loadArtists(!!catalog.artists.items.length); else void catalog.loadAlbums(!!catalog.albums.items.length) }
function changeArea(event: Event) {
  const value = Number((event.target as HTMLSelectElement).value)
  if (props.view === 'artists') { catalog.artistArea = value; void catalog.loadArtists() }
  else { catalog.albumArea = value; void catalog.loadAlbums() }
}
</script>

<template>
  <section class="catalog-view">
    <template v-if="view === 'artists' || view === 'albums'">
      <div class="page-heading catalog-heading"><div><span class="content-kind">QQ 音乐</span><h1>{{ view === 'artists' ? '歌手' : '专辑' }}</h1></div><form class="catalog-search" role="search" @submit.prevent="search"><button type="submit" aria-label="搜索"><Search :size="16" /></button><input v-model="filter" maxlength="100" :placeholder="view === 'artists' ? '搜索所有歌手' : '搜索专辑或歌手'" :aria-label="view === 'artists' ? '搜索所有歌手' : '搜索所有专辑'" /><button v-if="filter" type="button" aria-label="清除搜索" @click="clearSearch"><X :size="15" /></button></form></div>
      <nav class="collection-tabs" aria-label="音乐库分类"><button @click="emit('library')">歌单</button><button :class="{ selected: view === 'albums' }" @click="emit('navigate', 'albums')">专辑</button><button :class="{ selected: view === 'artists' }" @click="emit('navigate', 'artists')">歌手</button></nav>
      <div class="catalog-scope" role="group" aria-label="目录范围"><button :class="{ selected: scope === 'browse' }" @click="scope = 'browse'; filter = view === 'artists' ? catalog.artistQuery : catalog.albumQuery">浏览乐库</button><button :class="{ selected: scope === 'saved' }" @click="scope = 'saved'; filter = ''">我的收藏</button></div>
      <div class="catalog-directory-heading"><h2>{{ scope === 'saved' ? '我的收藏' : '' }}<template v-if="scope !== 'saved'">{{ (view === 'artists' ? catalog.artistQuery : catalog.albumQuery) ? '搜索结果' : view === 'artists' ? '热门歌手' : '新碟上架' }}</template></h2><select v-if="scope === 'browse' && !(view === 'artists' ? catalog.artistQuery : catalog.albumQuery)" :value="view === 'artists' ? catalog.artistArea : catalog.albumArea" aria-label="地区" @change="changeArea"><option v-for="area in view === 'artists' ? artistAreas : albumAreas" :key="area.id" :value="area.id">{{ area.name }}</option></select></div>
      <div v-if="view === 'artists'" class="artist-grid"><button v-for="item in shownArtists" :key="item.mid" class="artist-card" @click="emit('navigate', 'artist', item.mid)"><CoverArt :src="item.image" :alt="item.name" /><strong>{{ item.name }}</strong></button></div>
      <AlbumGrid v-else :albums="shownAlbums" @album="mid => emit('navigate', 'album', mid)" @artist="mid => emit('navigate', 'artist', mid)" />
      <CatalogPager :state="directory.state" :error="directory.error" :has-more="directory.hasMore" :loaded="directory.items.length" :total="directory.total" @load="loadDirectory" />
      <div v-if="directory.state === 'ready' && !directory.items.length" class="empty-state"><Users v-if="view === 'artists'" :size="32" /><Disc3 v-else :size="32" /><h3>没有找到匹配的内容</h3></div>
    </template>

    <template v-else-if="view === 'artist' && artist && catalog.artistState === 'ready'">
      <nav class="catalog-breadcrumb artist-breadcrumb" aria-label="当前位置"><button @click="emit('navigate', 'artists')">歌手</button><ChevronRight :size="13" /><span>{{ artist.artist.name }}</span></nav>
      <nav class="artist-tabs" aria-label="歌手内容">
        <button :class="{ selected: artistTab === 'overview' }" :aria-pressed="artistTab === 'overview'" @click="artistTab = 'overview'">概览</button>
        <button :class="{ selected: artistTab === 'songs' }" :aria-pressed="artistTab === 'songs'" @click="artistTab = 'songs'">歌曲 <span>{{ catalog.songs.total }}</span></button>
        <button :class="{ selected: artistTab === 'albums' }" :aria-pressed="artistTab === 'albums'" @click="artistTab = 'albums'">专辑 <span>{{ catalog.discography.total }}</span></button>
        <button :class="{ selected: artistTab === 'about' }" :aria-pressed="artistTab === 'about'" @click="artistTab = 'about'">简介</button>
      </nav>

      <div class="artist-detail" :class="{ 'artist-overview': artistTab === 'overview' }">
        <header class="artist-hero" aria-labelledby="artist-name">
          <div class="artist-portrait-frame">
            <CoverArt :src="artist.artist.image" :alt="artist.artist.name" class="artist-portrait" />
            <div class="artist-portrait-caption">
              <h1 id="artist-name">{{ artist.artist.name }}</h1>
              <p class="artist-subtitle">{{ catalog.songs.total }} 首歌曲 · {{ catalog.discography.total }} 张专辑</p>
            </div>
          </div>
          <div class="catalog-play-actions">
            <button class="button primary" :disabled="!orderedSongs.items.length || orderedSongs.state === 'loading' || preparingArtist" @click="playArtist()">
              <LoaderCircle v-if="preparingArtist" :size="15" class="spinning" />
              <Play v-else :size="15" fill="currentColor" />{{ preparingArtist ? `${orderedSongs.items.length}/${orderedSongs.total}` : '播放' }}
            </button>
            <button class="button secondary" :disabled="!orderedSongs.items.length || orderedSongs.state === 'loading' || preparingArtist" @click="playArtist(true)"><Shuffle :size="16" />随机播放</button>
            <button class="button secondary catalog-save" :class="{ selected: saved.hasArtist(artist.artist.mid) }"
              :disabled="saved.state !== 'ready' || !!saved.busy" :aria-pressed="saved.hasArtist(artist.artist.mid)" @click="saved.toggleArtist(artist.artist)">
              <Heart :size="16" :fill="saved.hasArtist(artist.artist.mid) ? 'currentColor' : 'none'" />
              {{ saved.hasArtist(artist.artist.mid) ? '已关注' : '关注歌手' }}
            </button>
          </div>
          <p v-if="playbackError" class="inline-error artist-playback-error" role="alert">{{ playbackError }}</p>
        </header>

        <section v-if="artistTab === 'overview' || artistTab === 'songs'" class="catalog-song-section" aria-labelledby="artist-songs-heading">
          <div class="section-heading">
            <h2 id="artist-songs-heading">{{ artistTab === 'songs' ? '全部歌曲' : artistSongOrder === 'popular' ? '热门歌曲' : '最新歌曲' }}</h2>
            <div class="artist-song-tools">
              <select v-if="catalog.songs.total" v-model="artistSongOrder" aria-label="歌手歌曲排序" :disabled="preparingArtist">
                <option value="popular">按热度</option>
                <option value="newest">按发行时间</option>
              </select>
              <button v-if="artistTab === 'overview' && artistTracks.length" class="text-button" aria-label="查看全部歌曲" @click="artistTab = 'songs'">查看全部<ChevronRight :size="14" /></button>
            </div>
          </div>
          <TrackList v-if="artistTracks.length" :tracks="artistTracks" :current-mid="currentMid" :playing="playing" :compact="artistTab === 'overview'" :cover-playback="artistTab === 'overview'"
            @play="track => emit('play', track, orderedSongs.items)" @artist="(name, track) => emit('artist', name, track)" @album="track => emit('album', track)"
            @play-next="track => emit('playNext', track)" @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', { ...target, tracks: orderedSongs.items, sequential: true })" />
          <p v-else-if="orderedSongs.state === 'ready'" class="muted">QQ 音乐暂未提供歌曲。</p>
          <CatalogPager v-if="artistTab === 'songs' || orderedSongs.state === 'error' || (orderedSongs.state === 'loading' && !orderedSongs.items.length)"
            :state="orderedSongs.state" :error="orderedSongs.error" :has-more="orderedSongs.hasMore" :loaded="orderedSongs.items.length" :total="orderedSongs.total" @load="catalog.loadArtistSongs(artistSongOrder)" />
        </section>

        <section v-if="artistTab === 'overview' || artistTab === 'about'" class="artist-about" aria-labelledby="artist-about-heading">
          <div class="section-heading"><h2 id="artist-about-heading">{{ artistTab === 'overview' ? '简介' : `关于 ${artist.artist.name}` }}</h2></div>
          <p :class="{ 'description-preview': artistTab === 'overview' }">{{ artist.description || 'QQ 音乐暂未提供歌手介绍。' }}</p>
          <button v-if="artistTab === 'overview' && artist.description" class="text-button" @click="artistTab = 'about'">完整简介<ChevronRight :size="14" /></button>
        </section>

        <section v-if="artistTab === 'overview' || artistTab === 'albums'" class="catalog-discography" aria-labelledby="artist-albums-heading">
          <div class="section-heading">
            <h2 id="artist-albums-heading">{{ artistTab === 'overview' ? '专辑' : '全部专辑' }}</h2>
            <button v-if="artistTab === 'overview' && albums.length" class="text-button" aria-label="查看全部专辑" @click="artistTab = 'albums'">查看全部<ChevronRight :size="14" /></button>
          </div>
          <AlbumGrid v-if="albums.length" :albums="albums" @album="mid => emit('navigate', 'album', mid)" @artist="mid => emit('navigate', 'artist', mid)" />
          <p v-else-if="catalog.discography.state === 'ready'" class="muted">QQ 音乐暂未提供专辑。</p>
          <CatalogPager v-if="artistTab === 'albums'" :state="catalog.discography.state" :error="catalog.discography.error" :has-more="catalog.discography.hasMore" :loaded="catalog.discography.items.length" :total="catalog.discography.total" @load="catalog.loadArtistAlbums" />
        </section>
      </div>
    </template>

    <template v-else-if="view === 'album' && album && catalog.albumState === 'ready'">
      <nav class="catalog-breadcrumb" aria-label="当前位置"><button @click="emit('navigate', 'albums')">专辑</button><ChevronRight :size="13" /><span>{{ album.album.name }}</span></nav>
      <div class="album-hero"><CoverArt :src="album.album.cover" :alt="album.album.name" class="album-hero-art" /><div class="album-hero-copy"><span class="content-kind">{{ album.album.albumType || '专辑' }}</span><h1>{{ album.album.name }}</h1><div class="album-artist-link"><template v-for="(item, index) in album.album.artists" :key="item.mid"><span v-if="index"> / </span><button v-if="item.mid" @click="emit('navigate', 'artist', item.mid)">{{ item.name }}</button><span v-else>{{ item.name }}</span></template><ChevronRight :size="16" /></div><p>{{ [album.genre, album.language, `${album.tracks.length} 首歌曲`, `${albumMinutes} 分钟`].filter(Boolean).join(' · ') }}</p><div class="catalog-play-actions"><button class="button primary" :disabled="!album.tracks.length" @click="emit('play', album.tracks[0]!, album.tracks)"><Play :size="15" fill="currentColor" />播放</button><button class="button secondary" :disabled="!album.tracks.length" @click="emit('shuffle', album.tracks)"><Shuffle :size="16" />随机播放</button><button class="button secondary catalog-save" :class="{ selected: saved.hasAlbum(album.album.mid) }" :disabled="saved.state !== 'ready' || !!saved.busy" :aria-pressed="saved.hasAlbum(album.album.mid)" @click="saved.toggleAlbum(album.album)"><Heart :size="16" :fill="saved.hasAlbum(album.album.mid) ? 'currentColor' : 'none'" />{{ saved.hasAlbum(album.album.mid) ? '已收藏' : '收藏专辑' }}</button></div></div></div>
      <TrackList :tracks="album.tracks" :current-mid="currentMid" :playing="playing" @play="track => emit('play', track, album!.tracks)" @artist="(name, track) => emit('artist', name, track)" @album="track => emit('album', track)" @play-next="track => emit('playNext', track)" @enqueue="track => emit('enqueue', track)" @actions="target => emit('actions', target)" />
      <div class="album-release"><span>{{ album.album.publishDate }}</span><span v-if="album.company"> · {{ album.company }}</span></div>
      <section class="album-about"><h2>专辑介绍</h2><p>{{ album.description || 'QQ 音乐暂未提供专辑介绍。' }}</p></section>
    </template>

    <div v-else class="catalog-detail-status" aria-live="polite"><template v-if="(view === 'artist' ? catalog.artistState : catalog.albumState) === 'loading'"><LoaderCircle :size="25" class="spinning" /></template><template v-else><p role="alert">{{ view === 'artist' ? catalog.artistError : catalog.albumError }}</p><button class="button secondary" @click="view === 'artist' ? catalog.loadArtist(artistMid, true) : catalog.loadAlbum(albumId, true)">重试</button></template></div>
  </section>
</template>
