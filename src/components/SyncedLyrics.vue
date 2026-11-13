<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { LayoutReason, LyricPlayer, type LyricLine, type LyricLineMouseEvent } from '@applemusic-like-lyrics/core'
import { tuneWordEmphasis } from '../lib/lyric-emphasis'

class FrameSyncedLyricPlayer extends LyricPlayer {
  private scrollLayoutPending = false
  private hasMeasuredViewport = false
  private browsing = false
  scrollPosition = 0
  scrollRange = 0

  constructor() {
    super()
    // AMLL owns the scroll position and bounds; reuse them without reading DOM geometry.
    const updateBoundary = this.scrollEngine.updateBoundary.bind(this.scrollEngine)
    this.scrollEngine.updateBoundary = (min, max) => {
      const offset = updateBoundary(min, max)
      this.scrollPosition = offset - min
      this.scrollRange = max - min
      return offset
    }
  }

  override calcLayout(reason: LayoutReason) {
    // AMLL treats wheel input differently from touch. Keep the same clear view
    // for both while Listener owns the return to playback.
    if (this.browsing) this.scrollState.isTouchScrolled = true
    if (reason === LayoutReason.ContinuousScroll || reason === LayoutReason.DiscreteScroll) {
      this.scrollLayoutPending = true
      return
    }
    // The whole lyrics page already animates in. Start the lines at their
    // intended position so the first gesture doesn't jump past that entrance.
    if (reason === LayoutReason.RebuildView) {
      super.calcLayout(LayoutReason.ContinuousScroll)
      return
    }
    if (reason === LayoutReason.Resize && this.size[1] > 0) {
      const snap = !this.hasMeasuredViewport || this.scrollState.isAutoAlignSuspended
      this.hasMeasuredViewport = true
      if (snap) {
        super.calcLayout(LayoutReason.ContinuousScroll)
        return
      }
    }
    super.calcLayout(reason)
  }

  flushScrollLayout() {
    if (!this.scrollLayoutPending) return false
    this.scrollLayoutPending = false
    // The engine has already accumulated all input deltas. Follow that position
    // directly instead of starting a new spring for every trackpad event.
    super.calcLayout(LayoutReason.ContinuousScroll)
    return true
  }

  setBrowsing(browsing: boolean) {
    this.browsing = browsing
    if (browsing) {
      this.scrollState.isAutoAlignSuspended = true
      this.scrollState.isTouchScrolled = true
    }
  }

  isBlurred(line: LyricLineMouseEvent['line']) {
    return this.currentLyricGroups.some(group => group.mainLine === line && group.blur > 0)
  }

  protected override buildLyricGroups() {
    super.buildLyricGroups()
    for (const group of this.currentLyricGroups) {
      group.element.classList.add('listener-lyric-row')
      if (this._getIsNonDynamic()) {
        const setTransform = group.setTransform.bind(group)
        group.setTransform = (top, immediate, delay, active, opacity, blur) =>
          setTransform(top, immediate, delay, active, opacity === .2 ? .43 : opacity, blur)
        continue
      }
      for (const line of [group.mainLine, group.bgLine]) {
        if (!line) continue
        const rebuild = line.rebuildElement.bind(line)
        line.rebuildElement = () => {
          rebuild()
          tuneWordEmphasis(line.getElement(), line.getLine())
        }
      }
    }
  }

  override resetScroll() {
    if (this.browsing) return
    this.scrollLayoutPending = false
    super.resetScroll()
  }
}

