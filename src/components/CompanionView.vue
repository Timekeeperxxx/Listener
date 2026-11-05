<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ArrowUpRight, AudioLines, LoaderCircle, Pause, Play, SkipBack, SkipForward, X } from '@lucide/vue'
import CoverArt from './CoverArt.vue'
import { errorMessage } from '../lib/api'
import { formatTime } from '../lib/format'
import type { CompanionState } from '../lib/companion'
const desktopLyrics = new URLSearchParams(location.search).get('view') === 'desktop-lyrics'
const state = ref<CompanionState | null>(null)
const error = ref('')
let unlisten: UnlistenFn | undefined
const progress = computed(() => state.value && state.value.duration > 0 ? state.value.time / state.value.duration / 10 : 0)
function wordProgress(start: number, end: number) {
  const time = state.value!.lyricTime
  return end > start ? Math.max(0, Math.min(100, (time - start) / (end - start) * 100)) : time >= end ? 100 : 0
}
async function action(value: string) { try { await invoke('companion_action', { action: value }) } catch (cause) { error.value = errorMessage(cause) } }
async function close() { try { await getCurrentWindow().close() } catch (cause) { error.value = errorMessage(cause) } }
function keydown(event: KeyboardEvent) { if (event.code === 'Space' && !(event.target as HTMLElement).closest('button')) { event.preventDefault(); void action('toggle') } }
onMounted(async () => {
  unlisten = await listen<CompanionState>('companion-state', event => { state.value = event.payload })
  document.addEventListener('keydown', keydown)
  await action('ready')
})
onUnmounted(() => { unlisten?.(); document.removeEventListener('keydown', keydown) })
</script>
<template>
  <div class="companion-shell" :class="{ 'desktop-lyrics': desktopLyrics }">
    <div class="companion-drag" data-tauri-drag-region><AudioLines :size="13" /><span data-tauri-drag-region>{{ desktopLyrics ? 'Listener 歌词' : 'Listener' }}</span><button class="icon-button small" aria-label="打开主窗口" @click="action('main')"><ArrowUpRight :size="14" /></button><button class="icon-button small" aria-label="关闭小窗口" @click="close"><X :size="14" /></button></div>
    <template v-if="desktopLyrics">
      <div class="desktop-lyric-line" data-tauri-drag-region><template v-if="state?.words.length"><span v-for="(word, index) in state.words" :key="index" :style="{ '--word-progress': `${wordProgress(word.startTime, word.endTime)}%` }">{{ word.word }}</span></template><span v-else class="desktop-lyric-placeholder">{{ state?.current ? `${state.current.title} · 暂无同步歌词` : '播放一首歌，歌词会在这里出现' }}</span></div>
      <p class="desktop-lyric-translation" data-tauri-drag-region>{{ state?.translation || state?.nextLine }}</p>
      <div class="desktop-lyric-controls"><button class="icon-button small" aria-label="上一首" @click="action('previous')"><SkipBack :size="15" /></button><button class="icon-button small" :aria-label="state?.playing ? '暂停' : '播放'" :disabled="!state?.current || state.loading" @click="action('toggle')"><Pause v-if="state?.playing" :size="15" /><Play v-else :size="15" /></button><button class="icon-button small" aria-label="下一首" @click="action('next')"><SkipForward :size="15" /></button></div>
    </template>
    <div v-else class="mini-content"><button class="mini-meta" aria-label="打开主窗口" @click="action('main')"><CoverArt :src="state?.current?.cover" /><span><strong>{{ state?.current?.title || '尚未选择歌曲' }}</strong><small>{{ state?.current?.artist || '在主窗口选择音乐' }}</small></span></button><div class="mini-controls"><button class="icon-button small" aria-label="上一首" :disabled="!state?.current" @click="action('previous')"><SkipBack :size="16" /></button><button class="play-toggle" :aria-label="state?.playing ? '暂停' : '播放'" :disabled="!state?.current || state.loading" @click="action('toggle')"><LoaderCircle v-if="state?.loading" :size="19" class="spinning" /><Pause v-else-if="state?.playing" :size="19" /><Play v-else :size="19" /></button><button class="icon-button small" aria-label="下一首" :disabled="!state?.current" @click="action('next')"><SkipForward :size="16" /></button></div><div class="mini-timeline"><time>{{ formatTime((state?.time || 0) / 1000) }}</time><div><i :style="{ width: `${progress}%` }"></i></div><time>{{ formatTime(state?.duration || 0) }}</time></div></div>
    <p v-if="error" class="companion-error" role="alert">{{ error }}</p>
  </div>
</template>
