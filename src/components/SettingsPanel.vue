<script setup lang="ts">
import { computed, markRaw, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  Captions, Check, ChevronDown, Command, Download, HardDrive, LoaderCircle,
  LogOut, Moon, Palette, Play, QrCode, RefreshCw, RotateCcw, Sun, UserRound, X,
} from '@lucide/vue'
import { api, desktop, errorMessage } from '../lib/api'
import { defaultSettings } from '../lib/preferences'
import { useLibraryStore } from '../stores/library'
import { usePlayerStore } from '../stores/player'
import type { ShortcutSettings, StorageUsage, UpdateInfo } from '../types'
import packageInfo from '../../package.json'

const props = defineProps<{ demo?: boolean; theme: 'light' | 'dark' }>()
const emit = defineEmits<{
  close: []; logout: []; connect: []; companion: [kind: 'mini' | 'desktop-lyrics']; downloads: []
  'update:theme': [theme: 'light' | 'dark']
}>()
const library = useLibraryStore(), player = usePlayerStore()

const sections = [
  { id: 'account', name: '账户', title: '账户', description: '管理 QQ 音乐登录与本机同步状态。', icon: markRaw(UserRound) },
  { id: 'appearance', name: '外观', title: '外观', description: '选择界面主题与歌词页的视觉呈现。', icon: markRaw(Palette) },
  { id: 'playback', name: '播放', title: '播放', description: '调整音质、衔接、后台播放与桌面工具。', icon: markRaw(Play) },
  { id: 'lyrics', name: '歌词', title: '歌词', description: '调整歌词排版、译文和时间轴。', icon: markRaw(Captions) },
  { id: 'shortcuts', name: '快捷键', title: '快捷键', description: '设置在其他应用中也能使用的全局快捷键。', icon: markRaw(Command) },
  { id: 'storage', name: '存储与更新', title: '存储与更新', description: '管理本机缓存、下载与应用版本。', icon: markRaw(HardDrive) },
] as const
type Section = (typeof sections)[number]['id']
const tab = ref<Section>('account')
const current = computed(() => sections.find(section => section.id === tab.value)!)

const storage = ref<StorageUsage | null>(null), storageError = ref(''), clearing = ref(false)
const shortcutDraft = ref<ShortcutSettings>({ ...player.settings.shortcuts }), shortcutError = ref(''), shortcutBusy = ref(false), shortcutSaved = ref(false)
const update = ref<UpdateInfo | null>(null), updateState = ref<'idle' | 'checking' | 'available' | 'latest' | 'installing' | 'error'>('idle'), updateError = ref(''), received = ref(0), total = ref<number | null>(null)
const reconnecting = ref(false)
let unlistenUpdate: UnlistenFn | undefined

const accountStatus = computed(() => props.demo ? '网页演示' : !library.profile ? '未连接' : library.profile.offline ? '离线模式' : '已连接 QQ 音乐')
const accountTone = computed(() => props.demo ? 'demo' : !library.profile ? 'guest' : library.profile.offline ? 'offline' : 'online')
const shortcutActions: { id: 'toggle' | 'next' | 'previous' | 'main'; name: string }[] = [
  { id: 'toggle', name: '播放 / 暂停' }, { id: 'previous', name: '上一首' }, { id: 'next', name: '下一首' }, { id: 'main', name: '显示主窗口' },
]

