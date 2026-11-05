<script setup lang="ts">
import { computed } from 'vue'
import { Heart, LoaderCircle, RefreshCw } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import type { Track } from '../types'
const props = defineProps<{ track: Track | null }>()
const library = useLibraryStore()
const known = computed(() => library.likedState === 'ready')
const liked = computed(() => props.track !== null && library.likedMids.has(props.track.mid))
const busy = computed(() => props.track !== null && library.likingMid === props.track.mid)
const syncing = computed(() => library.likedState === 'idle' || library.likedState === 'loading')
const label = computed(() => {
  if (!props.track) return '收藏当前歌曲'
  if (library.profile?.offline) return `离线时无法修改收藏 ${props.track.title}`
  if (library.likedState === 'error') return `重新读取收藏状态 ${props.track.title}`
  if (busy.value) return `正在更新收藏 ${props.track.title}`
  if (!known.value) return `正在读取收藏状态 ${props.track.title}`
  return `${liked.value ? '取消喜欢' : '加入我喜欢'} ${props.track.title}`
})
function activate() {
  if (library.likedState === 'error') void library.loadLiked(true)
  else if (props.track) void library.toggleLike(props.track)
}
</script>

<template>
  <button class="favorite-button" :class="{ liked: known && liked, busy }" :aria-label="label" :title="label"
    :aria-pressed="known ? liked : undefined" :aria-busy="busy || (syncing && !!track && !!library.profile)"
    :disabled="!track || !library.profile || library.profile.offline || library.mutationBusy || library.likingMid !== null || syncing"
    @click.stop="activate">
    <LoaderCircle v-if="busy || (syncing && track && library.profile && !library.profile.offline)" :size="18" class="spinning" />
    <RefreshCw v-else-if="library.likedState === 'error'" :size="18" />
    <Heart v-else :size="19" :fill="known && liked ? 'currentColor' : 'none'" :stroke-width="1.8" />
  </button>
</template>
