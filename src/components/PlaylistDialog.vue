<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { LoaderCircle, X } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import type { Playlist } from '../types'
const library = useLibraryStore()
const dialog = ref<HTMLDialogElement>()
const name = ref('')
const mode = ref<'create' | 'rename' | 'delete'>('create')
const selected = ref<Playlist | null>(null)
async function open(value: 'create' | 'rename' | 'delete', playlist: Playlist | null = null) {
  mode.value = value; selected.value = playlist; name.value = playlist ? playlist.name : ''; library.mutationError = ''
  await nextTick(); dialog.value!.showModal()
}
async function submit() {
  const success = await library.mutate(mode.value, selected.value ? selected.value.id : null, name.value, null)
  if (success) dialog.value!.close()
}
defineExpose({ open })
</script>
<template>
  <dialog ref="dialog" class="feature-dialog" aria-labelledby="playlist-edit-title" @cancel="library.mutationError = ''">
    <form @submit.prevent="submit">
      <div class="dialog-heading"><h2 id="playlist-edit-title">{{ mode === 'create' ? '创建歌单' : mode === 'rename' ? '重命名歌单' : '删除歌单' }}</h2><button class="icon-button" type="button" aria-label="关闭歌单编辑" :disabled="library.mutationBusy" @click="dialog?.close()"><X :size="18" /></button></div>
      <p v-if="mode === 'delete'" class="feature-copy">从 QQ 音乐中永久删除“{{ name }}”？歌曲本身和本地下载会保留。</p>
      <label v-else class="feature-field">歌单名称<input v-model="name" required maxlength="100" autofocus :disabled="library.mutationBusy" /></label>
      <p v-if="library.mutationError" class="inline-error" role="alert">{{ library.mutationError }}</p>
      <div class="dialog-actions"><button class="button secondary" type="button" :disabled="library.mutationBusy" @click="dialog?.close()">取消</button><button class="button primary" :class="{ danger: mode === 'delete' }" :disabled="library.mutationBusy || (mode !== 'delete' && !name.trim())"><LoaderCircle v-if="library.mutationBusy" :size="15" class="spinning" />{{ library.mutationBusy ? '正在向 QQ 音乐确认' : mode === 'delete' ? '永久删除' : '保存' }}</button></div>
    </form>
  </dialog>
</template>
