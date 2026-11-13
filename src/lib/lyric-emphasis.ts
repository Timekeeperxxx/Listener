import type { LyricLine } from '@applemusic-like-lyrics/core'

function sustainedNotes(line: LyricLine) {
  const words = line.words.filter(word => /\p{L}/u.test(word.word))
  if (words.length < 3) return []
  const last = words.length - 1
  const durations = words.map((word, index) =>
    Math.max(0, Math.min(word.endTime, words[index + 1]?.startTime ?? line.endTime) - word.startTime),
  )
  return words.filter((word, index) => {
    const token = word.word.trim().replace(/^\p{P}+|\p{P}+$/gu, '').trim()
    const length = Array.from(token).length
    if (!/^\p{L}$/u.test(token) && !/^[A-Za-z]{2,7}$/.test(token)) return false
    const isLast = index === last
    const boundary = words[index + 1]?.startTime ?? line.endTime
    const duration = durations[index]
    if (duration < Math.max(isLast ? 1950 : 1750, length * 335) || Math.abs(boundary - word.endTime) > 260) return false

    const neighbors = durations.slice(Math.max(0, index - 2), index + 3)
      .filter((value, offset) => {
        const neighbor = Math.max(0, index - 2) + offset
        return neighbor !== index && value >= 80 &&
          (neighbor !== last || Math.abs(line.endTime - words[last].endTime) <= 260)
      }).sort((a, b) => a - b)
    return neighbors.length >= 2 && duration >= neighbors[Math.floor(neighbors.length / 2)] * (isLast ? 3.6 : 3.2)
  })
}

// Keep AMLL's timing masks and regular lift. Replace its extra character
// animation only for the sustained notes that meet the iPhone thresholds.
export function tuneWordEmphasis(element: HTMLElement, line: LyricLine) {
  const animations = element.getAnimations({ subtree: true })
  const wordStarts = new Map<Element, number>()
  for (const animation of animations) {
    if (animation.id === 'float-word' && animation.effect instanceof KeyframeEffect && animation.effect.target) {
      wordStarts.set(animation.effect.target, line.startTime + (animation.effect.getTiming().delay ?? 0))
    }
  }
  const notes = sustainedNotes(line)
  for (const animation of animations) {
    if (!animation.id.startsWith('emphasize-word-') || !(animation.effect instanceof KeyframeEffect)) continue
    const effect = animation.effect
    let word = effect.target?.parentElement
    while (word && word !== element && !wordStarts.has(word)) word = word.parentElement
    const start = word ? wordStarts.get(word) : undefined
    const note = notes.find(note => note.startTime === start)
    if (!note || animation.id === 'emphasize-word-float') {
      // Keep the animation object usable by AMLL on pause, resume, and seek.
      effect.setKeyframes([])
      continue
    }
    effect.setKeyframes([
      { transform: 'scale(1)', textShadow: '0 0 .12em transparent', easing: 'ease-in-out' },
      { transform: 'translateY(-.02em) scale(1.02, 1.03)', textShadow: '0 0 .12em #ffffff45', easing: 'ease-in-out' },
      { transform: 'scale(1)', textShadow: '0 0 .12em transparent' },
    ])
    effect.updateTiming({ delay: note.startTime - line.startTime + 750, duration: note.endTime - note.startTime - 750 })
  }
}
