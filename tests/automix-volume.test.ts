import { afterEach, describe, expect, it, vi } from 'vitest'
import { QqAutoMix } from '../src/lib/qq-automix'

class FakeAudio {
  volume = 0.7
  paused = true
  src = ''
  preservesPitch = true
  load = vi.fn()
  pause = vi.fn(() => { this.paused = true })
  removeAttribute(name: string) { if (name === 'src') this.src = '' }
}

describe('direct-volume AutoMix', () => {
  afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks() })

  it('fades two real audio elements and releases the outgoing one without Web Audio', async () => {
    vi.useFakeTimers()
    let now = 0
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    const mixer = new QqAutoMix()
    const old = new FakeAudio(), next = new FakeAudio()
    old.src = 'old.mp3'
    next.volume = 0
    mixer.prepare(old as unknown as HTMLAudioElement, 'old.mp3')
    mixer.prepare(next as unknown as HTMLAudioElement, 'next.mp3')
    const ended = vi.fn()
    mixer.fade(old as unknown as HTMLAudioElement, next as unknown as HTMLAudioElement, { out: 0, input: 0, duration: 2, rate: 1, mode: 'amfilter2' }, 0, ended)
    expect(next.volume).toBeGreaterThan(0)
    expect(next.volume).toBeLessThan(0.7)
    now = 2_000
    vi.advanceTimersByTime(24)
    expect(ended).toHaveBeenCalledOnce()
    expect(old.paused).toBe(true)
    expect(next.volume).toBeCloseTo(0.7, 5)
    await mixer.dispose()
  })
})
