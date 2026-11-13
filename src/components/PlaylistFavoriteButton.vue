<script setup lang="ts">
import { computed } from 'vue'
import { Heart } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import type { Playlist } from '../types'

const props = defineProps<{ playlist: Playlist; iconOnly?: boolean }>()
const library = useLibraryStore()
const collectible = computed(() => (props.playlist.kind === 'public' || props.playlist.kind === 'collected') &&
  !library.playlists.some(item => item.id === props.playlist.id && item.kind === 'created'))
const saved = computed(() => library.hasPlaylistFavorite(props.playlist.id))
const label = computed(() => saved.value ? '取消收藏歌单' : '收藏歌单')
const unavailable = computed(() => library.profile?.offline ? '联网后可收藏歌单' :
  library.libraryState !== 'ready' || library.warnings.length ? '收藏状态暂不可用' : '')
</script>

<template>
  <button v-if="collectible" class="playlist-favorite" :class="[iconOnly ? 'icon-button' : 'button secondary', { active: saved }]"
    :disabled="!!unavailable || library.mutationBusy" :aria-label="`${label} ${playlist.name}`" :aria-pressed="saved"
    :title="unavailable || label" @click="library.togglePlaylistFavorite(playlist)">
    <Heart :size="16" :fill="saved ? 'currentColor' : 'none'"/>
    <span v-if="!iconOnly">{{ saved ? '已收藏' : '收藏' }}</span>
  </button>
</template>
