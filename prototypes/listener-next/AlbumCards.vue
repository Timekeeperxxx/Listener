<script setup lang="ts">
import { ArrowUpRight, Heart } from '@lucide/vue'
defineProps<{ albums: { id: number; name: string; artist: string; image: string; year: number | string }[]; saved: Set<number> }>()
const emit = defineEmits<{ open: [id: number]; artist: [name: string]; save: [id: number] }>()
</script>

<template>
  <div class="record-grid">
    <article v-for="(album, index) in albums" :key="album.id" class="record-card">
      <button class="record-sleeve" :aria-label="`查看专辑 ${album.name}`" @click="emit('open', album.id)">
        <img :src="album.image" :alt="album.name"/>
        <span class="sleeve-play"><ArrowUpRight :size="20"/></span><span class="album-year">{{ album.year }}</span>
      </button>
      <div class="record-caption">
        <span class="record-number">{{ String(index + 1).padStart(2, '0') }}</span>
        <div><button @click="emit('open', album.id)"><strong>{{ album.name }}</strong></button><button class="artist-link" @click="emit('artist', album.artist)"><small>{{ album.artist }}</small></button></div>
        <button class="record-more icon-button" :class="{ loved: saved.has(album.id) }" :aria-label="`${saved.has(album.id) ? '取消收藏' : '收藏'} ${album.name}`" @click="emit('save', album.id)"><Heart :size="15" :fill="saved.has(album.id) ? 'currentColor' : 'none'"/></button>
      </div>
    </article>
  </div>
</template>
