<script setup lang="ts">
import { ArrowUpRight } from '@lucide/vue'
import CoverArt from './CoverArt.vue'
import PlaylistFavoriteButton from './PlaylistFavoriteButton.vue'
import type { Playlist } from '../types'
defineProps<{ playlists: Playlist[]; compact?: boolean }>()
defineEmits<{ open: [playlist: Playlist] }>()
const playCount = new Intl.NumberFormat('zh-CN', { notation: 'compact', maximumFractionDigits: 1 })
</script>

<template>
  <div class="playlist-grid record-grid">
    <article v-for="(playlist, index) in playlists" :key="playlist.id" class="record-card">
      <div class="record-artwork">
        <button class="record-sleeve" :aria-label="`查看歌单 ${playlist.name}`" @click="$emit('open', playlist)">
          <CoverArt :src="playlist.cover" :liked="playlist.kind === 'liked'"/>
          <span class="card-arrow"><ArrowUpRight :size="19"/></span>
        </button>
        <PlaylistFavoriteButton :playlist="playlist" icon-only/>
      </div>
      <div class="record-caption">
        <span v-if="!compact" class="record-number">{{ String(index + 1).padStart(2, '0') }}</span>
        <div>
          <button :title="playlist.name" @click="$emit('open', playlist)"><strong>{{ playlist.name }}</strong></button>
          <small v-if="playlist.playCount !== undefined">{{ playCount.format(playlist.playCount) }} 次播放</small>
          <small v-else-if="!compact || playlist.count">{{ playlist.count }} 首歌曲<span v-if="playlist.kind === 'created'"> · 我创建的</span><span v-else-if="playlist.creator"> · {{ playlist.creator }}</span></small>
        </div>
      </div>
    </article>
  </div>
</template>
