<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch, nextTick } from 'vue'
import { AudioLines, Heart, Play, Pause, SkipBack, SkipForward, ListMusic, Volume2, VolumeX, ChevronDown, ArrowUpRight, ChevronRight, ArrowLeft, X, Search, Shuffle, Repeat, Settings2, Check, Disc3, Plus, Sun, Moon, MoreHorizontal, Download, SlidersHorizontal } from '@lucide/vue'
import TrackTable from './TrackTable.vue'
import AlbumCards from './AlbumCards.vue'
import PlaylistCards from './PlaylistCards.vue'
import SyncedLyrics from '../../src/components/SyncedLyrics.vue'
import '@applemusic-like-lyrics/core/style.css'
import data from '../../src/demo/data.json'
import type { LyricLine } from '@applemusic-like-lyrics/core'

const imageFiles = import.meta.glob<string>('../../src/demo/assets/*.jpg', { eager: true, query: '?url', import: 'default' })
const image = (file: string) => imageFiles[`../../src/demo/assets/${file}`]!
const albums = data.albums.map((album, index) => ({ ...album, id: index, image: image(album.cover), tracks: album.tracks.map(track => ({ ...track, cover: image(track.cover), key: track.mid })) }))
const tracks = albums.flatMap(album => album.tracks)
const artists = data.artists.map(artist => ({ ...artist, image: image(artist.image), albums: albums.filter(album => album.artist === artist.name) }))
const favorites = ref(new Set(tracks.filter((_, index) => index % 3 === 0).map(track => track.key)))
const savedAlbums = ref(new Set([0, 3, 5, 9]))
const savedArtists = ref(new Set(['周杰伦', '孙燕姿']))
const theme = ref<'light' | 'dark'>('light')
const navigation = ref('recommend')
const navItems = [{ id: 'recommend', label: '推荐' }, { id: 'discover', label: '发现' }, { id: 'favorites', label: '我的收藏' }]
const navIndex = computed(() => navItems.findIndex(item => item.id === navigation.value))
type Category = 'songs' | 'playlists' | 'albums' | 'artists'
const category = ref<Category>('albums')
const savedPlaylists = ref(new Set(['afternoon', 'familiar']))
const filter = ref('all')
const reverseOrder = ref(false)
const ordered = <T,>(items: T[]) => reverseOrder.value ? [...items].reverse() : items
const search = ref('')
const searchFocused = ref(false)
const searchField = ref<HTMLInputElement>()
const detail = ref<{ kind: 'playlist' | 'album' | 'artist'; id: string | number } | null>(null)
const pageScroll = ref<HTMLElement>()
watch([navigation, detail, category], () => { pageScroll.value!.scrollTop = 0 }, { flush: 'post' })
const focus = ref(false)
const queueOpen = ref(false)
const settingsOpen = ref(false)
const settingsPanel = ref<HTMLElement>()
watch(settingsOpen, async open => { if (open) { await nextTick(); settingsPanel.value!.querySelector<HTMLButtonElement>('button')!.focus() } })
function settingsKeyboard(event: KeyboardEvent) { if (event.key !== 'Tab') return; const fields = settingsPanel.value!.querySelectorAll<HTMLElement>('button,input'); const first = fields[0]!, last = fields[fields.length - 1]!; if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus() } else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus() } }
const playing = ref(false)
const selectedTrack = ref<string | null>(null)
const shuffle = ref(false)
const repeat = ref(false)
const current = ref(tracks.find(track => track.title === '晴天')!)
const queue = ref([current.value, tracks.find(track => track.title === '逆光')!, tracks.find(track => track.title === 'Blinding Lights')!, tracks.find(track => track.title === 'One Call Away')!, tracks.find(track => track.title === '天空')!])
const elapsed = ref(81)
let anchorSeconds = 81
let anchorWall = performance.now()
const readTime = () => Math.round(Math.min(current.value.duration, anchorSeconds + (playing.value ? (performance.now() - anchorWall) / 1000 : 0)) * 1000)
function setPosition(seconds: number) { elapsed.value = seconds; anchorSeconds = seconds; anchorWall = performance.now() }
watch(playing, value => { if (!value) { anchorSeconds += (performance.now() - anchorWall) / 1000; elapsed.value = anchorSeconds } anchorWall = performance.now() }, { flush: 'sync' })
const volume = ref(70)
const lastVolume = ref(70)
const lyricFont = ref(100)
const lyricSize = computed(() => `${Math.round(42 * lyricFont.value / 100)}px`)
const reducedMotion = ref(false)
const motionPreference = window.matchMedia('(prefers-reduced-motion: reduce)')
const updateMotion = () => { reducedMotion.value = motionPreference.matches }
const pointer = (event: PointerEvent) => { const target = event.currentTarget as HTMLElement; const box = target.getBoundingClientRect(); target.style.setProperty('--pointer-x', `${event.clientX - box.left}px`) }
const formatTime = (seconds: number) => `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, '0')}`
const albumForTrack = (key: string) => albums.find(album => album.tracks.some(track => track.key === key))!
const likedTracks = computed(() => tracks.filter(track => favorites.value.has(track.key)))
const catalogPlaylists = [
  { id: 'afternoon', name: '华语流行', image: image('album-1-1.jpg'), kind: 'catalog', tracks: [...albums[4]!.tracks, ...albums[3]!.tracks].slice(0, 18) },
  { id: 'city', name: '欧美流行', image: image('album-2-0.jpg'), kind: 'catalog', tracks: [...albums[5]!.tracks, ...albums[6]!.tracks] },
  { id: 'familiar', name: '华语经典', image: image('album-4-0.jpg'), kind: 'catalog', tracks: [...albums[9]!.tracks, ...albums[10]!.tracks] },
  { id: 'onrepeat', name: '欧美流行精选', image: image('album-3-1.jpg'), kind: 'catalog', tracks: [...albums[7]!.tracks, ...albums[8]!.tracks].slice(0, 23) },
]
const createdPlaylists = [
  { id: 'work', name: '工作', image: image('album-3-0.jpg'), kind: 'created', tracks: albums[7]!.tracks.slice(0, 8) },
  { id: 'commute', name: '通勤', image: image('album-0-0.jpg'), kind: 'created', tracks: albums[0]!.tracks.slice(0, 8) },
]
const personalPlaylists = computed(() => [...createdPlaylists, ...catalogPlaylists.filter(item => savedPlaylists.value.has(item.id))])
const spotlight = catalogPlaylists[0]!
const recommendedAlbums = [albums[2]!, albums[4]!, albums[5]!, albums[8]!, albums[9]!]
const recent = [current.value, tracks.find(track => track.title === '逆光')!, tracks.find(track => track.title === 'Blinding Lights')!, tracks.find(track => track.title === 'One Call Away')!, tracks.find(track => track.title === '天空')!, tracks.find(track => track.title === '愛情轉移')!]
const isSearching = computed(() => !!search.value.trim())
const matches = (text: string) => text.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase())
const scopedTracks = computed(() => navigation.value === 'favorites' ? likedTracks.value : tracks)
const scopedPlaylists = computed(() => navigation.value === 'favorites' ? personalPlaylists.value : catalogPlaylists)
const shownTracks = computed(() => ordered(scopedTracks.value.filter(track => matches(`${track.title} ${track.artist} ${track.album}`))))
const shownPlaylists = computed(() => ordered(scopedPlaylists.value.filter(item => (navigation.value !== 'favorites' || isSearching.value || filter.value === 'all' || (filter.value === 'created' ? item.kind === 'created' : savedPlaylists.value.has(item.id))) && matches(item.name))))
const shownAlbums = computed(() => albums.filter(item => (navigation.value !== 'favorites' || savedAlbums.value.has(item.id)) && matches(`${item.name} ${item.artist}`)))
const shownArtists = computed(() => ordered(artists.filter(item => (navigation.value !== 'favorites' || savedArtists.value.has(item.name)) && matches(item.name))))
const searchTracks = computed(() => shownTracks.value)
const selectedAlbum = computed(() => detail.value?.kind === 'album' ? albums.find(album => album.id === detail.value!.id)! : null)
const selectedArtist = computed(() => detail.value?.kind === 'artist' ? artists.find(artist => artist.name === detail.value!.id)! : null)
const selectedPlaylist = computed(() => detail.value?.kind === 'playlist' ? detail.value.id === 'recent' ? { id: 'recent', name: '最近在听', image: recent[0]!.cover, kind: 'recent', tracks: recent } : [...catalogPlaylists, ...createdPlaylists].find(playlist => playlist.id === detail.value!.id)! : null)
const detailTitle = computed(() => selectedAlbum.value?.name ?? selectedArtist.value?.name ?? selectedPlaylist.value?.name ?? '')
const detailImage = computed(() => selectedAlbum.value?.image ?? selectedArtist.value?.image ?? selectedPlaylist.value?.image ?? '')
const detailTracks = computed(() => selectedAlbum.value?.tracks ?? selectedArtist.value?.albums.flatMap(album => album.tracks) ?? selectedPlaylist.value?.tracks ?? [])
const detailSubtitle = computed(() => selectedAlbum.value ? `${selectedAlbum.value.artist} · ${selectedAlbum.value.year}` : selectedArtist.value ? selectedArtist.value.subtitle : selectedPlaylist.value?.kind === 'created' ? '我创建的歌单' : selectedPlaylist.value?.kind === 'recent' ? '最近播放' : '精选歌单')
const title = computed(() => navItems.find(item => item.id === navigation.value)!.label)
const categories = computed(() => [
  { id: 'songs' as const, label: navigation.value === 'favorites' ? '喜欢的歌曲' : '歌曲', count: scopedTracks.value.length },
  { id: 'albums' as const, label: '专辑', count: navigation.value === 'favorites' ? savedAlbums.value.size : albums.length },
  { id: 'artists' as const, label: '歌手', count: navigation.value === 'favorites' ? savedArtists.value.size : artists.length },
  { id: 'playlists' as const, label: '歌单', count: scopedPlaylists.value.length },
])
const collectionCount = computed(() => category.value === 'songs' ? shownTracks.value.length : category.value === 'albums' ? shownAlbums.value.length : category.value === 'artists' ? shownArtists.value.length : shownPlaylists.value.length)
const collectionStats = computed(() => `${collectionCount.value} ${category.value === 'songs' ? '首歌曲' : category.value === 'albums' ? '张专辑' : category.value === 'artists' ? '位歌手' : '张歌单'}`)
const displayAlbums = computed(() => ordered(navigation.value === 'discover' ? [...shownAlbums.value].reverse() : shownAlbums.value))
const announcements = ref('')
let toastTimer: ReturnType<typeof setTimeout>
function announce(message: string) { announcements.value = message; clearTimeout(toastTimer); toastTimer = setTimeout(() => { announcements.value = '' }, 2600) }
function selectNav(id: string) { navigation.value = id; detail.value = null; search.value = ''; category.value = id === 'favorites' ? 'songs' : 'albums'; filter.value = 'all'; reverseOrder.value = false }
function setCategory(value: Category) { category.value = value; detail.value = null }
function play(track = current.value, list = queue.value) { selectedTrack.value = track.key; current.value = track; queue.value = list; setPosition(0); playing.value = true }
function toggleTrack(track: typeof current.value, list: typeof tracks) { selectedTrack.value = track.key; if (current.value.key === track.key) { queue.value = list; playing.value = !playing.value } else play(track, list) }
function playAll(list: typeof tracks, random = false) { if (!list.length) return; shuffle.value = random; play(list[random ? Math.floor(Math.random() * list.length) : 0]!, list) }
function toggleLike(key: string) { const next = new Set(favorites.value); next.has(key) ? next.delete(key) : next.add(key); favorites.value = next }
function toggleAlbum(id: number) { const next = new Set(savedAlbums.value); next.has(id) ? next.delete(id) : next.add(id); savedAlbums.value = next }
function togglePlaylist(id: string) { const next = new Set(savedPlaylists.value); next.has(id) ? next.delete(id) : next.add(id); savedPlaylists.value = next }
function openPlaylist(id: string) { detail.value = { kind: 'playlist', id }; search.value = '' }
function toggleArtist(name: string) { const next = new Set(savedArtists.value); next.has(name) ? next.delete(name) : next.add(name); savedArtists.value = next }
function openAlbum(id: number) { detail.value = { kind: 'album', id }; search.value = '' }
function openArtist(name: string) { detail.value = { kind: 'artist', id: name }; search.value = ''; focus.value = false }
function next(step = 1) { const index = queue.value.findIndex(track => track.key === current.value.key); const nextIndex = shuffle.value && step > 0 ? (index + 1 + Math.floor(Math.random() * Math.max(1, queue.value.length - 1))) % queue.value.length : (index + step + queue.value.length) % queue.value.length; play(queue.value[nextIndex]!) }
function seek(event: Event) { setPosition(Number((event.target as HTMLInputElement).value)) }
function mute() { if (volume.value) { lastVolume.value = volume.value; volume.value = 0 } else volume.value = lastVolume.value }
function queueTrack(track: typeof current.value) { if (!queue.value.some(item => item.key === track.key)) queue.value = [...queue.value, track]; announce(`已加入队列 · ${track.title}`) }
const originalLines = ['晚风轻轻落在窗边', '把今天交给这一段旋律', '城市慢下来，灯光渐渐亮起', '让心事随着节拍轻轻摇摆', '此刻只想安静听一首歌', '时间在华语经典里停留', '沿着音符走过每一条街', '把温柔留给下一个明天']
const lyricLines = computed<LyricLine[]>(() => Array.from({ length: Math.ceil(current.value.duration / 8) }, (_, index) => { const text = originalLines[index % originalLines.length]!; const words = [...text]; const startTime = index * 8000; return { startTime, endTime: startTime + 7000, words: words.map((word, i) => ({ word, startTime: startTime + i * 6500 / words.length, endTime: startTime + (i + 1) * 6500 / words.length })), translatedLyric: '', romanLyric: '', isBG: false, isDuet: false } }))
let clock: ReturnType<typeof setInterval>
function keyboard(event: KeyboardEvent) { const target = event.target as HTMLElement; if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); focus.value = false; searchField.value!.focus(); return } if (event.key === 'Escape') { focus.value = false; queueOpen.value = false; settingsOpen.value = false; searchField.value!.blur(); return } if (target.closest('input,button,textarea') || event.metaKey || event.ctrlKey || event.altKey) return; if (event.code === 'Space') { event.preventDefault(); playing.value = !playing.value } if (event.key.toLowerCase() === 'l') focus.value = !focus.value }
onMounted(() => { updateMotion(); motionPreference.addEventListener('change', updateMotion); window.addEventListener('keydown', keyboard); clock = setInterval(() => { if (playing.value) { elapsed.value = readTime() / 1000; if (elapsed.value >= current.value.duration) { if (repeat.value) setPosition(0); else next() } } }, 100) })
onUnmounted(() => { clearInterval(clock); clearTimeout(toastTimer); window.removeEventListener('keydown', keyboard); motionPreference.removeEventListener('change', updateMotion) })
</script>

