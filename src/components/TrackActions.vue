<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { ChevronLeft, ChevronRight, Download, Heart, ListEnd, ListMusic, ListPlus, ListStart, LoaderCircle, Pause, Play, Plus, Trash2, X } from '@lucide/vue'
import { useLibraryStore } from '../stores/library'
import { useDownloadsStore } from '../stores/downloads'
import { usePlayerStore } from '../stores/player'
import { positionMenu, positionSubmenu } from '../lib/menu-position'
import type { TrackMenuTarget } from '../types'
const emit = defineEmits<{ downloads: [] }>()
const library = useLibraryStore(), downloads = useDownloadsStore(), player = usePlayerStore()
const context = shallowRef<TrackMenuTarget | null>(null)
const host = ref<HTMLElement>(), panel = ref<HTMLElement>(), subPanel = ref<HTMLElement>()
const addTrigger = ref<HTMLButtonElement>(), downloadTrigger = ref<HTMLButtonElement>()
const submenu = ref<'playlists' | 'download' | null>(null)
const positioned = ref(false), subPositioned = ref(false), opensLeft = ref(false)
const location = ref({ left: 0, top: 0 }), subLocation = ref({ left: 0, top: 0 })
const addingTo = ref<string | null>(null)
const feedback = ref(''), feedbackError = ref(false)
let feedbackTimer: ReturnType<typeof setTimeout> | undefined
let accountEpoch = 0
const targets = computed(() => library.playlists.filter(item => item.kind === 'created'))
const isCurrentPlaying = computed(() => context.value?.track.mid === player.current?.mid && player.playing)
const isLiked = computed(() => !!context.value && library.likedMids.has(context.value.track.mid))
const online = computed(() => !!library.profile && !library.profile.offline)
const downloading = computed(() => downloads.jobs.some(job => job.track.mid === context.value?.track.mid && job.state !== 'error'))
const menuStyle = computed(() => ({ left: `${location.value.left}px`, top: `${location.value.top}px`, visibility: positioned.value ? 'visible' as const : 'hidden' as const }))
const submenuStyle = computed(() => ({ left: `${subLocation.value.left}px`, top: `${subLocation.value.top}px`, visibility: subPositioned.value ? 'visible' as const : 'hidden' as const }))
const items = (element: HTMLElement) => Array.from(element.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled),button[role="menuitemcheckbox"]:not(:disabled)'))
function notify(message: string, error = false) { clearTimeout(feedbackTimer); feedback.value = message; feedbackError.value = error; if (!error) feedbackTimer = setTimeout(() => { feedback.value = '' }, 3200) }
function close(restoreFocus = true) {
  const anchor = context.value?.anchor
  context.value = null; submenu.value = null; positioned.value = false
  if (anchor) { anchor.setAttribute('aria-expanded', 'false'); if (restoreFocus && anchor.isConnected) anchor.focus({ preventScroll: true }) }
}
async function open(target: TrackMenuTarget) {
  if (context.value?.anchor === target.anchor) { close(); return }
  close(false)
  context.value = target; library.mutationError = ''; submenu.value = null; positioned.value = false
  target.anchor.setAttribute('aria-expanded', 'true')
  await nextTick()
  if (context.value !== target) return
  location.value = positionMenu(target.anchor.getBoundingClientRect(), panel.value!.getBoundingClientRect(), { width: innerWidth, height: innerHeight })
  positioned.value = true
  await nextTick()
  if (context.value === target) items(panel.value!)[0]!.focus({ preventScroll: true })
}
async function showSubmenu(kind: 'playlists' | 'download', focus = false) {
  submenu.value = kind; subPositioned.value = false
  await nextTick()
  if (!context.value || submenu.value !== kind) return
  const trigger = kind === 'playlists' ? addTrigger.value! : downloadTrigger.value!
  const result = positionSubmenu(panel.value!.getBoundingClientRect(), trigger.getBoundingClientRect(), subPanel.value!.getBoundingClientRect(), { width: innerWidth, height: innerHeight })
  subLocation.value = { left: result.left, top: result.top }; opensLeft.value = result.opensLeft; subPositioned.value = true
  await nextTick()
  if (focus && context.value && submenu.value === kind) (items(subPanel.value!)[0] ?? subPanel.value!).focus({ preventScroll: true })
}
function hideSubmenu() { submenu.value = null }
function outside(event: Event) {
  if (context.value && !host.value!.contains(event.target as Node) && !context.value.anchor.contains(event.target as Node)) close(false)
}
function scroll(event: Event) { if (context.value && !host.value!.contains(event.target as Node)) close(false) }
function dismiss() { close(false) }
function keyboard(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); close(); return }
  if (event.key === 'Tab') { close(); return }
  if (event.key === 'ArrowRight' && (event.target === addTrigger.value || event.target === downloadTrigger.value)) { event.preventDefault(); void showSubmenu(event.target === addTrigger.value ? 'playlists' : 'download', true); return }
  if (event.key === 'ArrowLeft' && submenu.value) { event.preventDefault(); const trigger = submenu.value === 'playlists' ? addTrigger.value! : downloadTrigger.value!; hideSubmenu(); trigger.focus({ preventScroll: true }); return }
  if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  const container = (event.target as HTMLElement).closest<HTMLElement>('[role="menu"]')!
  const buttons = items(container)
  if (!buttons.length) return // An empty playlist submenu has no actionable destination.
  const index = buttons.indexOf(document.activeElement as HTMLButtonElement)
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : index < 0 ? (event.key === 'ArrowDown' ? 0 : buttons.length - 1) : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length
  buttons[next]!.focus({ preventScroll: true }); buttons[next]!.scrollIntoView({ block: 'nearest' })
}
function play() {
  const target = context.value!
  if (target.track.mid === player.current?.mid) void player.toggle()
  else {
    if (target.sequential && player.shuffle) player.setShuffle(false)
    void player.play(target.track, target.tracks)
  }
  close()
}
function enqueue(next: boolean) { player.enqueue(context.value!.track, next); close() }
async function like() { const target = context.value!, epoch = accountEpoch; await library.toggleLike(target.track); if (epoch === accountEpoch && context.value === target && !library.favoriteError) close() }
async function mutate(id: string, name: string, add: boolean) {
  const target = context.value!, epoch = accountEpoch
  addingTo.value = id
  try {
    const success = await library.mutate(add ? 'add' : 'remove', id, null, target.track)
    if (epoch !== accountEpoch) return
    if (success) { notify(add ? `已添加到“${name}”` : '已从歌单移除'); if (context.value === target) close() }
    else if (library.mutationError) notify(library.mutationError, true)
  } finally { if (epoch === accountEpoch) addingTo.value = null }
}
async function download() {
  const target = context.value!, epoch = accountEpoch
  const pending = downloads.start(target.track)
  notify(`已加入下载队列：${target.track.title}`); close()
  await pending
  if (epoch !== accountEpoch) return
  const failed = downloads.jobs.find(job => job.track.mid === target.track.mid && job.state === 'error')
  if (failed) notify(failed.error, true)
  else if (downloads.records.some(record => record.track.mid === target.track.mid)) notify(`下载完成：${target.track.title}`)
}
function showDownloads() { close(); emit('downloads') }
watch(() => library.profile?.uin, () => { accountEpoch++; close(false); addingTo.value = null; feedback.value = ''; clearTimeout(feedbackTimer) }, { flush: 'sync' })
onMounted(() => { document.addEventListener('pointerdown', outside, true); document.addEventListener('focusin', outside); document.addEventListener('scroll', scroll, true); window.addEventListener('resize', dismiss); window.addEventListener('blur', dismiss) })
onUnmounted(() => { close(false); clearTimeout(feedbackTimer); document.removeEventListener('pointerdown', outside, true); document.removeEventListener('focusin', outside); document.removeEventListener('scroll', scroll, true); window.removeEventListener('resize', dismiss); window.removeEventListener('blur', dismiss) })
defineExpose({ open, close })
</script>

