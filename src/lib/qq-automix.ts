export interface QqMixPlan {
  out: number
  input: number
  duration: number
  rate: number
  mode: 'amfilter2'
}

interface Deck {
  source: MediaElementAudioSourceNode
  filter: BiquadFilterNode
  bass: BiquadFilterNode
  gain: GainNode
}

type Points = ReadonlyArray<readonly [number, number]>
// amfilter2 control points from the QQ Music macOS player's preset.
const outgoingGain: Points = [[0, 0], [0.5, 0], [0.75, -2], [0.85, -6], [0.92, -15], [1, -35]]
const incomingGain: Points = [[0.35, -30], [0.5, -15], [0.7, -7], [0.85, -3], [0.92, 0]]
const outgoingFilter: Points = [[0, 20000], [0.1, 10000], [0.25, 6500], [0.4, 4500], [0.65, 4000], [0.75, 3600], [0.85, 2500]]
const incomingFilter: Points = [[0.35, 10000], [0.4, 1500], [0.55, 800], [0.75, 700], [0.82, 550], [0.92, 100], [1, 20]]

function interpolate(points: Points, position: number) {
  if (position <= points[0][0]) return points[0][1]
  for (let i = 1; i < points.length; i++) {
    const [end, target] = points[i]
    if (position <= end) {
      const [start, initial] = points[i - 1]
      return initial + (target - initial) * (position - start) / (end - start)
    }
  }
  return points[points.length - 1][1]
}

export class QqAutoMix {
  readonly context = new AudioContext()
  private decks = new Map<HTMLAudioElement, Deck>()
  private fading: HTMLAudioElement | null = null
  private incoming: HTMLAudioElement | null = null
  private timer: ReturnType<typeof setTimeout> | undefined

  prepare(media: HTMLAudioElement, url: string) {
    const source = this.context.createMediaElementSource(media)
    const filter = this.context.createBiquadFilter()
    const bass = this.context.createBiquadFilter()
    const gain = this.context.createGain()
    filter.type = 'lowpass'
    filter.frequency.value = this.context.sampleRate / 2
    bass.type = 'lowshelf'
    bass.frequency.value = 180
    source.connect(filter).connect(bass).connect(gain).connect(this.context.destination)
    this.decks.set(media, { source, filter, bass, gain })
    media.preservesPitch = true
    // Web Audio must receive a CORS-enabled stream to process its samples.
    media.crossOrigin = 'anonymous'
    media.src = url
    media.load()
  }

  has(media: HTMLAudioElement) { return this.decks.has(media) }
  resume() { return this.context.resume() }

  silence(media: HTMLAudioElement) {
    this.decks.get(media)!.gain.gain.setValueAtTime(0, this.context.currentTime)
  }

  fade(old: HTMLAudioElement, next: HTMLAudioElement, plan: QqMixPlan, elapsed: number, onEnd: () => void) {
    const start = this.context.currentTime
    const remaining = plan.duration - elapsed
    const apply = (param: AudioParam, points: Points, convert: (value: number) => number) => {
      const curve = Float32Array.from({ length: 128 }, (_, i) => {
        const position = (elapsed + remaining * i / 127) / plan.duration
        return convert(interpolate(points, position))
      })
      param.cancelScheduledValues(0)
      param.setValueCurveAtTime(curve, start, remaining)
    }
    const from = this.decks.get(old)!
    const to = this.decks.get(next)!
    from.filter.type = 'lowpass'
    to.filter.type = 'highpass'
    apply(from.filter.frequency, outgoingFilter, hz => hz)
    apply(to.filter.frequency, incomingFilter, hz => hz)
    apply(from.gain.gain, outgoingGain, db => 10 ** (db / 20))
    apply(to.gain.gain, incomingGain, db => 10 ** (db / 20))
    const bassSwap = start + Math.max(0, plan.duration * 0.8 - elapsed)
    from.bass.gain.setValueAtTime(-15, bassSwap)
    to.bass.gain.setValueAtTime(-15, start)
    to.bass.gain.setValueAtTime(0, bassSwap)
    this.fading = old
    this.incoming = next
    this.timer = setTimeout(() => {
      this.finish()
      onEnd()
    }, remaining * 1000)
  }

  finish() {
    clearTimeout(this.timer)
    this.timer = undefined
    if (this.fading) this.release(this.fading)
    this.fading = null
    if (!this.incoming) return
    const media = this.incoming
    this.incoming = null
    const deck = this.decks.get(media)!
    const now = this.context.currentTime
    deck.filter.type = 'lowpass'
    for (const [param, value] of [
      [deck.gain.gain, 1], [deck.bass.gain, 0], [deck.filter.frequency, this.context.sampleRate / 2],
    ] as const) {
      param.cancelScheduledValues(0)
      param.setValueAtTime(value, now)
    }
    if (media.playbackRate !== 1) media.playbackRate = 1
  }

  release(media: HTMLAudioElement) {
    const deck = this.decks.get(media)
    if (!deck) return false
    media.pause()
    media.removeAttribute('src')
    media.load()
    deck.source.disconnect()
    deck.filter.disconnect()
    deck.bass.disconnect()
    deck.gain.disconnect()
    this.decks.delete(media)
    return true
  }

  async dispose() {
    this.finish()
    for (const media of this.decks.keys()) this.release(media)
    await this.context.close()
  }
}
