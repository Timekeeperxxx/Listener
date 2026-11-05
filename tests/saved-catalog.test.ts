import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
const api=vi.hoisted(()=>({savedCatalog:vi.fn(),saveArtist:vi.fn(),saveAlbum:vi.fn()}))
vi.mock('../src/lib/api',()=>({api,errorMessage:(e:Error)=>e.message}))
import { useSavedCatalogStore } from '../src/stores/saved-catalog'
const artist={mid:'realmid',name:'Artist',image:'portrait.jpg'}
describe('saved catalog ownership and confirmation',()=>{
  beforeEach(()=>{setActivePinia(createPinia());vi.resetAllMocks();api.savedCatalog.mockResolvedValue({artists:[],albums:[]})})
  it('changes the favorite only after persistence succeeds',async()=>{
    api.saveArtist.mockRejectedValueOnce(new Error('Disk full')).mockResolvedValueOnce({artists:[artist],albums:[]})
    const saved=useSavedCatalogStore();await saved.load();await saved.toggleArtist(artist)
    expect(saved.hasArtist(artist.mid)).toBe(false);expect(saved.error).toBe('Disk full')
    await saved.toggleArtist(artist);expect(saved.hasArtist(artist.mid)).toBe(true)
  })
  it('does not refill favorites after account reset',async()=>{
    let resolve!:(value:unknown)=>void;api.savedCatalog.mockReturnValue(new Promise(r=>{resolve=r}))
    const saved=useSavedCatalogStore();const loading=saved.load();saved.reset();resolve({artists:[artist],albums:[]});await loading
    expect(saved.items.artists).toEqual([]);expect(saved.state).toBe('idle')
  })
})
