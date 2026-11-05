// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, reactive } from 'vue'
import SyncedLyrics from '../src/components/SyncedLyrics.vue'
import type { LyricLine } from '@applemusic-like-lyrics/core'

const state = vi.hoisted(() => ({ renderer: null as unknown as {
  setLyricLines: ReturnType<typeof vi.fn>
  setCurrentTime: ReturnType<typeof vi.fn>
  update: ReturnType<typeof vi.fn>
  pause: ReturnType<typeof vi.fn>
  resume: ReturnType<typeof vi.fn>
  dispose: ReturnType<typeof vi.fn>
  resetScroll: ReturnType<typeof vi.fn>
} }))
vi.mock('@applemusic-like-lyrics/core', () => ({
  LyricPlayer: class extends EventTarget {
    constructor() { super(); state.renderer = this }
    getElement() { return document.createElement('div') }
    calcLayout = vi.fn()
    setAlignPosition = vi.fn()
    setWordFadeWidth = vi.fn()
    setLyricLines = vi.fn()
    setCurrentTime = vi.fn()
    setEnableSpring = vi.fn()
    setEnableScale = vi.fn()
    setEnableBlur = vi.fn()
    update = vi.fn()
    pause = vi.fn()
    resume = vi.fn()
    dispose = vi.fn()
    resetScroll = vi.fn()
  },
}))

describe('lyric frame ownership', () => {
  let frames: Map<number, FrameRequestCallback>
  let frameId: number
  let unmount: (() => void) | undefined
  beforeEach(() => {
    frames = new Map()
    frameId = 0
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++frameId, callback); return frameId })
    vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
  })
  afterEach(() => { unmount?.(); unmount = undefined; vi.restoreAllMocks(); vi.unstubAllGlobals() })
  function draw(now: number) {
    const pending = [...frames.values()]
    frames.clear()
    pending.forEach(callback => callback(now))
  }
  function mount(playing = true) {
    const readTime = vi.fn(() => 42500)
    const props = reactive({ lines: [] as LyricLine[], playing, active: true, reducedMotion: false, readTime })
    const app = createApp({ render: () => h(SyncedLyrics, props) })
    const root = document.createElement('div')
    document.body.append(root)
    app.mount(root)
    unmount = () => { app.unmount(); root.remove() }
    return { props, readTime, renderer: state.renderer, host: root.firstElementChild! }
  }
  it('reads the live clock immediately before each frame, independent of reactive UI time', () => {
    const { readTime, renderer } = mount()
    renderer.setCurrentTime.mockClear()
    renderer.update.mockClear()
    for (let frame = 1; frame <= 60; frame++) {
      const time = 42500 + Math.round(frame * 1000 / 60)
      readTime.mockReturnValue(time)
      draw(performance.now() + frame * 1000 / 60)
      expect(renderer.setCurrentTime).toHaveBeenLastCalledWith(time)
      expect(renderer.setCurrentTime.mock.invocationCallOrder.at(-1)).toBeLessThan(renderer.update.mock.invocationCallOrder.at(-1)!)
    }
    expect(renderer.update).toHaveBeenCalledTimes(60)
    expect(frames.size).toBe(1)
  })
  it('keeps paused lyrics laid out, then cancels all frames when hidden or unmounted', async () => {
    const { props, renderer } = mount(false)
    expect(renderer.pause).toHaveBeenCalledOnce()
    draw(performance.now() + 17)
    expect(renderer.update).toHaveBeenCalledOnce()
    props.active = false
    await nextTick()
    expect(frames.size).toBe(0)
    props.active = true
    await nextTick()
    expect(renderer.setCurrentTime).toHaveBeenLastCalledWith(42500, true)
    expect(frames.size).toBe(1)
    unmount!()
    unmount = undefined
    expect(frames.size).toBe(0)
    expect(renderer.dispose).toHaveBeenCalledOnce()
  })
  it('changing translations rebuilds lyrics at the current position and preserves pause state', async () => {
    const { props, readTime, renderer } = mount(false)
    const lines: LyricLine[] = [{ startTime: 42000, endTime: 46000, words: [{ word: 'Hello', startTime: 42000, endTime: 46000 }], translatedLyric: '你好', romanLyric: '', isBG: false, isDuet: false }]
    readTime.mockReturnValue(43750)
    props.lines = lines
    await nextTick()
    expect(renderer.setLyricLines).toHaveBeenLastCalledWith(lines, 43750)
    expect(renderer.resume).not.toHaveBeenCalled()
    props.playing = true
    await nextTick()
    expect(renderer.resume).toHaveBeenCalledOnce()
  })
  function followLines(): LyricLine[] {
    return [42000, 48000, 54000].map(startTime => ({ startTime, endTime: startTime + 5000, words: [{ word: 'Example', startTime, endTime: startTime + 5000 }], translatedLyric: '', romanLyric: '', isBG: false, isDuet: false }))
  }
  it('returns at the next lyric after the pointer leaves and the short delay has elapsed', async () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { props, host, readTime, renderer } = mount()
    props.lines = followLines()
    await nextTick()
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 500 }))
    host.dispatchEvent(new Event('pointerleave'))
    readTime.mockReturnValue(48000)
    draw(799)
    expect(renderer.resetScroll).not.toHaveBeenCalled()
    draw(800)
    expect(renderer.resetScroll).toHaveBeenCalledOnce()
    expect(renderer.setCurrentTime).toHaveBeenLastCalledWith(48000)
    draw(820)
    expect(renderer.resetScroll).toHaveBeenCalledOnce()
  })
  it('waits for the next lyric even after the delay, and cancels when the pointer re-enters', async () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { props, host, readTime, renderer } = mount()
    props.lines = followLines()
    await nextTick()
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 500 }))
    host.dispatchEvent(new Event('pointerleave'))
    draw(1200)
    expect(renderer.resetScroll).not.toHaveBeenCalled()
    host.dispatchEvent(new Event('pointerenter'))
    readTime.mockReturnValue(48000)
    draw(1400)
    expect(renderer.resetScroll).not.toHaveBeenCalled()
    host.dispatchEvent(new Event('pointerleave'))
    readTime.mockReturnValue(54000)
    draw(2200)
    expect(renderer.resetScroll).toHaveBeenCalledOnce()
  })
  it('does not pull paused lyrics back, and cancels a pending return when hidden', async () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { props, host, readTime, renderer } = mount(false)
    props.lines = followLines()
    await nextTick()
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 500 }))
    host.dispatchEvent(new Event('pointerleave'))
    readTime.mockReturnValue(48000)
    draw(2000)
    expect(renderer.resetScroll).not.toHaveBeenCalled()
    props.playing = true
    await nextTick()
    props.active = false
    await nextTick()
    readTime.mockReturnValue(54000)
    draw(4000)
    expect(renderer.resetScroll).not.toHaveBeenCalled()
  })
})
