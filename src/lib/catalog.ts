import { api } from './api'
import type { ArtistRef, Track } from '../types'

export function artistNames(track: Pick<Track, 'artist'>): string[] {
  return [...new Set(track.artist.split(' / ').map(name => name.trim()).filter(Boolean))]
}

export async function resolveArtist(track: Track, name: string): Promise<ArtistRef> {
  const known = track.artists?.find(artist => artist.name === name && artist.mid)
  if (known) return known
  // Saved tracks predating catalog IDs are resolved by the exact song MID.
  const detail = await api.catalogTrack(track.mid)
  const artist = detail.artists?.find(item => item.name === name && item.mid)
  if (!artist) throw new Error('QQ 音乐没有提供这位歌手的详情入口。')
  return artist
}

export async function resolveAlbum(track: Track): Promise<string> {
  if (track.albumMid) return track.albumMid
  const detail = await api.catalogTrack(track.mid)
  if (!detail.albumMid) throw new Error('QQ 音乐没有提供这张专辑的详情入口。')
  return detail.albumMid
}
