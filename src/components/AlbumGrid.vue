<script setup lang="ts">
import { ArrowUpRight, Heart } from '@lucide/vue'
import type { CatalogAlbum } from '../types'
import CoverArt from './CoverArt.vue'
import { useSavedCatalogStore } from '../stores/saved-catalog'
const saved = useSavedCatalogStore()
defineProps<{ albums: CatalogAlbum[] }>()
defineEmits<{ album: [mid: string]; artist: [mid: string] }>()
</script>

<template>
  <div class="catalog-album-grid">
    <article v-for="(item, index) in albums" :key="item.mid" class="album-card">
      <button class="album-art-button" :aria-label="`查看专辑 ${item.name}`" @click="$emit('album', item.mid)"><CoverArt :src="item.cover" :alt="item.name" /><span class="card-arrow"><ArrowUpRight :size="19" /></span></button>
      <div class="album-caption"><span class="record-number">{{ String(index + 1).padStart(2, '0') }}</span><div class="album-caption-copy">      <button class="album-name" :title="item.name" @click="$emit('album', item.mid)">{{ item.name }}</button>
      <div class="album-credit"><template v-for="(artist, i) in item.artists" :key="`${artist.mid}-${i}`"><span v-if="i"> / </span><button v-if="artist.mid" class="metadata-link" @click="$emit('artist', artist.mid)">{{ artist.name }}</button><span v-else>{{ artist.name }}</span></template><span v-if="!item.artists.length">{{ item.credit }}</span><span v-if="item.publishDate"> · {{ item.publishDate.slice(0, 4) }}</span></div>
</div><button class="record-save icon-button" :class="{ active: saved.hasAlbum(item.mid) }" :disabled="saved.state !== 'ready' || !!saved.busy" :aria-label="`${saved.hasAlbum(item.mid) ? '取消收藏' : '收藏'}专辑 ${item.name}`" @click="saved.toggleAlbum(item)"><Heart :size="15" :fill="saved.hasAlbum(item.mid) ? 'currentColor' : 'none'"/></button></div>
    </article>
  </div>
</template>
