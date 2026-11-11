export interface QqMixPlan {
  out: number
  input: number
  duration: number
  rate: number
  mode: 'amfilter2'
}

type Points = ReadonlyArray<readonly [number, number]>
// amfilter2 gain control points from the QQ Music macOS player's preset.
const outgoingGain: Points = [[0, 0], [0.5, 0], [0.75, -2], [0.85, -6], [0.92, -15], [1, -35]]
const incomingGain: Points = [[0.35, -30], [0.5, -15], [0.7, -7], [0.85, -3], [0.92, 0]]

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

function dbToVolume(db: number) {
  return Math.max(0, Math.min(1, 10 ** (db / 20)))
}

/**
 * QQ cut-point AutoMix using direct HTMLAudioElement volume changes.
 *
 * It deliberately does not create a MediaElementAudioSourceNode: routing the
 * playing element through Web Audio makes WebKit buffer/seek much more slowly,
 * which is visible as a large delay after clicking a lyric line to seek.
 */
export class QqAutoMix {
  // The player uses this state only to suspend/resume the graph. Direct volume
  // crossfades have no graph, so the calls are intentionally no-ops.
  readonly context = {
    state: 'running' as const,
    suspend: async () => {},
    resume: async () => {},
    close: async () => {},
  }
  private decks = new Set<HTMLAudioElement>()
  private fading: HTMLAudioElement | null = null
  private incoming: HTMLAudioElement | null = null
  private timer: ReturnType<typeof setTimeout> | undefined
  private baseVolume = 1

  prepare(media: HTMLAudioElement, url: string) {
    this.decks.add(media)
    media.preservesPitch = true
    media.src = url
    media.load()
  }

  has(media: HTMLAudioElement) { return this.decks.has(media) }
  resume() { return Promise.resolve() }

  silence(media: HTMLAudioElement) {
    media.volume = 0
  }

  fade(old: HTMLAudioElement, next: HTMLAudioElement, plan: QqMixPlan, elapsed: number, onEnd: () => void) {
    const started = performance.now()
    const base = Math.max(old.volume, next.volume, 0)
    this.baseVolume = base
    this.fading = old
    this.incoming = next
    next.volume = 0

    const step = () => {
      const position = Math.min(1, (elapsed + (performance.now() - started) / 1000) / plan.duration)
      old.volume = Math.max(0, Math.min(base, base * dbToVolume(interpolate(outgoingGain, position))))
      next.volume = Math.max(0, Math.min(base, base * dbToVolume(interpolate(incomingGain, position))))
      if (position < 1) {
        this.timer = setTimeout(step, 24)
        return
      }
      this.finish()
      onEnd()
    }
    step()
  }

  finish() {
    clearTimeout(this.timer)
    this.timer = undefined
    if (this.fading) {
      this.fading.volume = 0
      this.release(this.fading)
      this.fading = null
    }
    if (this.incoming) {
      this.incoming.volume = Math.min(1, this.baseVolume)
      this.incoming = null
    }
  }

  release(media: HTMLAudioElement) {
    if (!this.decks.has(media)) return false
    media.pause()
    media.removeAttribute('src')
    media.load()
    this.decks.delete(media)
    return true
  }

  async dispose() {
    this.finish()
    for (const media of [...this.decks]) this.release(media)
  }
}
