<script setup lang="ts">
import { ChevronRight } from '@lucide/vue'
import type { CatalogAlbum } from '../types'
import CoverArt from './CoverArt.vue'
defineProps<{ albums: CatalogAlbum[] }>()
defineEmits<{ album: [mid: string]; artist: [mid: string] }>()
</script>

<template>
  <div class="catalog-album-grid">
    <article v-for="item in albums" :key="item.mid" class="album-card">
      <button class="album-art-button" :aria-label="`查看专辑 ${item.name}`" @click="$emit('album', item.mid)"><CoverArt :src="item.cover" :alt="item.name" /><span class="card-arrow"><ChevronRight :size="21" /></span></button>
      <button class="album-name" :title="item.name" @click="$emit('album', item.mid)">{{ item.name }}</button>
      <div class="album-credit"><template v-for="(artist, i) in item.artists" :key="`${artist.mid}-${i}`"><span v-if="i"> / </span><button v-if="artist.mid" class="metadata-link" @click="$emit('artist', artist.mid)">{{ artist.name }}</button><span v-else>{{ artist.name }}</span></template><span v-if="!item.artists.length">{{ item.credit }}</span><span v-if="item.publishDate"> · {{ item.publishDate.slice(0, 4) }}</span></div>
    </article>
  </div>
</template>
