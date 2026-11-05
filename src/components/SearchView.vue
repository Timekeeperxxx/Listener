<script setup lang="ts">
import { computed, watch } from 'vue'
import { Clock3, Search } from '@lucide/vue'
import { useSearchStore, type SearchKind } from '../stores/search'
import type { Playlist, Track } from '../types'
import CoverArt from './CoverArt.vue'
import TrackList from './TrackList.vue'
import AlbumGrid from './AlbumGrid.vue'
import CatalogPager from './CatalogPager.vue'
const props = defineProps<{ keyword: string; currentMid?: string; playing: boolean }>()
const emit = defineEmits<{ query: [keyword: string]; artist: [mid: string]; album: [mid: string]; playlist: [playlist: Playlist]; trackArtist: [name: string, track: Track]; trackAlbum: [track: Track]; play: [track: Track, items: Track[]]; playNext: [track: Track]; enqueue: [track: Track]; actions: [track: Track] }>()
const search = useSearchStore()
const tabs: { id: SearchKind; name: string }[] = [{ id: 'songs', name: '歌曲' }, { id: 'artists', name: '歌手' }, { id: 'albums', name: '专辑' }, { id: 'playlists', name: '歌单' }]
const loaded = computed(() => search.results[search.kind].length)
watch(() => props.keyword, keyword => { void search.search(keyword); if (!keyword) void search.loadHistory() }, { immediate: true })
function select(kind: SearchKind) { void search.search(props.keyword, kind) }
</script>
<template>
  <section class="search-view">
    <div class="page-heading"><h1>{{ keyword ? `“${keyword}”` : '搜索' }}</h1></div>
    <template v-if="!keyword">
      <div class="section-heading"><h2>最近搜索</h2><button v-if="search.history.length" class="text-button" @click="search.clearHistory">清空</button></div>
      <p v-if="search.historyError" class="inline-error" role="alert">{{ search.historyError }}</p>
      <div v-if="search.history.length" class="search-history"><button v-for="term in search.history" :key="term" @click="emit('query', term)"><Clock3 :size="15" />{{ term }}</button></div>
      <div v-else class="empty-state"><Search :size="32" /><h3>搜索歌曲、歌手、专辑或歌单</h3></div>
    </template>
    <template v-else>
      <nav class="collection-tabs" aria-label="搜索分类"><button v-for="tab in tabs" :key="tab.id" :class="{ selected: search.kind === tab.id }" @click="select(tab.id)">{{ tab.name }}</button></nav>
      <p v-if="search.historyError" class="inline-error" role="status">{{ search.historyError }}</p>
      <TrackList v-if="search.kind === 'songs'" :tracks="search.results.songs" :current-mid="currentMid" :playing="playing" @play="track => emit('play', track, search.results.songs)" @artist="(name, track) => emit('trackArtist', name, track)" @album="track => emit('trackAlbum', track)" @play-next="track => emit('playNext', track)" @enqueue="track => emit('enqueue', track)" @actions="track => emit('actions', track)" />
      <div v-else-if="search.kind === 'artists'" class="artist-grid"><button v-for="artist in search.results.artists" :key="artist.mid" class="artist-card" :aria-label="`查看歌手 ${artist.name}`" @click="emit('artist', artist.mid)"><CoverArt :src="artist.image" :alt="artist.name" /><strong>{{ artist.name }}</strong></button></div>
      <AlbumGrid v-else-if="search.kind === 'albums'" :albums="search.results.albums" @artist="mid => emit('artist', mid)" @album="mid => emit('album', mid)" />
      <div v-else class="playlist-grid"><button v-for="playlist in search.results.playlists" :key="playlist.id" class="playlist-card" @click="emit('playlist', playlist)"><div class="playlist-art"><CoverArt :src="playlist.cover" :alt="playlist.name" /></div><strong>{{ playlist.name }}</strong><span>{{ playlist.creator }}<span v-if="playlist.creator"> · </span>{{ playlist.count }} 首歌曲</span></button></div>
      <CatalogPager :state="search.state" :error="search.error" :has-more="search.results.hasMore" :loaded="loaded" :total="search.results.total" @load="search.search(keyword, search.kind, loaded > 0)" />
      <div v-if="search.state === 'ready' && !loaded" class="empty-state"><Search :size="32" /><h3>没有找到匹配的内容</h3></div>
    </template>
  </section>
</template>
