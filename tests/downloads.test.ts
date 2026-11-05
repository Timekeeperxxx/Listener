import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia,setActivePinia } from 'pinia'
import type { Download,Track } from '../src/types'
const api=vi.hoisted(()=>({downloads:vi.fn(),download:vi.fn(),cancelDownload:vi.fn(),removeDownload:vi.fn()}))
vi.mock('../src/lib/api',()=>({api,errorMessage:(e:{message:string})=>e.message}))
import { useDownloadsStore } from '../src/stores/downloads'
const track:Track={id:1,mid:'mid1',mediaMid:'',title:'Test',artist:'A',album:'',cover:'',duration:120,vip:false}
const record:Download={track,quality:'无损 FLAC',filename:'a.flac',bytes:1234}
describe('download state reflects completed files',()=>{
 beforeEach(()=>{setActivePinia(createPinia());vi.resetAllMocks()})
 it('a failed download stays visible as failed, without a playable local record',async()=>{
  api.download.mockRejectedValue({message:'音源权限不足'})
  const store=useDownloadsStore();await store.start(track)
  expect(store.records).toEqual([]);expect(store.jobs[0].state).toBe('error');expect(store.jobs[0].error).toBe('音源权限不足')
 })
 it('finishing an old account download does not fill the new account library',async()=>{
  let resolve!:(value:Download)=>void
  api.download.mockReturnValue(new Promise(r=>resolve=r))
  const store=useDownloadsStore();const running=store.start(track);store.clear();resolve(record);await running
  expect(store.records).toEqual([]);expect(store.jobs).toEqual([])
 })
 it('only a successful file result is shown as downloaded',async()=>{
  api.download.mockResolvedValue(record)
  const store=useDownloadsStore();await store.start(track)
  expect(store.records).toEqual([record]);expect(store.jobs).toEqual([])
 })
})
