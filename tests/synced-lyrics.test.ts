// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, reactive } from 'vue'
import SyncedLyrics from '../src/components/SyncedLyrics.vue'
import { LayoutReason, type LyricLine } from '@applemusic-like-lyrics/core'

const state = vi.hoisted(() => ({ layout: vi.fn(), resetScroll: vi.fn(), renderer: null as unknown as {
  calcLayout: (reason: string) => void
  dispatchEvent: EventTarget['dispatchEvent']
  resetScroll: () => void
  scrollState: { isTouchScrolled: boolean; isAutoAlignSuspended: boolean }
  currentLyricGroups: { mainLine: { getLine: () => LyricLine }; blur: number }[]
  setLyricLines: ReturnType<typeof vi.fn>
  setCurrentTime: ReturnType<typeof vi.fn>
  update: ReturnType<typeof vi.fn>
  pause: ReturnType<typeof vi.fn>
  resume: ReturnType<typeof vi.fn>
  dispose: ReturnType<typeof vi.fn>
} }))
vi.mock('@applemusic-like-lyrics/core', () => ({
  LayoutReason: { ContinuousScroll: 'continuous-scroll', DiscreteScroll: 'discrete-scroll', ConfigChange: 'config-change', RebuildView: 'rebuild-view', Resize: 'resize' },
  LyricPlayer: class extends EventTarget {
    constructor() { super(); state.renderer = this }
    getElement() { return document.createElement('div') }
    size = [400, 400]
    scrollState = { isTouchScrolled: false, isAutoAlignSuspended: false }
    currentLyricGroups: typeof state.renderer.currentLyricGroups = []
    scrollEngine = { updateBoundary: (_min: number, _max: number) => 100 }
    calcLayout(reason: string) { this.scrollEngine.updateBoundary(-100, 300); state.layout(reason) }
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
    resetScroll() {
      this.scrollState.isTouchScrolled = false
      this.scrollState.isAutoAlignSuspended = false
      state.resetScroll()
    }
  },
}))

describe('lyric frame ownership', () => {
  let frames: Map<number, FrameRequestCallback>
  let frameId: number
  let unmount: (() => void) | undefined
  beforeEach(() => {
    state.layout.mockClear()
    state.resetScroll.mockClear()
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
    const props = reactive({ lines: [] as LyricLine[], playing, active: true, reducedMotion: false, readTime, seekRevision: 0 })
    const lineClick = vi.fn()
    const app = createApp({ render: () => h(SyncedLyrics, { ...props, onLineClick: lineClick }) })
    const root = document.createElement('div')
    document.body.append(root)
    app.mount(root)
    unmount = () => { app.unmount(); root.remove() }
    return { props, readTime, lineClick, renderer: state.renderer, host: root.firstElementChild! }
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
  it('coalesces input into one direct scroll layout per frame and fades its position indicator', () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { renderer, host } = mount(false)
    const scrollbar = host.querySelector('.lyric-scrollbar')!
    state.layout.mockClear()
    for (const reason of [LayoutReason.DiscreteScroll, LayoutReason.DiscreteScroll, LayoutReason.ContinuousScroll]) {
      renderer.calcLayout(reason)
    }
    expect(state.layout).not.toHaveBeenCalled()
    expect(scrollbar.classList.contains('is-visible')).toBe(false)
    draw(17)
    expect(state.layout.mock.calls).toEqual([[LayoutReason.ContinuousScroll]])
    expect(scrollbar.classList.contains('is-visible')).toBe(true)
    expect((scrollbar.firstElementChild as HTMLElement).style.transform).toBe('translateY(100px)')
    draw(666)
    expect(scrollbar.classList.contains('is-visible')).toBe(true)
    draw(667)
    expect(scrollbar.classList.contains('is-visible')).toBe(false)
    expect(state.layout).toHaveBeenCalledOnce()
  })
  it('keeps paused touch browsing clear after the last finger lifts', () => {
    const { renderer, host } = mount(false)
    host.dispatchEvent(new Event('touchmove', { bubbles: true }))
    host.dispatchEvent(Object.assign(new Event('touchend'), { touches: [{}] }))
    expect(renderer.scrollState.isTouchScrolled).toBe(true)
    host.dispatchEvent(Object.assign(new Event('touchend'), { touches: [] }))
    expect(renderer.scrollState.isTouchScrolled).toBe(true)
    draw(performance.now() + 6000)
    expect(state.resetScroll).not.toHaveBeenCalled()
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
  it('restores blur and auto alignment in the frame that reaches the next line after 2.5 seconds', async () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { props, host, readTime, renderer } = mount()
    props.lines = followLines()
    await nextTick()
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 500 }))
    expect(renderer.scrollState.isTouchScrolled).toBe(true)
    // The renderer can request an early reset when the next lyric enters its viewport.
    renderer.resetScroll()
    draw(2499)
    expect(state.resetScroll).not.toHaveBeenCalled()
    draw(2500)
    expect(state.resetScroll).not.toHaveBeenCalled()
    readTime.mockReturnValue(48000)
    draw(3100)
    expect(state.resetScroll).toHaveBeenCalledOnce()
    expect(renderer.setCurrentTime).toHaveBeenLastCalledWith(48000)
    expect(state.layout).toHaveBeenLastCalledWith(LayoutReason.ConfigChange)
    expect(renderer.scrollState.isTouchScrolled).toBe(false)
    draw(3120)
    expect(state.resetScroll).toHaveBeenCalledOnce()
  })
  it('restarts the countdown on wheel input and returns after five seconds in a long gap', async () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { props, host, readTime } = mount()
    props.lines = followLines()
    await nextTick()
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 500 }))
    vi.spyOn(performance, 'now').mockReturnValue(1500)
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 100 }))
    draw(4000)
    draw(6499)
    expect(state.resetScroll).not.toHaveBeenCalled()
    draw(6500)
    expect(state.resetScroll).toHaveBeenCalledOnce()
  })
  it('keeps the paused browsing position, then discards it when the view is hidden', async () => {
    vi.spyOn(performance, 'now').mockReturnValue(0)
    const { props, host, readTime } = mount(false)
    props.lines = followLines()
    await nextTick()
    host.dispatchEvent(new WheelEvent('wheel', { deltaY: 500 }))
    host.dispatchEvent(new Event('pointerleave'))
    readTime.mockReturnValue(48000)
    draw(6000)
    expect(state.resetScroll).not.toHaveBeenCalled()
    props.playing = true
    await nextTick()
    props.active = false
    await nextTick()
    readTime.mockReturnValue(54000)
    draw(7000)
    expect(state.resetScroll).toHaveBeenCalledOnce()
    expect(host.classList.contains('is-browsing')).toBe(false)
  })
  it('opens browsing on the first tap and seeks only after the line becomes clear', async () => {
    const { props, host, renderer, lineClick, readTime } = mount(false)
    const line = { getLine: () => followLines()[1] }
    const group = { mainLine: line, blur: 2 }
    renderer.currentLyricGroups.push(group)
    const click = () => renderer.dispatchEvent(Object.assign(new Event('line-click'), { line }))
    click()
    await nextTick()
    expect(host.classList.contains('is-browsing')).toBe(true)
    expect(lineClick).not.toHaveBeenCalled()
    group.blur = 0
    click()
    expect(lineClick).toHaveBeenCalledOnce()
    readTime.mockReturnValue(48000)
    props.seekRevision++
    await nextTick()
    expect(host.classList.contains('is-browsing')).toBe(false)
    expect(renderer.setCurrentTime).toHaveBeenLastCalledWith(48000, true)
  })
})