function bytes(value: number) { return value >= 1024 * 1024 ? `${(value / 1024 / 1024).toFixed(1)} MB` : `${(value / 1024).toFixed(1)} KB` }
function setTheme(value: 'light' | 'dark') { emit('update:theme', value) }
function lyrics(values: Partial<typeof player.settings.lyrics>) { player.updateSettings({ lyrics: { ...player.settings.lyrics, ...values } }) }
function setQuality(event: Event) { player.updateSettings({ audioQuality: (event.target as HTMLSelectElement).value as typeof player.settings.audioQuality }) }
function setCacheLimit(event: Event) { player.updateSettings({ cacheLimitMb: Number((event.target as HTMLSelectElement).value) }) }
function sleep(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  if (value !== 'active') player.setSleepTimer(value === 'off' ? null : value === 'track' ? 'track' : Number(value))
}
function resetGlobalOffset() { player.updateSettings({ lyricOffset: 0 }) }
function resetTrackOffset() { if (player.current) void player.updateTrackOffset(0) }
function resetShortcuts() { shortcutDraft.value = { ...defaultSettings().shortcuts }; shortcutSaved.value = false; shortcutError.value = '' }
async function reconnect() {
  reconnecting.value = true
  try { await library.initialize() } finally { reconnecting.value = false }
}
async function refresh() {
  shortcutDraft.value = { ...player.settings.shortcuts }; shortcutSaved.value = false; shortcutError.value = ''
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
function shortcutText(value: string) {
  return value.replaceAll('Control', '⌃').replaceAll('Alt', '⌥').replaceAll('Meta', '⌘').replaceAll('Super', '⌘').replaceAll('Shift', '⇧').replaceAll('Key', '').replaceAll('ArrowLeft', '←').replaceAll('ArrowRight', '→').replaceAll('+', ' ')
}
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
watch(tab, value => { if (value === 'storage' && library.profile) void loadStorage() })
watch(() => library.profile?.uin, owner => { if (!owner) { storage.value = null; update.value = null } })
onMounted(async () => { if (desktop) unlistenUpdate = await listen<{ received: number; total: number | null }>('update-progress', event => { received.value = event.payload.received; total.value = event.payload.total }) })
onUnmounted(() => unlistenUpdate?.())
defineExpose({ refresh })
</script>

<template>
  <div class="settings-shell">
    <aside class="settings-sidebar">
      <div class="settings-brand">
        <span class="settings-brand-mark"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 10v4M8 7v10M12 4v16M16 7v10M20 10v4" /></svg></span>
        <div><strong>Listener</strong><small>设置</small></div>
      </div>
      <nav class="settings-nav" aria-label="设置分类">
        <button v-for="section in sections" :key="section.id" :class="{ selected: tab === section.id }" :aria-current="tab === section.id ? 'page' : undefined" @click="tab = section.id">
          <component :is="section.icon" :size="17" :stroke-width="1.8" />
          <span>{{ section.name }}</span>
          <i v-if="tab === section.id" aria-hidden="true"></i>
        </button>
      </nav>
      <div class="settings-sidebar-foot"><span>Listener {{ packageInfo.version }}</span><span>macOS</span></div>
    </aside>

    <section class="settings-main" :aria-labelledby="`settings-${tab}-title`">
      <header class="settings-header">
        <div>
          <span class="settings-eyebrow">设置</span>
          <h2 :id="`settings-${tab}-title`">{{ current.title }}</h2>
          <p>{{ current.description }}</p>
        </div>
        <button class="settings-close" aria-label="关闭设置" title="关闭设置" @click="emit('close')"><X :size="19" /></button>
      </header>

      <div class="settings-content">
        <template v-if="tab === 'account'">
          <section class="settings-card settings-account-card">
            <div class="settings-account-main">
              <img v-if="library.profile?.avatar" :src="library.profile.avatar" alt="" class="settings-avatar" />
              <span v-else class="settings-avatar settings-avatar-fallback"><UserRound :size="22" /></span>
              <div class="settings-account-name">
                <strong>{{ library.profile?.nickname || (demo ? '网页演示' : '未连接 QQ 音乐') }}</strong>
                <span class="settings-status" :data-tone="accountTone">{{ accountStatus }}</span>
              </div>
              <button v-if="library.profile && !library.profile.offline && !demo" class="settings-link danger" @click="emit('logout')"><LogOut :size="15" />退出</button>
              <button v-else-if="library.profile?.offline" class="settings-link" :disabled="reconnecting" @click="reconnect"><LoaderCircle v-if="reconnecting" :size="15" class="spinning" /><RefreshCw v-else :size="15" />重新连接</button>
              <button v-else-if="!library.profile && !demo" class="button primary settings-small-button" @click="emit('connect')"><QrCode :size="15" />连接</button>
            </div>
            <p class="settings-card-note">登录凭据只保存在此 Mac 的钥匙串；歌单、播放位置与收藏按 QQ 账户隔离。</p>
          </section>

          <div class="settings-group">
            <h3>同步</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>QQ 音乐账户</strong><p>扫码授权后读取歌单、我喜欢和公开目录。</p></div><span class="settings-value">{{ library.profile ? '已授权' : '未连接' }}</span></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>歌手与专辑收藏</strong><p>关注歌手和收藏专辑会同步到 QQ 音乐。</p></div><span class="settings-value">自动同步</span></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>播放位置</strong><p>队列与进度只保存在此 Mac，并在下次打开时恢复。</p></div><span class="settings-value">本机</span></div>
            </div>
          </div>

          <div v-if="demo" class="settings-group"><div class="settings-card settings-demo-card"><strong>网页演示</strong><p>此页面只演示界面；账户登录、真实播放和同步功能需要在 macOS 应用中使用。</p></div></div>
        </template>

        <template v-else-if="tab === 'appearance'">
          <div class="settings-group">
            <h3>主题</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>界面外观</strong><p>跟随你的偏好，立即应用到整个窗口。</p></div><div class="settings-segmented" role="group" aria-label="界面外观">
                <button :class="{ selected: theme === 'light' }" :aria-pressed="theme === 'light'" @click="setTheme('light')"><Sun :size="14" />浅色</button>
                <button :class="{ selected: theme === 'dark' }" :aria-pressed="theme === 'dark'" @click="setTheme('dark')"><Moon :size="14" />深色</button>
              </div></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>歌词流动背景</strong><p>歌词页随封面颜色变化的柔和动态背景。</p></div><button class="settings-switch" :class="{ on: player.settings.animatedBackground }" role="switch" :aria-checked="player.settings.animatedBackground" aria-label="歌词流动背景" @click="player.updateSettings({ animatedBackground: !player.settings.animatedBackground })"><span></span></button></div>
            </div>
          </div>
          <p class="settings-note">主题选择保存在当前 Mac，不随 QQ 账户同步。</p>
        </template>

        <template v-else-if="tab === 'playback'">
          <div class="settings-group">
            <h3>声音</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>播放与下载音质</strong><p>优先播放已下载的本地版本；实际可用音质由 QQ 音乐决定。</p></div><label class="settings-select"><select :value="player.settings.audioQuality" aria-label="播放与下载音质" @change="setQuality"><option value="auto">自动 · MP3</option><option value="standard">128 kbps</option><option value="high">320 kbps</option><option value="lossless">无损 FLAC</option><option value="hires">Hi-Res FLAC</option></select><ChevronDown :size="14" /></label></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>自动衔接</strong><p>按 QQ 音乐的切点交叉淡化，不改变播放速度。</p></div><button class="settings-switch" :class="{ on: player.settings.smartMix }" role="switch" :aria-checked="player.settings.smartMix" aria-label="自动衔接" @click="player.updateSettings({ smartMix: !player.settings.smartMix, preloadNext: true })"><span></span></button></div>
              <p v-if="player.settings.smartMix && player.mixState === 'idle'" class="settings-row-note">播放新歌曲后生效。</p>
              <p v-else-if="player.mixMode" class="settings-row-note" role="status">{{ player.mixMode }}中</p>
              <p v-else-if="player.settings.smartMix && player.preloadState === 'loading'" class="settings-row-note" role="status">正在准备 QQ 衔接方案</p>
              <p v-if="player.mixError" class="settings-row-note error" role="alert">{{ player.mixError }}</p>
              <div class="settings-row"><div class="settings-row-copy"><strong>下一首预加载</strong><p>在歌曲结束前准备下一首，减少切歌等待。</p></div><button class="settings-switch" :class="{ on: player.settings.preloadNext }" role="switch" :aria-checked="player.settings.preloadNext" :disabled="player.settings.smartMix" aria-label="下一首预加载" @click="player.updateSettings({ preloadNext: !player.settings.preloadNext })"><span></span></button></div>
              <p v-if="player.preloadError" class="settings-row-note error" role="status">下一首预加载失败：{{ player.preloadError }}。切歌时会重新读取音源。</p>
            </div>
          </div>

          <div class="settings-group">
            <h3>播放行为</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>关闭窗口后继续播放</strong><p>通过菜单栏或 Dock 重新打开主窗口。</p></div><button class="settings-switch" :class="{ on: player.settings.closeToTray }" role="switch" :aria-checked="player.settings.closeToTray" aria-label="关闭窗口后继续播放" @click="player.updateSettings({ closeToTray: !player.settings.closeToTray })"><span></span></button></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>定时停止</strong><p>{{ player.sleepAfterTrack ? '当前歌曲播完后停止' : player.sleepDeadline ? `剩余约 ${player.sleepRemaining} 分钟` : '到时自动暂停' }}</p></div><label class="settings-select"><select :value="player.sleepAfterTrack ? 'track' : player.sleepDeadline ? 'active' : 'off'" aria-label="定时停止" @change="sleep"><option value="off">关闭</option><option v-if="player.sleepDeadline" value="active">剩余 {{ player.sleepRemaining }} 分钟</option><option value="15">15 分钟</option><option value="30">30 分钟</option><option value="60">60 分钟</option><option value="120">120 分钟</option><option value="track">播完本首</option></select><ChevronDown :size="14" /></label></div>
            </div>
          </div>

          <div class="settings-group">
            <h3>桌面工具</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>迷你播放器</strong><p>置顶的小窗口，只同步当前歌曲和播放控制。</p></div><button class="button secondary settings-small-button" :disabled="!desktop" @click="emit('companion', 'mini')">打开</button></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>桌面歌词</strong><p>置顶显示当前歌词，可拖动位置。</p></div><button class="button secondary settings-small-button" :disabled="!desktop" @click="emit('companion', 'desktop-lyrics')">打开</button></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>下载与离线</strong><p>查看已下载歌曲和本机缓存。</p></div><button class="settings-link" @click="emit('downloads')">管理<Download :size="14" /></button></div>
            </div>
          </div>
        </template>

        <template v-else-if="tab === 'lyrics'">
          <div class="settings-preview" :style="{ '--preview-scale': player.settings.lyrics.fontScale / 100, '--preview-line': player.settings.lyrics.lineHeight }">
            <div class="settings-preview-head"><span>歌词预览</span><span>{{ player.settings.lyrics.fontScale }}%</span></div>
            <p class="settings-preview-line muted">沿着月光的方向</p>
            <p class="settings-preview-line active">寻找你留下的回响</p>
            <p class="settings-preview-line muted">让时间停在这一行</p>
          </div>
          <div class="settings-group">
            <h3>排版</h3>
            <div class="settings-card">
              <div class="settings-row settings-slider-row"><div class="settings-row-copy"><strong>歌词字号</strong><p>当前 {{ player.settings.lyrics.fontScale }}%</p></div><label class="settings-range"><input type="range" min="70" max="150" step="5" :value="player.settings.lyrics.fontScale" aria-label="歌词字号" :style="{ '--progress': `${((player.settings.lyrics.fontScale - 70) / 80) * 100}%` }" @input="lyrics({ fontScale: Number(($event.target as HTMLInputElement).value) })" /></label></div>
              <div class="settings-row settings-slider-row"><div class="settings-row-copy"><strong>歌词行距</strong><p>当前 {{ player.settings.lyrics.lineHeight.toFixed(1) }}</p></div><label class="settings-range"><input type="range" min="1.2" max="2.2" step="0.1" :value="player.settings.lyrics.lineHeight" aria-label="歌词行距" :style="{ '--progress': `${((player.settings.lyrics.lineHeight - 1.2) / 1) * 100}%` }" @input="lyrics({ lineHeight: Number(($event.target as HTMLInputElement).value) })" /></label></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>显示译文</strong><p>在歌词下方显示 QQ 音乐提供的翻译。</p></div><button class="settings-switch" :class="{ on: player.settings.lyrics.showTranslation }" role="switch" :aria-checked="player.settings.lyrics.showTranslation" aria-label="显示译文" @click="lyrics({ showTranslation: !player.settings.lyrics.showTranslation })"><span></span></button></div>
              <div class="settings-row"><div class="settings-row-copy"><strong>显示音译</strong><p>显示 QQ 音乐提供的罗马音或音译。</p></div><button class="settings-switch" :class="{ on: player.settings.lyrics.showRomanization }" role="switch" :aria-checked="player.settings.lyrics.showRomanization" aria-label="显示音译" @click="lyrics({ showRomanization: !player.settings.lyrics.showRomanization })"><span></span></button></div>
            </div>
          </div>
          <div class="settings-group">
            <h3>时间轴</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>全局时间微调</strong><p>正值提前，负值延后；应用到所有歌曲。</p></div><div class="settings-number"><input type="number" min="-5000" max="5000" step="100" :value="player.settings.lyricOffset" aria-label="全局歌词偏移毫秒" @change="player.updateSettings({ lyricOffset: Number(($event.target as HTMLInputElement).value) })" /><span>ms</span><button class="settings-link" :disabled="!player.settings.lyricOffset" aria-label="重置全局歌词偏移" @click="resetGlobalOffset"><RotateCcw :size="14" /></button></div></div>
              <div v-if="player.current" class="settings-row"><div class="settings-row-copy"><strong>本首额外微调</strong><p>{{ player.current.title }}</p></div><div class="settings-number"><input type="number" min="-5000" max="5000" step="100" :value="player.trackOffset" aria-label="本首歌词偏移毫秒" @change="player.updateTrackOffset(Number(($event.target as HTMLInputElement).value))" /><span>ms</span><button class="settings-link" :disabled="!player.trackOffset" aria-label="重置本首歌词偏移" @click="resetTrackOffset"><RotateCcw :size="14" /></button></div></div>
            </div>
            <p v-if="player.trackOffsetError" class="settings-note error" role="alert">{{ player.trackOffsetError }}</p>
          </div>
        </template>

        <template v-else-if="tab === 'shortcuts'">
          <div class="settings-group">
            <h3>全局快捷键</h3>
            <div class="settings-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>启用全局快捷键</strong><p>在其他应用中也能控制播放与显示主窗口。</p></div><button class="settings-switch" :class="{ on: shortcutDraft.enabled }" role="switch" :aria-checked="shortcutDraft.enabled" aria-label="启用全局快捷键" @click="shortcutDraft.enabled = !shortcutDraft.enabled; shortcutSaved = false"><span></span></button></div>
              <div v-for="action in shortcutActions" :key="action.id" class="settings-row settings-shortcut-row"><div class="settings-row-copy"><strong>{{ action.name }}</strong><p>点击右侧按键框，然后按下组合键。</p></div><input class="settings-shortcut-input" readonly :disabled="!shortcutDraft.enabled || shortcutBusy" :aria-label="`${action.name}全局快捷键`" :value="shortcutText(shortcutDraft[action.id])" @keydown="capture($event, action.id)" /></div>
            </div>
          </div>
          <p class="settings-note">每个快捷键需要至少两个修饰键，且不能与其他快捷键重复。</p>
          <p v-if="shortcutError" class="settings-note error" role="alert">{{ shortcutError }}</p>
          <p v-if="shortcutSaved" class="settings-note success" role="status">快捷键已保存</p>
          <div class="settings-actions"><button class="button secondary" :disabled="!desktop || shortcutBusy" @click="resetShortcuts"><RotateCcw :size="14" />恢复默认</button><button class="button primary" :disabled="!desktop || shortcutBusy" @click="saveShortcuts"><LoaderCircle v-if="shortcutBusy" :size="15" class="spinning" />应用快捷键</button></div>
        </template>

        <template v-else>
          <template v-if="library.profile">
            <div v-if="storage" class="settings-storage-grid">
              <div class="settings-storage-card"><span class="settings-storage-icon"><Captions :size="17" /></span><strong>{{ bytes(storage.cacheBytes) }}</strong><span>歌词缓存 · {{ storage.cacheCount }} 首</span></div>
              <div class="settings-storage-card"><span class="settings-storage-icon"><Download :size="17" /></span><strong>{{ bytes(storage.downloadBytes) }}</strong><button class="settings-inline-link" @click="emit('downloads')">已下载 · {{ storage.downloadCount }} 首</button></div>
              <div class="settings-storage-card"><span class="settings-storage-icon"><HardDrive :size="17" /></span><strong>{{ bytes(storage.databaseBytes) }}</strong><span>本机资料文件</span></div>
            </div>
            <p v-if="storageError" class="settings-note error" role="alert">{{ storageError }}</p>
            <div class="settings-group">
              <h3>缓存</h3>
              <div class="settings-card">
                <div class="settings-row"><div class="settings-row-copy"><strong>歌词缓存上限</strong><p>达到上限后优先清理较早的歌词。</p></div><label class="settings-select"><select :value="player.settings.cacheLimitMb" aria-label="歌词缓存上限" @change="setCacheLimit"><option v-for="size in [16,32,64,128,256,512]" :key="size" :value="size">{{ size }} MB</option></select><ChevronDown :size="14" /></label></div>
                <div class="settings-row"><div class="settings-row-copy"><strong>清理歌词缓存</strong><p>不会移除下载的歌曲或账户收藏。</p></div><button class="button secondary settings-small-button" :disabled="clearing || !storage?.cacheCount" @click="clearCache"><LoaderCircle v-if="clearing" :size="15" class="spinning" />清理</button></div>
              </div>
            </div>
          </template>
          <p v-else class="settings-empty">连接 QQ 音乐后查看本机存储。</p>

          <div class="settings-group">
            <h3>应用更新</h3>
            <div class="settings-card settings-update-card">
              <div class="settings-row"><div class="settings-row-copy"><strong>Listener {{ packageInfo.version }}</strong><p>macOS · Apple Silicon</p></div><button class="button secondary settings-small-button" :disabled="!desktop || updateState === 'checking' || updateState === 'installing'" @click="checkUpdate"><RefreshCw :size="14" :class="{ spinning: updateState === 'checking' }" />{{ updateState === 'checking' ? '检查中' : '检查更新' }}</button></div>
              <p v-if="updateState === 'latest'" class="settings-note success" role="status">已是最新版本</p>
              <div v-if="update" class="settings-update-available"><p>可更新至 <strong>{{ update.version }}</strong></p><p v-if="update.notes" class="settings-update-notes">{{ update.notes }}</p><button class="button primary settings-small-button" :disabled="updateState === 'installing'" @click="installUpdate"><Download :size="14" />安装并重启</button></div>
              <p v-if="updateState === 'installing'" class="settings-note" role="status">{{ received ? `正在下载 ${bytes(received)}${total ? ` / ${bytes(total)}` : ''}` : '正在准备更新' }}</p>
              <p v-if="updateError" class="settings-note error" role="alert">{{ updateError }}</p>
            </div>
          </div>
        </template>
      </div>
    </section>
  </div>
</template>