const props = defineProps<{
  lines: LyricLine[]
  playing: boolean
  active: boolean
  reducedMotion: boolean
  readTime: () => number
  lineHeight?: number
  fontScale?: number
  seekRevision?: number
}>()
const emit = defineEmits<{ lineClick: [event: LyricLineMouseEvent] }>()
const host = ref<HTMLDivElement>()
const scrollbar = ref<HTMLDivElement>()
const scrollThumb = ref<HTMLDivElement>()
// Keep the renderer outside Vue's reactive graph. Its animation state is owned
// by AMLL; only the explicit input props below need Vue watchers.
let renderer: FrameSyncedLyricPlayer
let scrollbarVisibleUntil = 0
const browsing = ref(false)
let touching = false
let returnToPlayback: { earliest: number; deadline: number; nextLine?: number } | null = null
function cancelReturn() { returnToPlayback = null }
function scheduleReturn(now = performance.now()) {
  if (!browsing.value || touching || !props.playing || !props.active) return
  returnToPlayback = { earliest: now + 2500, deadline: now + 5000 }
}
function wheel(event: WheelEvent) {
  if (!event.deltaY) return
  setBrowsing(true)
  scheduleReturn()
}
function browseByTouch() {
  touching = true
  setBrowsing(true)
  cancelReturn()
}
function finishTouch(event: TouchEvent) {
  if (!touching || event.touches.length) return
  touching = false
  scheduleReturn()
}
function setBrowsing(value: boolean) {
  if (browsing.value === value) return
  browsing.value = value
  renderer.setBrowsing(value)
}
function finishBrowsing() {
  touching = false
  cancelReturn()
  setBrowsing(false)
  hideScrollbar()
}
function shouldReturn(now: number, time: number) {
  const pending = returnToPlayback
  if (!pending || now < pending.earliest) return false
  if (now >= pending.deadline) return true
  if (pending.nextLine === undefined) {
    pending.nextLine = props.lines.find(line => !line.isBG && line.startTime > time)?.startTime ?? Infinity
  }
  return time >= pending.nextLine
}
function hideScrollbar() {
  scrollbarVisibleUntil = 0
  scrollbar.value!.classList.remove('is-visible')
}
function updateScrollbar(now: number) {
  if (renderer.flushScrollLayout() && renderer.scrollRange > 0) {
    const height = renderer.size[1]
    const thumbHeight = Math.min(height, Math.max(24, height * height / (height + renderer.scrollRange)))
    const top = (height - thumbHeight) * renderer.scrollPosition / renderer.scrollRange
    scrollThumb.value!.style.height = `${thumbHeight}px`
    scrollThumb.value!.style.transform = `translateY(${top}px)`
    if (!scrollbarVisibleUntil) scrollbar.value!.classList.add('is-visible')
    scrollbarVisibleUntil = now + 650
  } else if (scrollbarVisibleUntil && now >= scrollbarVisibleUntil) {
    hideScrollbar()
  }
}
function lineClick(event: Event) {
  const click = event as LyricLineMouseEvent
  if (renderer.isBlurred(click.line)) {
    setBrowsing(true)
    renderer.calcLayout(LayoutReason.ConfigChange)
    scheduleReturn()
    return
  }
  emit('lineClick', click)
}
function syncTo(time: number) {
  finishBrowsing()
  renderer.resetScroll()
  renderer.setCurrentTime(time, true)
  renderer.calcLayout(LayoutReason.ConfigChange)
}
onMounted(() => {
  renderer = new FrameSyncedLyricPlayer()
  host.value!.appendChild(renderer.getElement())
  renderer.setAlignPosition(0.42)
  renderer.setWordFadeWidth(0.5)
  watch(() => [props.lineHeight, props.fontScale], () => {
    renderer.getElement().style.lineHeight = String(props.lineHeight ?? 1.5)
    renderer.calcLayout(LayoutReason.ConfigChange)
  }, { immediate: true, flush: 'post' })
  renderer.addEventListener('line-click', lineClick)
  watch(() => props.lines, lines => {
    finishBrowsing()
    renderer.setLyricLines(lines, props.readTime())
  }, { immediate: true })
  watch(() => props.playing, playing => {
    if (playing) renderer.resume()
    else renderer.pause()
    if (!playing) cancelReturn()
    else scheduleReturn()
  }, { immediate: true })
  watch(() => props.reducedMotion, reduced => {
    renderer.setEnableSpring(!reduced)
    renderer.setEnableScale(!reduced)
    renderer.setEnableBlur(!reduced)
  }, { immediate: true })
  watch(() => props.seekRevision, () => syncTo(props.readTime()))
  watch(() => props.active, (active, _previous, cleanup) => {
    if (!active) {
      if (browsing.value) {
        finishBrowsing()
        renderer.resetScroll()
      } else hideScrollbar()
      return
    }
    renderer.setCurrentTime(props.readTime(), true)
    let frame = 0
    let previous = performance.now()
    const draw = (now: number) => {
      const time = props.readTime()
      const recenter = shouldReturn(now, time)
      if (recenter) {
        finishBrowsing()
        renderer.resetScroll()
      }
      renderer.setCurrentTime(time)
      if (recenter) renderer.calcLayout(LayoutReason.ConfigChange)
      updateScrollbar(now)
      renderer.update(now - previous)
      previous = now
      frame = requestAnimationFrame(draw)
    }
    frame = requestAnimationFrame(draw)
    cleanup(() => cancelAnimationFrame(frame))
  }, { immediate: true })
})
onUnmounted(() => {
  cancelReturn()
  renderer.removeEventListener('line-click', lineClick)
  renderer.dispose()
})
</script>

<template>
  <div ref="host" class="synced-lyrics" :class="{ 'is-browsing': browsing }" @wheel.capture.passive="wheel" @touchmove.capture.passive="browseByTouch" @touchend="finishTouch" @touchcancel="finishTouch">
    <div ref="scrollbar" class="lyric-scrollbar" aria-hidden="true"><div ref="scrollThumb" class="lyric-scrollbar-thumb"></div></div>
  </div>
</template>

<style scoped>
.synced-lyrics { position: relative; }
.synced-lyrics :deep(.listener-lyric-row [class*="_lyricLine"]:not([class*="_lyricLineWrapper"], [class*="_gradientMask"])) {
  --bright-mask-alpha: .43;
  --dark-mask-alpha: .43;
}
.synced-lyrics :deep(.listener-lyric-row [class*="_gradientMask"]) {
  --bright-mask-alpha: 1;
  --dark-mask-alpha: .41;
}
.synced-lyrics :deep(.listener-lyric-row [class*="_lyricSubLine"]) { opacity: .5; }
.synced-lyrics :deep(.listener-lyric-row [class*="_gradientMask"] > [class*="_lyricSubLine"]) { opacity: .76; }
@media (hover: hover) {
  .synced-lyrics.is-browsing :deep(.listener-lyric-row:hover:not(:has(> [class*="_gradientMask"]))) {
    opacity: 1 !important;
  }
  .synced-lyrics.is-browsing :deep(.listener-lyric-row:hover [class*="_lyricLine"]:not([class*="_lyricLineWrapper"], [class*="_gradientMask"])) {
    --bright-mask-alpha: 1;
    --dark-mask-alpha: 1;
  }
  .synced-lyrics.is-browsing :deep(.listener-lyric-row:hover [class*="_lyricSubLine"]) { opacity: .76; }
}
.lyric-scrollbar {
  position: absolute;
  z-index: 1;
  inset-block: 0;
  inset-inline-end: -8px;
  width: 4px;
  pointer-events: none;
  opacity: 0;
  transition: opacity 180ms ease;
}
.lyric-scrollbar.is-visible { opacity: 1; }
.lyric-scrollbar-thumb { width: 100%; border-radius: 999px; background: #ffffff80; }
@media (prefers-reduced-motion: reduce) {
  .lyric-scrollbar { transition: none; }
}
</style>
