<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { LyricLineMouseEvent } from '@applemusic-like-lyrics/core'
import '@applemusic-like-lyrics/core/style.css'
import { AudioLines, RefreshCw, Music2 } from '@lucide/vue'
import { api } from '../lib/api'
import { usePlayerStore } from '../stores/player'
import CoverArt from './CoverArt.vue'
import SyncedLyrics from './SyncedLyrics.vue'
import type { CatalogAlbum, Track } from '../types'
import { artistNames } from '../lib/catalog'

const player = usePlayerStore()
const syncedLyrics = ref<InstanceType<typeof SyncedLyrics>>()
const props = defineProps<{ showTranslation: boolean; showRomanization: boolean }>()
const emit = defineEmits<{ artist: [name: string, track: Track]; album: [mid: string] }>()
const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
const reducedMotion = ref(motionQuery.matches)
const updateMotion = () => { reducedMotion.value = motionQuery.matches }
onMounted(() => motionQuery.addEventListener('change', updateMotion))
onUnmounted(() => { albumRequest++; motionQuery.removeEventListener('change', updateMotion) })
const animationEnabled = computed(() => !!player.current && player.settings.animatedBackground && !reducedMotion.value)
const album = ref<CatalogAlbum | null>(null)
const albumCache = new Map<string, CatalogAlbum>()
let albumRequest = 0
function releaseYear(value: string): string {
  const text = value.trim()
  if (!text) return ''
  const date = text.match(/^(\d{4})/)
  if (date) return date[1]
  if (/^\d{10,13}$/.test(text)) {
    const stamp = Number(text)
    const parsed = new Date(stamp >= 1e12 ? stamp : stamp * 1000)
    if (!Number.isNaN(parsed.getTime())) return String(parsed.getFullYear())
  }
  return ''
}
const albumName = computed(() => player.current?.album || album.value?.name || '')
const albumMid = computed(() => player.current?.albumMid || album.value?.mid || '')
const releaseLabel = computed(() => releaseYear(album.value?.publishDate ?? ''))
watch(() => player.current?.albumMid ?? '', async mid => {
  const request = ++albumRequest
  album.value = mid ? albumCache.get(mid) ?? null : null
  if (!mid || album.value) return
  try {
    const result = await api.albumMeta(mid)
    if (request === albumRequest && player.current?.albumMid === mid) {
      albumCache.set(mid, result)
      album.value = result
    }
  } catch {
    // The cover badge is optional; a missing album detail never replaces the track identity.
  }
}, { immediate: true })
const displayedLines = computed(() => player.lyricLines.map(line => ({
  ...line,
  translatedLyric: props.showTranslation ? line.translatedLyric : '',
  romanLyric: props.showRomanization ? line.romanLyric : '',
  words: props.showRomanization ? line.words : line.words.map(word => ({ ...word, romanWord: undefined })),
})))
function lineClick(event: LyricLineMouseEvent) {
  const line = event.line.getLine()
  const offset = player.settings.lyricOffset + player.trackOffset
  player.seek((line.startTime - offset) / 1000)
  syncedLyrics.value?.syncTo(line.startTime)
}
</script>

<template>
  <section class="lyrics-scene" :style="{ '--lyric-font-scale': player.settings.lyrics.fontScale / 100 }"
    :class="{ 'has-track': !!player.current, 'has-cover': !!player.current?.cover, 'ambient-motion': animationEnabled, 'ambient-visible': player.visualActive }"
    aria-label="正在播放与歌词">
    <div class="lyric-ambient" :style="player.current?.cover ? { backgroundImage: `url('${player.current.cover}')` } : {}"></div>
    <div class="lyric-shade"></div>
    <div v-if="player.current" class="now-playing-layout">
      <div class="now-playing-record">
        <div class="now-playing-cover">
          <CoverArt :src="player.current.cover" :alt="player.current.album" class="large-cover" />
          <span v-if="releaseLabel" class="cover-release" :title="`发行时间 ${album?.publishDate ?? ''}`">{{ releaseLabel }}</span>
        </div>
        <div class="now-playing-meta"><h1 :title="player.current.title">{{ player.current.title }}</h1><p><template v-for="(name, index) in artistNames(player.current)" :key="name"><span v-if="index"> / </span><button class="metadata-link" @click="$emit('artist', name, player.current)">{{ name }}</button></template></p><p v-if="albumName" class="now-playing-album" :title="albumName"><button v-if="albumMid" class="metadata-link now-playing-album-name" @click="emit('album', albumMid)">{{ albumName }}</button><span v-else class="now-playing-album-name">{{ albumName }}</span></p></div>
        <div class="now-playing-controls"><slot name="controls"></slot></div>
      </div>
      <div class="lyric-pane">
        <SyncedLyrics v-if="player.lyricLines.length" ref="syncedLyrics" :key="player.current.mid" class="amll-player"
          :lines="displayedLines" :playing="player.playing" :active="player.visualActive" :font-scale="player.settings.lyrics.fontScale" :line-height="player.settings.lyrics.lineHeight"
          :reduced-motion="reducedMotion" :read-time="player.readLyricTime" @line-click="lineClick" />
        <div v-else class="lyric-empty">
          <AudioLines v-if="player.lyricState === 'loading'" :size="32" class="pulse-soft" />
          <Music2 v-else :size="32" :stroke-width="1.2" />
          <h2>{{ player.lyricState === 'loading' ? '正在加载歌词' : player.lyricState === 'error' ? '歌词加载失败' : '暂无同步歌词' }}</h2>
          <p v-if="player.lyricError">{{ player.lyricError }}</p>
          <button v-if="player.lyricState === 'error'" class="button ghost" @click="player.reloadLyrics"><RefreshCw :size="14" />重新加载</button>
        </div>
        <div v-if="player.lyricWarnings.length" class="lyric-footnote" role="status">{{ player.lyricWarnings[0] }}</div>
      </div>
    </div>
    <div v-else class="nothing-playing">
      <div class="empty-record"><AudioLines :size="56" :stroke-width="1" /></div>
      <h1>未在播放</h1>
    </div>
  </section>
</template>