<template>
  <div v-if="context" ref="host" class="track-menu-host" @keydown.stop="keyboard" @contextmenu.prevent>
    <div ref="panel" class="track-context-menu" role="menu" :aria-label="`${context.track.title} 的更多操作`" :style="menuStyle">
      <button class="track-menu-item" role="menuitem" @pointerenter="hideSubmenu" @click="play"><Pause v-if="isCurrentPlaying" :size="17"/><Play v-else :size="17"/><span>{{ isCurrentPlaying ? '暂停' : '播放' }}</span></button>
      <button class="track-menu-item" role="menuitem" @pointerenter="hideSubmenu" @click="enqueue(true)"><ListStart :size="17"/><span>下一首播放</span></button>
      <button class="track-menu-item" role="menuitem" @pointerenter="hideSubmenu" @click="enqueue(false)"><ListEnd :size="17"/><span>加入播放队列</span></button>
      <div class="track-menu-separator" role="separator"></div>
      <button class="track-menu-item" role="menuitemcheckbox" :aria-checked="isLiked" :disabled="!online || library.likedState === 'loading' || library.mutationBusy" @pointerenter="hideSubmenu" @click="like"><Heart :size="17" :fill="isLiked ? 'currentColor' : 'none'"/><span>{{ isLiked ? '取消喜欢' : '我喜欢' }}</span></button>
      <button ref="addTrigger" class="track-menu-item" :class="{ expanded: submenu === 'playlists' }" role="menuitem" aria-haspopup="menu" :aria-expanded="submenu === 'playlists'" :disabled="!online" @pointerenter="showSubmenu('playlists')" @click="showSubmenu('playlists', true)"><Plus :size="17"/><span>添加到</span><ChevronLeft v-if="submenu === 'playlists' && opensLeft" class="track-menu-arrow" :size="14"/><ChevronRight v-else class="track-menu-arrow" :size="14"/></button>
      <button ref="downloadTrigger" class="track-menu-item" :class="{ expanded: submenu === 'download' }" role="menuitem" aria-haspopup="menu" :aria-expanded="submenu === 'download'" @pointerenter="showSubmenu('download')" @click="showSubmenu('download', true)"><Download :size="17"/><span>下载</span><ChevronLeft v-if="submenu === 'download' && opensLeft" class="track-menu-arrow" :size="14"/><ChevronRight v-else class="track-menu-arrow" :size="14"/></button>
      <template v-if="library.view === 'playlist' && library.selected?.kind === 'created'"><div class="track-menu-separator" role="separator"></div><button class="track-menu-item destructive" role="menuitem" :disabled="!online || library.mutationBusy" @pointerenter="hideSubmenu" @click="mutate(library.selected.id, library.selected.name, false)"><Trash2 :size="17"/><span>从此歌单移除</span></button></template>
    </div>
    <div v-if="submenu" ref="subPanel" class="track-context-menu track-context-submenu" role="menu" :aria-label="submenu === 'playlists' ? '添加到歌单' : '下载选项'" tabindex="-1" :style="submenuStyle">
      <template v-if="submenu === 'playlists'"><p v-if="library.libraryState === 'loading' && !targets.length" class="track-menu-empty" role="status">正在读取歌单</p><template v-else-if="library.libraryError"><p class="track-menu-error" role="alert">{{ library.libraryError }}</p><button class="track-menu-item" role="menuitem" @click="library.refresh">重新读取歌单</button></template><button v-for="playlist in targets" :key="playlist.id" class="track-menu-item" role="menuitem" :disabled="library.mutationBusy || !online" :title="playlist.name" @click="mutate(playlist.id, playlist.name, true)"><LoaderCircle v-if="library.mutationBusy && addingTo === playlist.id" :size="17" class="spinning"/><ListPlus v-else :size="17"/><span>{{ playlist.name }}</span></button><p v-if="!targets.length && library.libraryState === 'ready'" class="track-menu-empty">暂无自建歌单</p></template>
      <template v-else><button class="track-menu-item" role="menuitem" :disabled="!online || downloading" @click="download"><Download :size="17"/><span>{{ downloading ? '正在下载' : '下载到此 Mac' }}</span></button><button class="track-menu-item" role="menuitem" @click="showDownloads"><ListMusic :size="17"/><span>下载与离线</span></button></template>
    </div>
  </div>
  <div v-if="feedback" class="track-action-feedback" :class="{ error: feedbackError }" :role="feedbackError ? 'alert' : 'status'"><span>{{ feedback }}</span><button class="icon-button small" aria-label="关闭操作提示" @click="feedback = ''"><X :size="15"/></button></div>
