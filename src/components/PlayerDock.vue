<script setup lang="ts">
import { AudioLines, ListMusic, LoaderCircle, Pause, Play, Repeat, Repeat1, Shuffle, SkipBack, SkipForward, Volume2, VolumeX } from '@lucide/vue'
import { usePlayerStore } from '../stores/player'
import { formatTime } from '../lib/format'
import CoverArt from './CoverArt.vue'
import FavoriteButton from './FavoriteButton.vue'
defineProps<{ lyrics?: boolean }>()
const emit = defineEmits<{ lyrics: []; queue: [] }>()
const player = usePlayerStore()
function setVolume(event: Event) { player.updateSettings({ volume: Number((event.target as HTMLInputElement).value) }) }
function seek(event: Event) { player.seek(Number((event.target as HTMLInputElement).value)) }
</script>

<template>
      <footer class="player-dock" :class="{ 'dock-idle': !player.current }" aria-label="播放控制">
        <div class="dock-track-group"><button class="dock-track" :disabled="!player.current" title="打开歌词页" @click="emit('lyrics')"><CoverArt :src="player.current?.cover" /><span><strong>{{ player.current?.title || '未在播放' }}</strong><small>{{ player.current?.artist || '选择一首歌曲' }}</small></span></button><FavoriteButton :track="player.current" class="dock-favorite" /></div>
        <div class="dock-center"><div class="playback-buttons"><button class="icon-button small" :class="{ active: player.shuffle }" :disabled="!player.current" :aria-pressed="player.shuffle" aria-label="随机播放" title="随机播放" @click="player.toggleShuffle"><Shuffle :size="15" /></button><button class="icon-button" :disabled="!player.current" aria-label="上一首" title="上一首" @click="player.previous"><SkipBack :size="20" fill="currentColor" /></button><button class="play-toggle" :disabled="!player.current || player.sourceState === 'loading'" :aria-label="player.playing ? '暂停' : '播放'" :title="player.playing ? '暂停（空格）' : player.restored ? '继续上次播放（空格）' : '播放（空格）'" @click="player.toggle"><LoaderCircle v-if="player.buffering" :size="22" class="spinning" /><Pause v-else-if="player.playing" :size="20" fill="currentColor" /><Play v-else :size="20" fill="currentColor" /></button><button class="icon-button" :disabled="!player.current" aria-label="下一首" title="下一首" @click="player.next()"><SkipForward :size="20" fill="currentColor" /></button><button class="icon-button small" :class="{ active: player.repeat !== 'off' }" :disabled="!player.current" :aria-label="`循环模式：${player.repeat === 'off' ? '顺序播放' : player.repeat === 'all' ? '列表循环' : '单曲循环'}`" title="切换循环模式" @click="player.cycleRepeat"><Repeat1 v-if="player.repeat === 'one'" :size="15" /><Repeat v-else :size="15" /></button></div><div class="timeline"><time>{{ formatTime(player.time / 1000) }}</time><input type="range" min="0" :max="player.duration || 1" step="0.1" :value="player.time / 1000" :disabled="!player.current || player.sourceState !== 'ready'" :style="{ '--progress': `${player.progress}%` }" aria-label="播放进度" :aria-valuetext="formatTime(player.time / 1000)" @input="seek" /><time>{{ formatTime(player.duration) }}</time></div></div>
        <div class="dock-tools"><FavoriteButton v-if="lyrics" :track="player.current" class="dock-favorite" /><div class="volume-control"><button class="icon-button small" :aria-label="player.settings.volume === 0 ? '恢复音量' : '静音'" @click="player.updateSettings({ volume: player.settings.volume === 0 ? 0.7 : 0 })"><VolumeX v-if="player.settings.volume === 0" :size="17" /><Volume2 v-else :size="17" /></button><input type="range" min="0" max="1" step="0.01" :value="player.settings.volume" :style="{ '--progress': `${player.settings.volume * 100}%` }" aria-label="音量" @input="setVolume" /></div><div class="dock-divider"></div><button class="icon-button lyric-toggle" :class="{ active: lyrics }" aria-label="切换歌词页" title="歌词（L）" @click="emit('lyrics')"><AudioLines :size="18" /></button><button class="icon-button" aria-label="播放队列" title="待播列表" @click="emit('queue')"><ListMusic :size="19" /></button></div>
        <span v-if="player.mixMode" class="dock-mix-status" role="status">{{ player.mixMode }}中</span>
      </footer>
</template>
