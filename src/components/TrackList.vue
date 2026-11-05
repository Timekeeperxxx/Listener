<script setup lang="ts">
import { AudioLines, ListEnd, ListStart, MoreHorizontal, Play } from '@lucide/vue'
import CoverArt from './CoverArt.vue'
import FavoriteButton from './FavoriteButton.vue'
import { formatTime } from '../lib/format'
import type { Track } from '../types'
import { artistNames } from '../lib/catalog'
defineProps<{ tracks: Track[]; currentMid?: string; playing: boolean; compact?: boolean }>()
defineEmits<{ play: [track: Track]; playNext: [track: Track]; enqueue: [track: Track]; actions: [track: Track]; artist: [name: string, track: Track]; album: [track: Track] }>()
</script>

<template>
  <div class="track-table" :class="{ 'compact-tracks': compact }" role="table" aria-label="歌曲列表">
    <div class="track-table-head track-grid" role="row">
      <span role="columnheader">#</span><span role="columnheader">歌曲</span><span role="columnheader" class="favorite-heading">喜欢</span><span role="columnheader" class="track-album-heading">专辑</span><span role="columnheader" class="track-duration">时长</span><span role="columnheader" class="track-actions-heading">操作</span>
    </div>
    <div v-for="(track, index) in tracks" :key="track.mid" class="track-row track-grid" :class="{ 'track-current': track.mid === currentMid }"
      role="row" @click="$emit('play', track)">
      <span class="track-number" role="cell"><AudioLines v-if="track.mid === currentMid && playing" :size="15" class="equalizer-icon" /><span v-else class="row-number">{{ String(index + 1).padStart(2, '0') }}</span><Play class="row-play" :size="13" fill="currentColor" /></span>
      <span role="cell" class="track-primary"><button class="track-cover-play" :aria-label="`播放 ${track.title}，${track.artist}`" @click.stop="$emit('play', track)"><CoverArt :src="track.cover" /></button><span class="track-names"><button class="track-title" @click.stop="$emit('play', track)">{{ track.title }} <span v-if="track.vip" class="vip-label">VIP</span></button><span class="track-artist"><template v-for="(name, i) in artistNames(track)" :key="name"><span v-if="i"> / </span><button class="metadata-link" @click.stop="$emit('artist', name, track)">{{ name }}</button></template></span></span></span>
      <span class="favorite-cell" role="cell"><FavoriteButton :track="track" /></span>
      <span class="track-album" role="cell"><button v-if="track.album" class="metadata-link" @click.stop="$emit('album', track)">{{ track.album }}</button><span v-else>—</span></span><span role="cell" class="track-duration">{{ formatTime(track.duration) }}</span>
      <span role="cell" class="track-actions" @click.stop><button class="icon-button small" :aria-label="`下一首播放 ${track.title}`" title="下一首播放" @click="$emit('playNext', track)"><ListStart :size="16" /></button><button class="icon-button small" :aria-label="`加入队列 ${track.title}`" title="加入播放队列" @click="$emit('enqueue', track)"><ListEnd :size="16" /></button><button class="icon-button small" :aria-label="`更多操作 ${track.title}`" title="添加到歌单与下载" @click="$emit('actions', track)"><MoreHorizontal :size="16" /></button></span>
    </div>
  </div>
</template>
