<script setup lang="ts">
import { ref } from 'vue'
import { AudioLines, ListEnd, ListStart, MoreHorizontal, Pause, Play } from '@lucide/vue'
import CoverArt from './CoverArt.vue'
import FavoriteButton from './FavoriteButton.vue'
import { usePlayerStore } from '../stores/player'
import { formatTime } from '../lib/format'
import type { Track, TrackMenuTarget } from '../types'
import { artistNames } from '../lib/catalog'
const props = defineProps<{ tracks: Track[]; currentMid?: string; playing: boolean; compact?: boolean }>()
const emit = defineEmits<{ play: [track: Track]; playNext: [track: Track]; enqueue: [track: Track]; actions: [target: TrackMenuTarget]; artist: [name: string, track: Track]; album: [track: Track] }>()
const player = usePlayerStore()
const selectedMid = ref('')
function openActions(track: Track, event: MouseEvent) { selectedMid.value = track.mid; emit('actions', { track, tracks: props.tracks, anchor: event.currentTarget as HTMLElement }) }
function toggle(track: Track) { selectedMid.value = track.mid; if (track.mid === props.currentMid) void player.toggle(); else emit('play', track) }
</script>

<template>
  <div class="track-table" :class="{ 'compact-tracks': compact }" role="grid" aria-label="歌曲列表">
    <div class="track-table-head track-grid" role="row"><span role="columnheader">#</span><span role="columnheader">歌曲</span><span role="columnheader" class="favorite-heading">喜欢</span><span role="columnheader" class="track-album-heading">专辑</span><span role="columnheader" class="track-duration">时长</span><span role="columnheader" class="track-actions-heading">操作</span></div>
    <div v-for="(track, index) in tracks" :key="track.mid" class="track-row track-grid" :class="{ 'track-current': track.mid === currentMid, selected: selectedMid === track.mid }" role="row" :aria-selected="selectedMid === track.mid" tabindex="0" @click="selectedMid = track.mid" @dblclick="emit('play', track)" @keydown.enter.self.prevent.stop="emit('play', track)" @keydown.space.self.prevent.stop="selectedMid = track.mid">
      <span role="gridcell"><button class="track-number track-index" :aria-label="`${track.mid === currentMid && playing ? '暂停' : '播放'} ${track.title}`" @click.stop="toggle(track)" @dblclick.stop><span class="track-normal-icon"><AudioLines v-if="track.mid === currentMid && playing" :size="15"/><span v-else>{{ String(index + 1).padStart(2, '0') }}</span></span><span class="track-hover-icon"><Pause v-if="track.mid === currentMid && playing" :size="14" fill="currentColor"/><Play v-else :size="14" fill="currentColor"/></span></button></span>
      <span role="gridcell" class="track-primary"><button class="track-cover-play" :aria-label="`选中 ${track.title}`" @click.stop="selectedMid = track.mid" @dblclick.stop="emit('play', track)"><CoverArt :src="track.cover"/></button><span class="track-names"><button class="track-title" @click.stop="selectedMid = track.mid" @dblclick.stop="emit('play', track)" @keydown.enter.prevent.stop="emit('play', track)">{{ track.title }} <span v-if="track.vip" class="vip-label">VIP</span></button><span class="track-artist" @dblclick.stop><template v-for="(name, i) in artistNames(track)" :key="name"><span v-if="i"> / </span><button class="metadata-link" @click.stop="emit('artist', name, track)">{{ name }}</button></template></span></span></span>
      <span class="favorite-cell" role="gridcell" @dblclick.stop><FavoriteButton :track="track"/></span>
      <span class="track-album" role="gridcell" @dblclick.stop><button v-if="track.album" class="metadata-link" @click.stop="emit('album', track)">{{ track.album }}</button><span v-else>—</span></span><span role="gridcell" class="track-duration">{{ formatTime(track.duration) }}</span>
      <span role="gridcell" class="track-actions" @click.stop @dblclick.stop><button class="icon-button small" :aria-label="`下一首播放 ${track.title}`" title="下一首播放" @click="emit('playNext', track)"><ListStart :size="16"/></button><button class="icon-button small" :aria-label="`加入队列 ${track.title}`" title="加入播放队列" @click="emit('enqueue', track)"><ListEnd :size="16"/></button><button class="icon-button small" :aria-label="`更多操作 ${track.title}`" title="更多操作" aria-haspopup="menu" aria-expanded="false" @click="openActions(track, $event)"><MoreHorizontal :size="16"/></button></span>
    </div>
  </div>
</template>
