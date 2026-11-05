import { beforeEach, describe, expect, it, vi } from 'vitest'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ isTauri:()=>true, invoke, convertFileSrc:(value:string)=>`asset:${value}` }))
import { api } from '../src/lib/api'
const expired = {code:'AUTH_EXPIRED',message:'登录失效'}
describe('session renewal boundaries', () => {
  beforeEach(()=>{invoke.mockReset()})
  it('renews once before retrying a read and returns its actual result', async () => {
    invoke.mockRejectedValueOnce(expired).mockResolvedValueOnce(undefined).mockResolvedValueOnce({playlists:[],warnings:[]})
    expect(await api.library()).toEqual({playlists:[],warnings:[]})
    expect(invoke.mock.calls.map(call=>call[0])).toEqual(['get_library','renew_session','get_library'])
  })
  it('does not automatically retry a cloud write', async () => {
    invoke.mockRejectedValue(expired)
    await expect(api.mutatePlaylist('create',null,'歌单',null)).rejects.toEqual(expired)
    expect(invoke).toHaveBeenCalledTimes(1)
  })
  it('does not replace a failed renewal with an empty successful library', async () => {
    invoke.mockRejectedValueOnce(expired).mockRejectedValueOnce({code:'NETWORK_ERROR',message:'网络断开'})
    await expect(api.library()).rejects.toMatchObject({code:'NETWORK_ERROR'})
    expect(invoke).toHaveBeenCalledTimes(2)
  })
  it('a second expired response is reported after a single retry', async () => {
    invoke.mockRejectedValueOnce(expired).mockResolvedValueOnce(undefined).mockRejectedValueOnce(expired)
    await expect(api.library()).rejects.toEqual(expired)
    expect(invoke).toHaveBeenCalledTimes(3)
  })
})
