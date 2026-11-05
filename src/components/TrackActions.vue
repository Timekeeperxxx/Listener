<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { ChevronRight, Download, ListPlus, LoaderCircle, Trash2, X } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import { useDownloadsStore } from '../stores/downloads'
import type { Track } from '../types'
const library = useLibraryStore()
const downloads = useDownloadsStore()
const dialog = ref<HTMLDialogElement>()
const track = ref<Track | null>(null)
const addingTo = ref<string | null>(null)
const targets = computed(() => library.playlists.filter(playlist => playlist.kind === 'created'))
async function open(value: Track) {
  track.value = value; library.mutationError = ''
  await nextTick(); dialog.value!.showModal()
}
async function mutate(id: string, add: boolean) {
  addingTo.value = id
  try { if (await library.mutate(add ? 'add' : 'remove', id, null, track.value)) dialog.value!.close() }
  finally { addingTo.value = null }
}
function download() { void downloads.start(track.value!); dialog.value!.close() }
defineExpose({ open })
</script>
<template>
  <dialog ref="dialog" class="feature-dialog track-actions-dialog" aria-labelledby="track-actions-title">
    <div class="dialog-heading"><h2 id="track-actions-title">更多操作</h2><button class="icon-button" aria-label="关闭歌曲操作" :disabled="library.mutationBusy" @click="dialog?.close()"><X :size="18" /></button></div>
    <p v-if="track" class="feature-copy"><strong>{{ track.title }}</strong><br />{{ track.artist }}</p>
    <p v-if="library.mutationError" class="inline-error" role="alert">{{ library.mutationError }}</p>
    <h3 class="destination-heading">添加到歌单</h3>
    <div class="playlist-destinations"><button v-for="playlist in targets" :key="playlist.id" class="playlist-destination" :aria-label="`加入歌单 ${playlist.name}`" :disabled="library.mutationBusy || library.profile?.offline" @click="mutate(playlist.id, true)"><LoaderCircle v-if="library.mutationBusy && addingTo === playlist.id" :size="18" class="spinning" /><ListPlus v-else :size="18" /><span>{{ playlist.name }}</span><ChevronRight :size="16" /></button><p v-if="!targets.length" class="feature-footnote">还没有自建歌单，可在音乐库中创建。</p></div>
    <button v-if="library.view === 'playlist' && library.selected?.kind === 'created'" class="feature-action" :disabled="library.mutationBusy || library.profile?.offline" @click="mutate(library.selected.id, false)"><Trash2 :size="18" />从此歌单移除</button>
    <button class="feature-action" :disabled="library.profile?.offline" @click="download"><Download :size="18" />下载到此 Mac</button>
    <p class="feature-footnote">收藏和歌单操作会同步到 QQ 音乐；下载可在“下载与离线”中管理。</p>
  </dialog>
</template>