<template>
  <div class="listener-next" :data-theme="theme">
    <header class="app-header" :inert="settingsOpen || focus">
      <button class="wordmark" aria-label="返回推荐" @click="selectNav('recommend')"><AudioLines :size="25" :stroke-width="1.7"/><span>listener<span class="brand-period">.</span></span></button>
      <nav class="glass-navigation" aria-label="主导航" @pointermove="pointer">
        <span class="glass-lens" :style="{ transform: `translateX(${navIndex * 100}%)` }"></span>
        <button v-for="item in navItems" :key="item.id" :class="{ selected: navigation === item.id }" :aria-current="navigation === item.id ? 'page' : undefined" @click="selectNav(item.id)">{{ item.label }}</button>
      </nav>
      <div class="header-actions"><label class="search-field" :class="{ focused: searchFocused }"><Search :size="15"/><input ref="searchField" v-model="search" :aria-label="navigation === 'favorites' ? '搜索我的收藏' : '搜索全部音乐'" :placeholder="navigation === 'favorites' ? '搜索我的收藏' : '搜索音乐'" @focus="searchFocused = true; detail = null" @blur="searchFocused = false"/><kbd>⌘ K</kbd><button v-if="search" aria-label="清空搜索" @click="search = ''"><X :size="13"/></button></label><button class="icon-button theme-toggle" :aria-label="theme === 'light' ? '切换夜间模式' : '切换日间模式'" @click="theme = theme === 'light' ? 'dark' : 'light'"><Moon v-if="theme === 'light'" :size="19"/><Sun v-else :size="19"/></button><button class="avatar-button" aria-label="外观与预览设置" @click="settingsOpen = true">L</button></div>
    </header>

    <main ref="pageScroll" :inert="settingsOpen || focus" class="page-scroll" :class="{ 'detail-page': !!detail }">
      <div class="page-content">
        <template v-if="isSearching">
          <div class="page-title search-title"><div><h1>{{ navigation === 'favorites' ? '搜索我的收藏' : '搜索' }}</h1><p>“{{ search }}”</p></div><span class="design-badge">设计预览</span></div>
          <section v-if="searchTracks.length" class="search-results"><div class="section-topline"><h2>歌曲 <small>{{ searchTracks.length }}</small></h2></div><div v-for="track in searchTracks" :key="track.key" class="search-result" :class="{ selected: selectedTrack === track.key }" @click="selectedTrack = track.key" @dblclick="play(track, searchTracks)"><button class="search-track-play" :aria-label="`播放 ${track.title}`" @click.stop="toggleTrack(track, searchTracks)" @dblclick.stop><img :src="track.cover" alt=""/><span><Pause v-if="current.key === track.key && playing" :size="16" fill="currentColor"/><Play v-else :size="16" fill="currentColor"/></span></button><button class="search-track-name" @click.stop="selectedTrack = track.key" @dblclick.stop="play(track, searchTracks)" @keydown.enter.prevent.stop="play(track, searchTracks)"><strong>{{ track.title }}</strong><small>{{ track.artist }} · {{ track.album }}</small></button></div></section>
          <section v-if="shownArtists.length" class="search-results"><div class="section-topline"><h2>歌手</h2></div><div class="artist-grid compact"><button v-for="artist in shownArtists" :key="artist.name" class="artist-card" @click="openArtist(artist.name)"><img :src="artist.image" :alt="artist.name"/><strong>{{ artist.name }}</strong><small>{{ artist.subtitle }}</small></button></div></section>
          <section v-if="shownAlbums.length" class="search-results"><div class="section-topline"><h2>专辑 <small>{{ shownAlbums.length }}</small></h2></div><AlbumCards :albums="shownAlbums" :saved="savedAlbums" @open="openAlbum" @artist="openArtist" @save="toggleAlbum"/></section>
          <section v-if="shownPlaylists.length" class="search-results"><div class="section-topline"><h2>歌单 <small>{{ shownPlaylists.length }}</small></h2></div><PlaylistCards :playlists="shownPlaylists" :saved="savedPlaylists" @open="openPlaylist" @save="togglePlaylist"/></section>
          <div v-if="!searchTracks.length && !shownAlbums.length && !shownArtists.length && !shownPlaylists.length" class="empty-state"><Search :size="30"/><h2>没有找到相关音乐</h2><p>试试歌名、歌手或专辑名称</p></div>
        </template>

        <template v-else-if="detail">
          <button class="back-link" @click="detail = null"><ArrowLeft :size="15"/>返回{{ title }}</button>
          <section class="detail-hero"><div class="detail-art" :class="{ portrait: detail.kind === 'artist' }"><img :src="detailImage" :alt="detailTitle"/></div><div class="detail-copy"><span class="eyebrow">{{ detail.kind === 'album' ? '专辑' : detail.kind === 'artist' ? '歌手' : '歌单' }}</span><h1>{{ detailTitle }}</h1><button v-if="selectedAlbum" class="detail-artist" @click="openArtist(selectedAlbum.artist)">{{ selectedAlbum.artist }}<ArrowUpRight :size="15"/></button><p v-else>{{ detailSubtitle }}</p><p class="detail-meta">{{ selectedAlbum?.year }}<span v-if="selectedAlbum"> · </span>{{ detailTracks.length }} 首歌曲<span v-if="selectedArtist"> · {{ selectedArtist.albums.length }} 张专辑</span></p><div class="detail-actions"><button class="primary-button" :disabled="!detailTracks.length" @click="playAll(detailTracks)"><Play :size="15" fill="currentColor"/>播放全部</button><button class="outline-button" @click="playAll(detailTracks, true)"><Shuffle :size="16"/>随机播放</button><button v-if="selectedAlbum" class="round-button" :class="{ active: savedAlbums.has(selectedAlbum.id) }" :aria-label="savedAlbums.has(selectedAlbum.id) ? '取消收藏专辑' : '收藏专辑'" @click="toggleAlbum(selectedAlbum.id)"><Heart :size="19" :fill="savedAlbums.has(selectedAlbum.id) ? 'currentColor' : 'none'"/></button><button v-if="selectedArtist" class="round-button" :class="{ active: savedArtists.has(selectedArtist.name) }" :aria-label="savedArtists.has(selectedArtist.name) ? '取消收藏歌手' : '收藏歌手'" @click="toggleArtist(selectedArtist.name)"><Heart :size="19" :fill="savedArtists.has(selectedArtist.name) ? 'currentColor' : 'none'"/></button><button v-if="selectedPlaylist?.kind === 'catalog'" class="round-button" :class="{ active: savedPlaylists.has(selectedPlaylist.id) }" :aria-label="savedPlaylists.has(selectedPlaylist.id) ? '取消收藏歌单' : '收藏歌单'" @click="togglePlaylist(selectedPlaylist.id)"><Heart :size="19" :fill="savedPlaylists.has(selectedPlaylist.id) ? 'currentColor' : 'none'"/></button></div></div></section>
          <section v-if="selectedArtist" class="artist-discography"><div class="section-topline"><h2>发行专辑</h2><span>{{ selectedArtist.albums.length }} RELEASES</span></div><div class="record-grid artist-albums"><article v-for="album in selectedArtist.albums" :key="album.id" class="record-card"><button class="record-sleeve" :aria-label="`查看专辑 ${album.name}`" @click="openAlbum(album.id)"><img :src="album.image" :alt="album.name"/><span class="sleeve-play"><ArrowUpRight :size="20"/></span></button><div class="record-caption"><div><button @click="openAlbum(album.id)"><strong>{{ album.name }}</strong></button><small>{{ album.year }} · {{ album.tracks.length }} 首歌曲</small></div></div></article></div></section>
          <section class="track-section"><div class="section-topline"><h2>{{ selectedArtist ? '歌曲' : '曲目' }}<small>{{ detailTracks.length }}</small></h2><button class="subtle-button" @click="playAll(detailTracks, true)"><Shuffle :size="14"/>随机排列</button></div><TrackTable :tracks="detailTracks" :current-key="current.key" :selected-key="selectedTrack" :playing="playing" :favorites="favorites" @select="selectedTrack = $event" @play="play($event, detailTracks)" @toggle="toggleTrack($event, detailTracks)" @album="openAlbum(albumForTrack($event).id)" @like="toggleLike" @queue="queueTrack"/></section>
        </template>

        <template v-else>
          <div class="page-title">
            <div><h1>{{ title }}</h1><p v-if="navigation === 'favorites'">{{ likedTracks.length }} 首歌曲 · {{ savedAlbums.size }} 张专辑 · {{ savedArtists.size }} 位歌手</p><p v-else-if="navigation === 'discover'">{{ tracks.length }} 首歌曲 · {{ albums.length }} 张专辑 · {{ artists.length }} 位歌手</p></div>
            <span class="design-badge"><span></span>设计预览</span>
          </div>

          <template v-if="navigation === 'recommend'">
            <section class="listening-overview">
              <div class="favorite-feature recommendation-feature">
                <button class="favorite-open" :aria-label="`查看歌单 ${spotlight.name}`" @click="openPlaylist(spotlight.id)">
                  <span class="feature-label">精选歌单</span><h2>{{ spotlight.name }}</h2><p>{{ spotlight.tracks.length }} 首歌曲</p>
                  <span class="feature-covers"><img :src="image('album-1-0.jpg')" alt="逆光"/><img :src="image('album-0-0.jpg')" alt="七里香"/><img :src="spotlight.image" alt="克卜勒"/></span>
                </button>
                <button class="feature-play" :aria-label="`播放歌单 ${spotlight.name}`" @click="playAll(spotlight.tracks)"><Play :size="14" fill="currentColor"/>播放全部<ArrowUpRight :size="15"/></button>
              </div>
              <div class="recent-section">
                <div class="section-topline"><h2>最近播放<span class="small-dot"></span></h2><button class="subtle-button" aria-label="查看最近播放" @click="openPlaylist('recent')">查看全部<ArrowUpRight :size="14"/></button></div>
                <div class="recent-grid">
                  <div v-for="track in recent" :key="track.key" class="recent-item" :class="{ current: current.key === track.key, selected: selectedTrack === track.key }" @click="selectedTrack = track.key" @dblclick="play(track, recent)">
                    <button class="recent-cover" :aria-label="`${current.key === track.key && playing ? '暂停' : '播放'} ${track.title}`" @click.stop="toggleTrack(track, recent)" @dblclick.stop><img :src="track.cover" alt=""/><span class="recent-hover"><Pause v-if="current.key === track.key && playing" :size="14" fill="currentColor"/><Play v-else :size="14" fill="currentColor"/></span></button>
                    <button class="recent-text" @click.stop="selectedTrack = track.key" @dblclick.stop="play(track, recent)" @keydown.enter.prevent.stop="play(track, recent)"><strong>{{ track.title }}</strong><small>{{ track.artist }}</small></button>
                    <AudioLines v-if="current.key === track.key && playing" class="now-indicator" :size="17"/>
                  </div>
                </div>
              </div>
            </section>
            <section class="recommend-section"><div class="section-topline"><h2>专辑推荐</h2><button class="subtle-button" @click="selectNav('discover')">全部专辑<ArrowUpRight :size="14"/></button></div><AlbumCards :albums="recommendedAlbums" :saved="savedAlbums" @open="openAlbum" @artist="openArtist" @save="toggleAlbum"/></section>
            <section class="recommend-section"><div class="section-topline"><h2>精选歌单</h2><button class="subtle-button" @click="selectNav('discover'); setCategory('playlists')">全部歌单<ArrowUpRight :size="14"/></button></div><PlaylistCards :playlists="catalogPlaylists" :saved="savedPlaylists" @open="openPlaylist" @save="togglePlaylist"/></section>
          </template>

          <section v-else class="collection-section catalog-collection" :aria-label="navigation === 'favorites' ? '我的收藏内容' : '音乐目录'">
            <div class="collection-toolbar">
              <div class="collection-tabs" aria-label="音乐分类"><button v-for="item in categories" :key="item.id" :class="{ selected: category === item.id }" :aria-pressed="category === item.id" @click="setCategory(item.id)">{{ item.label }}<span>{{ item.count }}</span></button></div>
              <div class="collection-actions"><span>{{ collectionStats }}</span><button class="icon-button" aria-label="反转列表顺序" :aria-pressed="reverseOrder" @click="reverseOrder = !reverseOrder"><SlidersHorizontal :size="16"/></button></div>
            </div>
            <div v-if="category === 'playlists' && navigation === 'favorites'" class="collection-filters" aria-label="歌单筛选"><button v-for="item in [{ id: 'all', name: '全部' }, { id: 'created', name: '我创建的' }, { id: 'collected', name: '我收藏的' }]" :key="item.id" :class="{ selected: filter === item.id }" @click="filter = item.id">{{ item.name }}</button></div>
            <template v-if="collectionCount">
              <div v-if="category === 'songs'" class="catalog-songs">
                <div class="song-list-actions"><button class="primary-button" @click="playAll(shownTracks)"><Play :size="14" fill="currentColor"/>播放全部</button><button class="outline-button" @click="playAll(shownTracks, true)"><Shuffle :size="15"/>随机播放</button><span>{{ collectionStats }}</span></div>
                <TrackTable :tracks="shownTracks" :current-key="current.key" :selected-key="selectedTrack" :playing="playing" :favorites="favorites" @select="selectedTrack = $event" @play="play($event, shownTracks)" @toggle="toggleTrack($event, shownTracks)" @album="openAlbum(albumForTrack($event).id)" @like="toggleLike" @queue="queueTrack"/>
              </div>
              <PlaylistCards v-else-if="category === 'playlists'" :playlists="shownPlaylists" :saved="savedPlaylists" @open="openPlaylist" @save="togglePlaylist"/>
              <AlbumCards v-else-if="category === 'albums'" :albums="displayAlbums" :saved="savedAlbums" @open="openAlbum" @artist="openArtist" @save="toggleAlbum"/>
              <div v-else class="artist-grid"><article v-for="artist in shownArtists" :key="artist.name" class="artist-card"><button class="artist-image" :aria-label="`查看歌手 ${artist.name}`" @click="openArtist(artist.name)"><img :src="artist.image" :alt="artist.name"/><span><ArrowUpRight :size="20"/></span></button><div class="artist-caption"><div><button @click="openArtist(artist.name)"><strong>{{ artist.name }}</strong></button><small>{{ artist.subtitle }}</small></div><button class="icon-button" :class="{ loved: savedArtists.has(artist.name) }" :aria-label="`${savedArtists.has(artist.name) ? '取消收藏' : '收藏'} ${artist.name}`" @click="toggleArtist(artist.name)"><Heart :size="15" :fill="savedArtists.has(artist.name) ? 'currentColor' : 'none'"/></button></div></article></div>
            </template>
            <div v-else class="empty-state"><Heart :size="30"/><h2>{{ category === 'songs' ? '还没有喜欢的歌曲' : category === 'albums' ? '还没有收藏的专辑' : category === 'artists' ? '还没有收藏的歌手' : '暂无歌单' }}</h2><button class="outline-button" @click="selectNav('discover')">前往发现<ArrowUpRight :size="14"/></button></div>
          </section>
        </template>
      </div>
    </main>

    <footer :inert="settingsOpen || focus" class="player-dock" aria-label="播放控制">
      <input class="dock-progress" type="range" aria-label="播放进度" min="0" :max="current.duration" :value="elapsed" :style="{ '--progress': `${elapsed / current.duration * 100}%` }" @input="seek"/>
      <div class="dock-identity"><button class="dock-art" aria-label="打开歌词页面" @click="focus = true"><img :src="current.cover" :alt="current.album"/><span><ChevronDown :size="17"/></span></button><div class="dock-song"><button @click="focus = true"><strong>{{ current.title }}</strong></button><button @click="openArtist(current.artist)"><small>{{ current.artist }}</small></button></div><button class="icon-button dock-heart" :class="{ loved: favorites.has(current.key) }" :aria-label="favorites.has(current.key) ? '取消喜欢当前歌曲' : '喜欢当前歌曲'" @click="toggleLike(current.key)"><Heart :size="19" :fill="favorites.has(current.key) ? 'currentColor' : 'none'"/></button></div>
      <div class="dock-center"><div class="transport"><button class="icon-button minor" :class="{ active: shuffle }" aria-label="随机播放" :aria-pressed="shuffle" @click="shuffle = !shuffle"><Shuffle :size="17"/></button><button class="icon-button skip" aria-label="上一首" @click="next(-1)"><SkipBack :size="19" fill="currentColor"/></button><button class="play-button" :aria-label="playing ? '暂停' : '播放'" @click="playing = !playing"><Pause v-if="playing" :size="21" fill="currentColor"/><Play v-else :size="21" fill="currentColor"/></button><button class="icon-button skip" aria-label="下一首" @click="next()"><SkipForward :size="19" fill="currentColor"/></button><button class="icon-button minor" :class="{ active: repeat }" aria-label="单曲循环" :aria-pressed="repeat" @click="repeat = !repeat"><Repeat :size="17"/></button></div><span class="dock-time">{{ formatTime(elapsed) }}<span>/</span>{{ formatTime(current.duration) }}</span></div>
      <div class="dock-tools"><span class="playback-dot" :class="{ playing }"></span><button class="icon-button lyric-button" aria-label="展开歌词" @click="focus = true"><AudioLines :size="19"/></button><button class="icon-button" :class="{ active: queueOpen }" aria-label="播放队列" :aria-expanded="queueOpen" @click="queueOpen = !queueOpen"><ListMusic :size="21"/></button><span class="tool-divider"></span><button class="icon-button volume-button" aria-label="静音" @click="mute"><Volume2 v-if="volume" :size="18"/><VolumeX v-else :size="18"/></button><input class="volume-range" type="range" aria-label="音量" min="0" max="100" v-model.number="volume" :style="{ '--progress': `${volume}%` }"/></div>
    </footer>

    <Transition name="queue"><aside v-if="queueOpen" :inert="settingsOpen || focus" class="queue-panel" aria-label="播放队列"><div class="queue-header"><div><h2>接下来听 <small>{{ queue.length }}</small></h2></div><button class="icon-button" aria-label="关闭队列" @click="queueOpen = false"><X :size="18"/></button></div><div class="queue-list"><div v-for="(track, index) in queue" :key="track.key" class="queue-row" :class="{ current: track.key === current.key, selected: selectedTrack === track.key }" tabindex="0" @click="selectedTrack = track.key" @dblclick="play(track)" @keydown.enter.prevent.stop="play(track)"><button class="queue-index track-index" :aria-label="`${current.key === track.key && playing ? '暂停' : '播放'} ${track.title}`" @click.stop="toggleTrack(track, queue)" @dblclick.stop><span class="track-normal-icon"><AudioLines v-if="current.key === track.key && playing" :size="14"/><span v-else>{{ String(index + 1).padStart(2, '0') }}</span></span><span class="track-hover-icon"><Pause v-if="current.key === track.key && playing" :size="13" fill="currentColor"/><Play v-else :size="13" fill="currentColor"/></span></button><img :src="track.cover" alt=""/><span><strong>{{ track.title }}</strong><small>{{ track.artist }}</small></span><span class="queue-duration">{{ formatTime(track.duration) }}</span></div></div></aside></Transition>
    <Transition name="focus"><section v-if="focus" :inert="settingsOpen" class="focus-scene" aria-label="沉浸歌词页面"><div class="focus-ambient" :style="{ backgroundImage: `url('${current.cover}')` }"></div><header class="focus-header"><button class="glass-close" aria-label="关闭歌词页面" @click="focus = false"><ChevronDown :size="23"/></button><span class="focus-wordmark"><AudioLines :size="17"/>listener.</span><button class="glass-close" aria-label="歌词外观设置" @click="settingsOpen = true"><Settings2 :size="20"/></button></header><div class="focus-layout"><div class="focus-record"><div class="focus-cover"><img :src="current.cover" :alt="current.album"/></div><div class="focus-meta"><div><h1>{{ current.title }}</h1><button @click="openArtist(current.artist)">{{ current.artist }}<ArrowUpRight :size="14"/></button></div><button class="icon-button" :class="{ loved: favorites.has(current.key) }" aria-label="喜欢当前歌曲" @click="toggleLike(current.key)"><Heart :size="23" :fill="favorites.has(current.key) ? 'currentColor' : 'none'"/></button></div><div class="focus-timeline"><input type="range" aria-label="歌词页面播放进度" min="0" :max="current.duration" :value="elapsed" :style="{ '--progress': `${elapsed / current.duration * 100}%` }" @input="seek"/><div><time>{{ formatTime(elapsed) }}</time><time>{{ formatTime(current.duration) }}</time></div></div><div class="transport focus-transport"><button class="icon-button minor" :class="{ active: shuffle }" aria-label="歌词页随机播放" @click="shuffle = !shuffle"><Shuffle :size="20"/></button><button class="icon-button" aria-label="歌词页上一首" @click="next(-1)"><SkipBack :size="24" fill="currentColor"/></button><button class="play-button" :aria-label="playing ? '歌词页暂停' : '歌词页播放'" @click="playing = !playing"><Pause v-if="playing" :size="25" fill="currentColor"/><Play v-else :size="25" fill="currentColor"/></button><button class="icon-button" aria-label="歌词页下一首" @click="next()"><SkipForward :size="24" fill="currentColor"/></button><button class="icon-button minor" :class="{ active: repeat }" aria-label="歌词页单曲循环" @click="repeat = !repeat"><Repeat :size="20"/></button></div></div><div class="focus-lyric-pane" :style="{ '--lyric-size': lyricSize }"><SyncedLyrics :key="current.key" class="preview-lyrics" :lines="lyricLines" :playing="playing" :active="focus" :reduced-motion="reducedMotion" :read-time="readTime" :font-scale="lyricFont" :line-height="1.5" @line-click="event => { setPosition(event.line.getLine().startTime / 1000) }"/></div></div></section></Transition>
    <Transition name="modal"><div v-if="settingsOpen" class="modal-scrim" @click.self="settingsOpen = false"><section ref="settingsPanel" class="appearance-panel" @keydown="settingsKeyboard" role="dialog" aria-modal="true" aria-label="外观与预览设置"><div class="appearance-heading"><div><h2>外观与偏好</h2></div><button class="icon-button" aria-label="关闭外观设置" @click="settingsOpen = false"><X :size="19"/></button></div><div class="theme-options"><button :class="{ selected: theme === 'light' }" @click="theme = 'light'"><span class="theme-swatch light-swatch"><Sun :size="25"/></span><span>白昼 · 白与蓝<Check v-if="theme === 'light'" :size="15"/></span></button><button :class="{ selected: theme === 'dark' }" @click="theme = 'dark'"><span class="theme-swatch dark-swatch"><Moon :size="25"/></span><span>夜间 · 灰与橙<Check v-if="theme === 'dark'" :size="15"/></span></button></div><div class="appearance-row"><span>歌词字号</span><input type="range" aria-label="歌词字号" min="80" max="130" step="5" v-model.number="lyricFont"/><span>{{ lyricFont }}%</span></div><div class="preview-note"><AudioLines :size="18"/><p>设计预览，不连接 QQ 音乐。收藏仅保留到关闭页面。</p></div><button class="primary-button appearance-done" @click="settingsOpen = false">完成<ArrowUpRight :size="15"/></button></section></div></Transition>
    <Transition name="toast"><div v-if="announcements" class="toast-message" role="status"><Check :size="16"/>{{ announcements }}</div></Transition>
  </div>
</template>
