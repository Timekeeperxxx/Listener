import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { QqAutoMix } from '../src/lib/qq-automix'

class FakeParam {
  value = 0
  cancelScheduledValues = vi.fn()
  setValueCurveAtTime = vi.fn()
  setValueAtTime = vi.fn()
}
class FakeNode {
  connect() { return this }
  disconnect = vi.fn()
}
class FakeContext {
  currentTime = 0
  sampleRate = 48_000
  destination = new FakeNode()
  createMediaElementSource() { return new FakeNode() }
  createBiquadFilter() { return Object.assign(new FakeNode(), { type: 'lowpass', frequency: new FakeParam(), gain: new FakeParam() }) }
  createGain() { return Object.assign(new FakeNode(), { gain: new FakeParam() }) }
  close = vi.fn(async () => {})
}
class FakeAudio {
  paused = true
  playbackRate = 1
  src = ''
  preservesPitch = true
  crossOrigin: string | null = null
  pause = vi.fn(() => { this.paused = true })
  load = vi.fn()
  removeAttribute(name: string) { if (name === 'src') this.src = '' }
}

describe('QQ AutoMix transition teardown', () => {
  beforeEach(() => { vi.useFakeTimers(); vi.stubGlobal('AudioContext', FakeContext) })
  afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals() })

  it('pauses the outgoing track immediately but defers load and node teardown', async () => {
    const mixer = new QqAutoMix()
    const old = new FakeAudio() as unknown as HTMLAudioElement
    const next = new FakeAudio() as unknown as HTMLAudioElement
    mixer.prepare(old, 'old.mp3'); mixer.prepare(next, 'next.mp3')
    next.playbackRate = 1.2
    const onEnd = vi.fn()
    mixer.fade(old, next, { out: 0, input: 0, duration: 2, rate: 1.2, mode: 'amfilter2' }, 0, onEnd)

    vi.advanceTimersByTime(2_000)
    expect(onEnd).toHaveBeenCalledOnce()
    expect((old as unknown as FakeAudio).paused).toBe(true)
    expect((old as unknown as FakeAudio).src).toBe('old.mp3')
    expect(mixer.has(old)).toBe(true)

    vi.advanceTimersByTime(599)
    expect((old as unknown as FakeAudio).src).toBe('old.mp3')
    vi.advanceTimersByTime(1)
    expect((old as unknown as FakeAudio).src).toBe('')
    expect(mixer.has(old)).toBe(false)
    await mixer.dispose()
  })

  it('returns the incoming track to normal rate over three seconds', async () => {
    const mixer = new QqAutoMix()
    const old = new FakeAudio() as unknown as HTMLAudioElement
    const next = new FakeAudio() as unknown as HTMLAudioElement
    mixer.prepare(old, 'old.mp3'); mixer.prepare(next, 'next.mp3')
    next.playbackRate = 1.2
    mixer.fade(old, next, { out: 0, input: 0, duration: 2, rate: 1.2, mode: 'amfilter2' }, 0, () => {})

    vi.advanceTimersByTime(2_000)
    expect(next.playbackRate).toBe(1.2)
    vi.advanceTimersByTime(1_500)
    expect(next.playbackRate).toBeGreaterThan(1)
    expect(next.playbackRate).toBeLessThan(1.2)
    vi.advanceTimersByTime(1_500)
    expect(next.playbackRate).toBeCloseTo(1, 5)
    await mixer.dispose()
  })
})
