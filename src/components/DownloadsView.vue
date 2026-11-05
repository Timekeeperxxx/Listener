<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Download, LoaderCircle, Play, RefreshCw, Trash2, X } from '@lucide/vue'
import CoverArt from './CoverArt.vue'
import FavoriteButton from './FavoriteButton.vue'
import { useDownloadsStore } from '../stores/downloads'
import { usePlayerStore } from '../stores/player'
import type { Download as DownloadRecord } from '../types'
const downloads = useDownloadsStore()
const player = usePlayerStore()
const removeDialog = ref<HTMLDialogElement>()
const pendingRemove = ref<DownloadRecord | null>(null)
function openRemove(record: DownloadRecord) { pendingRemove.value = record; removeDialog.value!.showModal() }
async function confirmRemove() {
  const mid = pendingRemove.value!.track.mid
  player.releaseDownload(mid)
  await downloads.remove(mid)
  removeDialog.value!.close()
  pendingRemove.value = null
}
const size = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`
onMounted(downloads.load)
</script>
<template>
  <section class="downloads-view">
    <div class="page-heading"><div><h1>下载与离线</h1><p>{{ downloads.records.length }} 首已下载</p></div><button class="text-button" :disabled="downloads.loading" @click="downloads.load"><RefreshCw :size="15" />刷新</button></div>
    <p v-if="downloads.error" class="inline-error" role="alert">{{ downloads.error }}</p>
    <div v-for="job in downloads.jobs" :key="job.track.mid" class="download-row">
      <CoverArt :src="job.track.cover" /><div class="download-info"><strong>{{ job.track.title }}</strong><small v-if="job.state === 'error'" class="download-error">{{ job.error }}</small><small v-else>{{ job.state === 'queued' ? '排队或正在获取音源' : `${size(job.received)}${job.total ? ` / ${size(job.total)}` : ''}` }}</small><progress v-if="job.state !== 'error'" :value="job.total ? job.received : undefined" :max="job.total || 1" :aria-label="`${job.track.title} 下载进度`"></progress></div>
      <button v-if="job.state === 'error'" class="text-button" @click="downloads.start(job.track)">重试</button><button v-else class="icon-button" :aria-label="`取消下载 ${job.track.title}`" @click="downloads.cancel(job.track.mid)"><X :size="17" /></button>
    </div>
    <div v-if="downloads.loading" class="empty-state"><LoaderCircle :size="24" class="spinning" /><p>正在读取本地下载</p></div>
    <div v-for="record in downloads.records" :key="record.filename" class="download-row">
      <button class="download-play" :aria-label="`离线播放 ${record.track.title}`" @click="player.play(record.track, downloads.records.map(item => item.track))"><CoverArt :src="record.track.cover" /><span class="download-info"><strong>{{ record.track.title }}</strong><small>{{ record.track.artist }} · {{ record.quality }} · {{ size(record.bytes) }}</small></span><Play :size="16" /></button>
      <FavoriteButton :track="record.track" />
      <button class="icon-button" :aria-label="`移除下载 ${record.track.title}`" title="仅移除此 Mac 上的音频" @click="openRemove(record)"><Trash2 :size="16" /></button>
    </div>
    <div v-if="!downloads.loading && !downloads.jobs.length && !downloads.records.length && !downloads.error" class="empty-state"><Download :size="32" /><h3>还没有下载的歌曲</h3></div>
    <dialog ref="removeDialog" class="feature-dialog" aria-labelledby="remove-download-title"><div class="dialog-heading"><h2 id="remove-download-title">移除本地下载</h2><button class="icon-button" aria-label="取消移除下载" @click="removeDialog?.close()"><X :size="18" /></button></div><p class="feature-copy">移除此 Mac 上的“{{ pendingRemove?.track.title }}”？QQ 音乐的收藏和歌单会保留。<span v-if="pendingRemove?.track.mid === player.current?.mid && player.localSource">当前本地播放会先暂停。</span></p><div class="dialog-actions"><button class="button secondary" @click="removeDialog?.close()">取消</button><button class="button primary danger" @click="confirmRemove">移除下载</button></div></dialog>
  </section>
</template>
