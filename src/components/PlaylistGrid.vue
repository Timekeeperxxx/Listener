<script setup lang="ts">
import { ArrowUpRight } from '@lucide/vue'
import CoverArt from './CoverArt.vue'
import type { Playlist } from '../types'
defineProps<{ playlists: Playlist[] }>()
defineEmits<{ open: [playlist: Playlist] }>()
</script>

<template>
  <div class="playlist-grid record-grid">
    <article v-for="(playlist, index) in playlists" :key="playlist.id" class="record-card">
      <button class="record-sleeve" :aria-label="`查看歌单 ${playlist.name}`" @click="$emit('open', playlist)"><CoverArt :src="playlist.cover" :alt="playlist.name" :liked="playlist.kind === 'liked'"/><span class="card-arrow"><ArrowUpRight :size="19"/></span></button>
      <div class="record-caption"><span class="record-number">{{ String(index + 1).padStart(2, '0') }}</span><div><button @click="$emit('open', playlist)"><strong>{{ playlist.name }}</strong></button><small>{{ playlist.count }} 首歌曲<span v-if="playlist.kind === 'created'"> · 我创建的</span><span v-else-if="playlist.creator"> · {{ playlist.creator }}</span></small></div></div>
    </article>
  </div>
</template>
