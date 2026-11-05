import { describe, expect, it } from 'vitest'
import { filterTracks } from '../src/lib/track-filter'
import type { Track } from '../src/types'
const track = (id: number, title: string, artist: string, album: string, duration: number): Track => ({ id, mid:`mid${id}`,mediaMid:'',title,artist,album,duration,cover:'',vip:false })
const tracks = [track(1,'Night Flight','Alice','Moon',180),track(2,'日光','张三','天空',90),track(3,'归途','李四','Moon',240)]
describe('song filtering and ordering', () => {
  it('searches title, artist and album without changing the original queue', () => {
    expect(filterTracks(tracks,'  NIGHT ', 'default').map(t=>t.id)).toEqual([1])
    expect(filterTracks(tracks,'张三','default').map(t=>t.id)).toEqual([2])
    expect(filterTracks(tracks,'moon','default').map(t=>t.id)).toEqual([1,3])
    expect(filterTracks(tracks,'Ｎｉｇｈｔ','default').map(t=>t.id)).toEqual([1])
    expect(tracks.map(t=>t.id)).toEqual([1,2,3])
  })
  it('sorts the complete matched set by duration in either direction', () => {
    expect(filterTracks(tracks,'','duration').map(t=>t.id)).toEqual([2,1,3])
    expect(filterTracks(tracks,'','duration',true).map(t=>t.id)).toEqual([3,1,2])
    expect(filterTracks(tracks,'absent','title')).toEqual([])
  })
})
