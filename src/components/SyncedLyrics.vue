<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { LyricPlayer, type LyricLine, type LyricLineMouseEvent } from '@applemusic-like-lyrics/core'

const props = defineProps<{
  lines: LyricLine[]
  playing: boolean
  active: boolean
  reducedMotion: boolean
  readTime: () => number
  lineHeight?: number
  fontScale?: number
}>()
const emit = defineEmits<{ lineClick: [event: LyricLineMouseEvent] }>()
const host = ref<HTMLDivElement>()
// Keep the renderer outside Vue's reactive graph. Its animation state is owned
// by AMLL; only the explicit input props below need Vue watchers.
let renderer: LyricPlayer
let browsing = false
let pointerInside = false
let returnToPlayback: { after: number; lyricTime: number } | null = null
function cancelReturn() { returnToPlayback = null }
function scheduleReturn() {
  if (!browsing || !props.playing || !props.active) return
  const time = props.readTime()
  // The final line has no successor; it can return after the same short delay.
  const next = props.lines.find(line => !line.isBG && line.startTime > time)
  returnToPlayback = { after: performance.now() + 800, lyricTime: next ? next.startTime : time }
}
function pointerEnter() { pointerInside = true; cancelReturn() }
function pointerLeave() { pointerInside = false; scheduleReturn() }
function browse() { browsing = true; cancelReturn() }
const lineClick = (event: Event) => emit('lineClick', event as LyricLineMouseEvent)
function syncTo(time: number) {
  if (!renderer) return
  browsing = false
  cancelReturn()
  renderer.setCurrentTime(time, true)
  renderer.resetScroll()
}
defineExpose({ syncTo })

onMounted(() => {
  renderer = new LyricPlayer()
  host.value!.appendChild(renderer.getElement())
  renderer.setAlignPosition(0.42)
  renderer.setWordFadeWidth(0.5)
  watch(() => [props.lineHeight, props.fontScale], () => {
    renderer.getElement().style.lineHeight = String(props.lineHeight ?? 1.5)
    renderer.calcLayout('config-change')
  }, { immediate: true, flush: 'post' })
  renderer.addEventListener('line-click', lineClick)
  watch(() => props.lines, lines => {
    browsing = false
    cancelReturn()
    renderer.setLyricLines(lines, props.readTime())
  }, { immediate: true })
  watch(() => props.playing, playing => {
    if (playing) renderer.resume()
    else renderer.pause()
    if (!playing) cancelReturn()
    else if (!pointerInside) scheduleReturn()
  }, { immediate: true })
  watch(() => props.reducedMotion, reduced => {
    renderer.setEnableSpring(!reduced)
    renderer.setEnableScale(!reduced)
    renderer.setEnableBlur(!reduced)
  }, { immediate: true })
  watch(() => props.active, (active, _previous, cleanup) => {
    if (!active) { cancelReturn(); return }
    renderer.setCurrentTime(props.readTime(), true)
    if (!pointerInside) scheduleReturn()
    let frame = 0
    let previous = performance.now()
    const draw = (now: number) => {
      const time = props.readTime()
      if (returnToPlayback && now >= returnToPlayback.after && time >= returnToPlayback.lyricTime) {
        renderer.resetScroll()
        returnToPlayback = null
        browsing = false
      }
      renderer.setCurrentTime(time)
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

<template><div ref="host" @pointerenter="pointerEnter" @pointerleave="pointerLeave" @wheel.capture.passive="browse" @touchmove.capture.passive="browse" @touchend="pointerLeave"></div></template>
