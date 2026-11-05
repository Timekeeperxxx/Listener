import type { Track } from '../types'
export type TrackSort = 'default' | 'title' | 'artist' | 'album' | 'duration'
export function filterTracks(tracks: Track[], query: string, sort: TrackSort, descending = false): Track[] {
  const keyword = query.trim().normalize('NFKC').toLocaleLowerCase()
  const result = tracks.filter(track => [track.title, track.artist, track.album].some(value => value.normalize('NFKC').toLocaleLowerCase().includes(keyword)))
  if (sort !== 'default') result.sort((a, b) => (sort === 'duration' ? a.duration - b.duration : a[sort].localeCompare(b[sort], 'zh-CN')) * (descending ? -1 : 1))
  return result
}
