<script setup lang="ts">
import { ArrowUpRight, Heart } from '@lucide/vue'
import type { Track } from '../../src/types'
defineProps<{ playlists: { id: string; name: string; image: string; kind: string; tracks: Track[] }[]; saved: Set<string> }>()
const emit = defineEmits<{ open: [id: string]; save: [id: string] }>()
</script>

<template>
  <div class="record-grid playlist-grid">
    <article v-for="(playlist, index) in playlists" :key="playlist.id" class="record-card">
      <button class="record-sleeve playlist-sleeve" :aria-label="`查看歌单 ${playlist.name}`" @click="emit('open', playlist.id)">
        <img :src="playlist.image" :alt="playlist.name"/><span class="sleeve-play"><ArrowUpRight :size="20"/></span>
      </button>
      <div class="record-caption">
        <span class="record-number">{{ String(index + 1).padStart(2, '0') }}</span>
        <div><button @click="emit('open', playlist.id)"><strong>{{ playlist.name }}</strong></button><small>{{ playlist.tracks.length }} 首歌曲<span v-if="playlist.kind === 'created'"> · 我创建的</span></small></div>
        <button v-if="playlist.kind === 'catalog'" class="record-more icon-button" :class="{ loved: saved.has(playlist.id) }" :aria-label="`${saved.has(playlist.id) ? '取消收藏' : '收藏'}歌单 ${playlist.name}`" @click="emit('save', playlist.id)"><Heart :size="15" :fill="saved.has(playlist.id) ? 'currentColor' : 'none'"/></button>
      </div>
    </article>
  </div>
</template>