</template>

<style>
.track-menu-host{position:fixed;inset:0;z-index:70;pointer-events:none}
.track-context-menu{position:fixed;width:216px;max-width:calc(100vw - 16px);max-height:calc(100vh - 16px);overflow-y:auto;padding:6px;border:1px solid var(--border);border-radius:11px;background:var(--dock);color:var(--ink);box-shadow:0 12px 38px #16243a26,0 2px 7px #16243a0d;backdrop-filter:blur(24px) saturate(150%);-webkit-backdrop-filter:blur(24px) saturate(150%);pointer-events:auto;user-select:none;-webkit-user-select:none}
[data-theme=dark] .track-context-menu{background:rgba(40,43,49,.97);border-color:#ffffff16;box-shadow:0 12px 38px #0006,0 2px 7px #0004}
.track-menu-item{display:flex;align-items:center;gap:11px;width:100%;min-height:34px;padding:7px 10px;border-radius:6px;background:none;color:var(--ink);font-size:13px;font-weight:400;text-align:left;line-height:20px;outline:none!important}
.track-menu-item>svg{flex-shrink:0;color:var(--muted)}.track-menu-item>span{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.track-menu-item .track-menu-arrow{margin-left:auto;flex-shrink:0;opacity:.7}
.track-menu-item:hover:not(:disabled),.track-menu-item:focus-visible,.track-menu-item.expanded{background:var(--wash);color:var(--accent)}.track-menu-item:hover:not(:disabled)>svg,.track-menu-item:focus-visible>svg,.track-menu-item.expanded>svg{color:var(--accent)}.track-menu-item:disabled{opacity:.43}.track-menu-item.destructive{color:#d4525f}
.track-menu-separator{height:1px;margin:5px 4px;background:var(--line)}.track-context-submenu{width:232px;max-height:min(360px,calc(100vh - 16px));z-index:1}.track-context-submenu:focus-visible{outline:2px solid var(--accent);outline-offset:1px}
.track-menu-empty,.track-menu-error{padding:8px 10px;margin:0;font-size:12px;color:var(--muted);line-height:1.6}.track-menu-error{color:#d4525f;overflow-wrap:anywhere}
.track-action-feedback{position:fixed;z-index:60;right:28px;bottom:96px;display:flex;align-items:center;gap:12px;max-width:min(450px,calc(100vw - 40px));padding:10px 12px 10px 15px;border:1px solid var(--border);border-radius:10px;background:var(--dock);color:var(--ink);font-size:12px;line-height:1.6;box-shadow:0 8px 28px #0002;backdrop-filter:blur(20px);-webkit-backdrop-filter:blur(20px)}.track-action-feedback.error{border-color:#d4525f66}.track-action-feedback>span{overflow-wrap:anywhere}
</style>
