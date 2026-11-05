import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
const api = vi.hoisted(() => ({ searchTracks:vi.fn(),catalogArtists:vi.fn(),catalogAlbums:vi.fn(),searchPlaylists:vi.fn(),searchHistory:vi.fn(),recordSearch:vi.fn(),clearSearchHistory:vi.fn() }))
vi.mock('../src/lib/api',()=>({api,errorMessage:(e:Error)=>e.message}))
import { useSearchStore } from '../src/stores/search'
const page = (items:unknown[],total=items.length,nextOffset=items.length)=>({items,total,nextOffset,hasMore:nextOffset<total})
describe('unified QQ search',()=>{
  beforeEach(()=>{setActivePinia(createPinia());vi.resetAllMocks();api.recordSearch.mockResolvedValue(undefined);api.searchHistory.mockResolvedValue(['Jay'])})
  it('keeps one query across all four types and remembers it once',async()=>{
    api.searchTracks.mockResolvedValue(page([]));api.catalogArtists.mockResolvedValue(page([]));api.catalogAlbums.mockResolvedValue(page([]));api.searchPlaylists.mockResolvedValue(page([]))
    const search=useSearchStore()
    await search.search('Jay','songs');await search.search('Jay','artists');await search.search('Jay','albums');await search.search('Jay','playlists')
    expect(api.searchTracks).toHaveBeenCalledWith('Jay',0);expect(api.catalogArtists).toHaveBeenCalledWith('Jay',0,-100);expect(api.catalogAlbums).toHaveBeenCalledWith('Jay',0,1);expect(api.searchPlaylists).toHaveBeenCalledWith('Jay',0)
    expect(api.recordSearch).toHaveBeenCalledTimes(1)
  })
  it('uses QQ pagination and retains existing results after a failed next page',async()=>{
    api.searchTracks.mockResolvedValueOnce(page([{mid:'one'}],50,20)).mockRejectedValueOnce(new Error('Network failed'))
    const search=useSearchStore();await search.search('Jay');await search.search('Jay','songs',true)
    expect(api.searchTracks).toHaveBeenLastCalledWith('Jay',20);expect(search.results.songs).toHaveLength(1);expect(search.error).toBe('Network failed');expect(search.results.hasMore).toBe(true)
  })
  it('rejects delayed results after query changes or account reset',async()=>{
    let resolve!:(value:unknown)=>void
    api.searchTracks.mockReturnValueOnce(new Promise(r=>{resolve=r})).mockResolvedValueOnce(page([{mid:'new'}]))
    const search=useSearchStore();const old=search.search('old');await search.search('new');resolve(page([{mid:'old'}]));await old
    expect(search.results.songs.map(t=>t.mid)).toEqual(['new'])
    api.searchTracks.mockReturnValueOnce(new Promise(r=>{resolve=r}));const pending=search.search('other');search.reset();resolve(page([{mid:'other'}]));await pending;expect(search.results.songs).toEqual([])
  })
  it('reports history failures without replacing successful search results',async()=>{
    api.recordSearch.mockRejectedValue(new Error('Disk full'));api.searchTracks.mockResolvedValue(page([{mid:'one'}]))
    const search=useSearchStore();await search.search('Jay');await vi.waitFor(()=>expect(search.historyError).toBe('Disk full'));expect(search.state).toBe('ready');expect(search.results.songs).toHaveLength(1)
  })
})
