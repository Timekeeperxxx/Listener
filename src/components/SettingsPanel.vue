<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Check, Download, LoaderCircle, LogOut, QrCode, RefreshCw } from '@lucide/vue'
import { api, desktop, errorMessage } from '../lib/api'
import { useLibraryStore } from '../stores/library'
import { usePlayerStore } from '../stores/player'
import type { ShortcutSettings, StorageUsage, UpdateInfo } from '../types'
import packageInfo from '../../package.json'
const props = defineProps<{ demo?: boolean }>()
const emit = defineEmits<{ logout: []; connect: []; companion: [kind: 'mini' | 'desktop-lyrics']; downloads: [] }>()
const library = useLibraryStore(), player = usePlayerStore()
const tab = ref<'playback' | 'lyrics' | 'shortcuts' | 'storage'>('playback')
const tabs = [{ id: 'playback', name: '播放' }, { id: 'lyrics', name: '歌词' }, { id: 'shortcuts', name: '快捷键' }, { id: 'storage', name: '存储与更新' }] as const
const storage = ref<StorageUsage | null>(null), storageError = ref(''), clearing = ref(false)
const shortcutDraft = ref<ShortcutSettings>({ ...player.settings.shortcuts }), shortcutError = ref(''), shortcutBusy = ref(false), shortcutSaved = ref(false)
const update = ref<UpdateInfo | null>(null), updateState = ref<'idle' | 'checking' | 'available' | 'latest' | 'installing' | 'error'>('idle'), updateError = ref(''), received = ref(0), total = ref<number | null>(null)
let unlistenUpdate: UnlistenFn | undefined
function bytes(value: number) { return value >= 1024 * 1024 ? `${(value / 1024 / 1024).toFixed(1)} MB` : `${(value / 1024).toFixed(1)} KB` }
async function refresh() {
  shortcutDraft.value = { ...player.settings.shortcuts }; shortcutSaved.value = false
  if (library.profile) await loadStorage()
  if (desktop) {
    try { shortcutError.value = (await api.desktopStatus()).shortcutError }
    catch (cause) { shortcutError.value = errorMessage(cause) }
  }
}
async function loadStorage() {
  storageError.value = ''
  try { storage.value = await api.storageUsage() }
  catch (cause) { storageError.value = errorMessage(cause) }
}
async function clearCache() {
  clearing.value = true; storageError.value = ''
  try { await api.clearLyricCache(); await loadStorage() }
  catch (cause) { storageError.value = errorMessage(cause) }
  finally { clearing.value = false }
}
function lyrics(values: Partial<typeof player.settings.lyrics>) { player.updateSettings({ lyrics: { ...player.settings.lyrics, ...values } }) }
function sleep(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  if (value !== 'active') player.setSleepTimer(value === 'off' ? null : value === 'track' ? 'track' : Number(value))
}
const shortcutActions: { id: 'toggle' | 'next' | 'previous' | 'main'; name: string }[] = [{ id: 'toggle', name: '播放 / 暂停' }, { id: 'previous', name: '上一首' }, { id: 'next', name: '下一首' }, { id: 'main', name: '显示主窗口' }]
function shortcutText(value: string) { return value.replaceAll('Control', '⌃').replaceAll('Alt', '⌥').replaceAll('Meta', '⌘').replaceAll('Super', '⌘').replaceAll('Shift', '⇧').replaceAll('Key', '').replaceAll('ArrowLeft', '←').replaceAll('ArrowRight', '→').replaceAll('+', ' ') }
function capture(event: KeyboardEvent, action: 'toggle' | 'next' | 'previous' | 'main') {
  if (event.key === 'Tab') return
  event.preventDefault(); event.stopPropagation()
  if (event.key === 'Escape') { (event.target as HTMLInputElement).blur(); return }
  if (['Control', 'Alt', 'Meta', 'Shift'].includes(event.key)) return
  const modifiers = [event.ctrlKey && 'Control', event.altKey && 'Alt', event.metaKey && 'Super', event.shiftKey && 'Shift'].filter(Boolean)
  if (modifiers.length < 2) { shortcutError.value = '请同时按住至少两个修饰键，再按一个按键。'; return }
  shortcutDraft.value = { ...shortcutDraft.value, [action]: [...modifiers, event.code].join('+') }
  shortcutError.value = ''; shortcutSaved.value = false
}
async function saveShortcuts() {
  shortcutBusy.value = true; shortcutError.value = ''; shortcutSaved.value = false
  try { await player.updateShortcuts({ ...shortcutDraft.value }); shortcutSaved.value = true }
  catch (cause) { shortcutError.value = errorMessage(cause) }
  finally { shortcutBusy.value = false }
}
async function checkUpdate() {
  updateState.value = 'checking'; updateError.value = ''
  try { update.value = await api.checkUpdate(); updateState.value = update.value ? 'available' : 'latest' }
  catch (cause) { updateState.value = 'error'; updateError.value = errorMessage(cause) }
}
async function installUpdate() {
  updateState.value = 'installing'; updateError.value = ''; received.value = 0; total.value = null
  try { player.pause(); await player.flushPlayback(); await api.installUpdate() }
  catch (cause) { updateState.value = 'error'; updateError.value = errorMessage(cause) }
}
onMounted(async () => { if (desktop) unlistenUpdate = await listen<{ received: number; total: number | null }>('update-progress', event => { received.value = event.payload.received; total.value = event.payload.total }) })
onUnmounted(() => unlistenUpdate?.())
defineExpose({ refresh })
</script>
<template>
  <div class="settings-panel">
    <nav class="settings-tabs" aria-label="设置分类"><button v-for="item in tabs" :key="item.id" :class="{ selected: tab === item.id }" @click="tab = item.id">{{ item.name }}</button></nav>
    <div v-if="tab === 'playback'">
      <div v-if="library.profile" class="settings-account"><img v-if="library.profile.avatar" :src="library.profile.avatar" alt="" class="avatar" /><div><strong>{{ library.profile.nickname }}</strong><span>{{ demo ? '网页演示' : library.profile.offline ? '离线模式' : '已连接 QQ 音乐' }}</span></div><Check :size="18" /></div>
      <div class="setting-row"><div><strong>播放与下载音质</strong><p>优先播放已下载的本地版本</p></div><select :value="player.settings.audioQuality" aria-label="音质" @change="player.updateSettings({ audioQuality: ($event.target as HTMLSelectElement).value as typeof player.settings.audioQuality })"><option value="auto">自动 · MP3</option><option value="standard">128 kbps</option><option value="high">320 kbps</option><option value="lossless">无损 FLAC</option><option value="hires">Hi-Res FLAC</option></select></div>
      <div class="setting-row"><div><strong>下一首预加载</strong><p>在歌曲结束前准备下一首，减少切歌等待</p></div><button class="switch" :class="{ on: player.settings.preloadNext }" role="switch" :aria-checked="player.settings.preloadNext" aria-label="下一首预加载" @click="player.updateSettings({ preloadNext: !player.settings.preloadNext })"><span></span></button></div>
      <p v-if="player.preloadError" class="inline-error" role="status">下一首预加载失败：{{ player.preloadError }}。切歌时会重新读取音源。</p>
      <div class="setting-row"><div><strong>关闭窗口后继续播放</strong><p>通过菜单栏或 Dock 重新打开</p></div><button class="switch" :class="{ on: player.settings.closeToTray }" role="switch" :aria-checked="player.settings.closeToTray" aria-label="关闭窗口后继续播放" @click="player.updateSettings({ closeToTray: !player.settings.closeToTray })"><span></span></button></div>
      <div class="setting-row"><div><strong>定时停止</strong><p>{{ player.sleepAfterTrack ? '当前歌曲播完后停止' : player.sleepDeadline ? `剩余约 ${player.sleepRemaining} 分钟` : '到时暂停' }}</p></div><select :value="player.sleepAfterTrack ? 'track' : player.sleepDeadline ? 'active' : 'off'" aria-label="定时停止" @change="sleep"><option value="off">关闭</option><option v-if="player.sleepDeadline" value="active">剩余 {{ player.sleepRemaining }} 分钟</option><option value="15">15 分钟</option><option value="30">30 分钟</option><option value="60">60 分钟</option><option value="120">120 分钟</option><option value="track">播完本首</option></select></div>
      <div class="setting-row desktop-tools"><div><strong>桌面工具</strong><p>置顶显示，可拖动位置</p></div><div><button class="button secondary" :disabled="!desktop" @click="emit('companion', 'mini')">迷你播放器</button><button class="button secondary" :disabled="!desktop" @click="emit('companion', 'desktop-lyrics')">桌面歌词</button></div></div>
      <p class="feature-footnote">歌手和专辑收藏保存在此 Mac，按 QQ 账户区分。</p>
      <button v-if="library.profile" class="button secondary logout-button" @click="emit('logout')"><LogOut :size="15" />断开 QQ 音乐</button><button v-else class="button primary logout-button" @click="emit('connect')"><QrCode :size="15" />连接 QQ 音乐</button>
    </div>
    <div v-else-if="tab === 'lyrics'">
      <div class="setting-row"><div><strong>歌词字号</strong></div><label class="settings-slider"><input type="range" min="70" max="150" step="5" :value="player.settings.lyrics.fontScale" aria-label="歌词字号" @input="lyrics({ fontScale: Number(($event.target as HTMLInputElement).value) })" /><span>{{ player.settings.lyrics.fontScale }}%</span></label></div>
      <div class="setting-row"><div><strong>歌词行距</strong></div><label class="settings-slider"><input type="range" min="1.2" max="2.2" step="0.1" :value="player.settings.lyrics.lineHeight" aria-label="歌词行距" @input="lyrics({ lineHeight: Number(($event.target as HTMLInputElement).value) })" /><span>{{ player.settings.lyrics.lineHeight.toFixed(1) }}</span></label></div>
      <div class="setting-row"><strong>显示译文</strong><button class="switch" :class="{ on: player.settings.lyrics.showTranslation }" role="switch" :aria-checked="player.settings.lyrics.showTranslation" aria-label="显示译文" @click="lyrics({ showTranslation: !player.settings.lyrics.showTranslation })"><span></span></button></div>
      <div class="setting-row"><strong>显示音译</strong><button class="switch" :class="{ on: player.settings.lyrics.showRomanization }" role="switch" :aria-checked="player.settings.lyrics.showRomanization" aria-label="显示音译" @click="lyrics({ showRomanization: !player.settings.lyrics.showRomanization })"><span></span></button></div>
      <div class="setting-row"><div><strong>流动背景</strong></div><button class="switch" :class="{ on: player.settings.animatedBackground }" role="switch" :aria-checked="player.settings.animatedBackground" aria-label="流动背景" @click="player.updateSettings({ animatedBackground: !player.settings.animatedBackground })"><span></span></button></div>
      <div class="setting-row"><div><strong>全局时间微调</strong><p>正值提前，负值延后</p></div><label class="offset-input"><input type="number" min="-5000" max="5000" step="100" :value="player.settings.lyricOffset" aria-label="全局歌词偏移毫秒" @change="player.updateSettings({ lyricOffset: Number(($event.target as HTMLInputElement).value) })" /><span>ms</span></label></div>
      <div v-if="player.current" class="setting-row"><div><strong>本首额外微调</strong><p>{{ player.current.title }}</p></div><label class="offset-input"><input type="number" min="-5000" max="5000" step="100" :value="player.trackOffset" aria-label="本首歌词偏移毫秒" @change="player.updateTrackOffset(Number(($event.target as HTMLInputElement).value))" /><span>ms</span></label></div>
      <p v-if="player.trackOffsetError" class="inline-error" role="alert">{{ player.trackOffsetError }}</p>
    </div>
    <div v-else-if="tab === 'shortcuts'">
      <div class="setting-row"><strong>全局快捷键</strong><button class="switch" :class="{ on: shortcutDraft.enabled }" role="switch" :aria-checked="shortcutDraft.enabled" aria-label="全局快捷键" @click="shortcutDraft.enabled = !shortcutDraft.enabled; shortcutSaved = false"><span></span></button></div>
      <div v-for="action in shortcutActions" :key="action.id" class="setting-row"><strong>{{ action.name }}</strong><input class="shortcut-capture" readonly :disabled="!shortcutDraft.enabled || shortcutBusy" :aria-label="`${action.name}全局快捷键`" :value="shortcutText(shortcutDraft[action.id])" @keydown="capture($event, action.id)" /></div>
      <p class="feature-footnote">点击按键框，按下修饰键和目标按键。至少使用两个修饰键。</p>
      <p v-if="shortcutError" class="inline-error" role="alert">{{ shortcutError }}</p><p v-if="shortcutSaved" class="setting-success" role="status">快捷键已保存</p>
      <button class="button primary" :disabled="!desktop || shortcutBusy" @click="saveShortcuts"><LoaderCircle v-if="shortcutBusy" :size="15" class="spinning" />应用快捷键</button>
    </div>
    <div v-else>
      <template v-if="library.profile"><div v-if="storage" class="storage-overview"><div><strong>{{ bytes(storage.cacheBytes) }}</strong><span>歌词缓存 · {{ storage.cacheCount }} 首</span></div><div><strong>{{ bytes(storage.downloadBytes) }}</strong><button class="text-button" @click="emit('downloads')">已下载音乐 · {{ storage.downloadCount }} 首</button></div><div><strong>{{ bytes(storage.databaseBytes) }}</strong><span>本机资料文件</span></div></div><p v-if="storageError" class="inline-error" role="alert">{{ storageError }}</p><div class="setting-row"><strong>歌词缓存上限</strong><select :value="player.settings.cacheLimitMb" aria-label="歌词缓存上限" @change="player.updateSettings({ cacheLimitMb: Number(($event.target as HTMLSelectElement).value) })"><option v-for="size in [16,32,64,128,256,512]" :key="size" :value="size">{{ size }} MB</option></select></div><button class="button secondary" :disabled="clearing || !storage?.cacheCount" @click="clearCache"><LoaderCircle v-if="clearing" :size="15" class="spinning" />清理歌词缓存</button></template><p v-else class="feature-copy">连接 QQ 音乐后查看本机存储。</p>
      <section class="update-section"><h3>应用更新</h3><p class="feature-copy">当前版本 {{ packageInfo.version }}</p><p v-if="updateState === 'latest'" class="setting-success" role="status">已是最新版本</p><p v-if="update" class="feature-copy">可更新至 {{ update.version }}</p><p v-if="update?.notes" class="update-notes">{{ update.notes }}</p><p v-if="updateState === 'installing'" role="status">{{ received ? `正在下载 ${bytes(received)}${total ? ` / ${bytes(total)}` : ''}` : '正在准备更新' }}</p><p v-if="updateError" class="inline-error" role="alert">{{ updateError }}</p><div class="settings-update-actions"><button class="button secondary" :disabled="!desktop || updateState === 'checking' || updateState === 'installing'" @click="checkUpdate"><RefreshCw :size="14" :class="{ spinning: updateState === 'checking' }" />{{ updateState === 'checking' ? '正在检查' : '检查更新' }}</button><button v-if="update" class="button primary" :disabled="updateState === 'installing'" @click="installUpdate"><Download :size="14" />安装并重启</button></div></section>
    </div>
    <div class="settings-footer"><span>Listener {{ packageInfo.version }}</span><span>macOS</span></div>
  </div>
</template>
