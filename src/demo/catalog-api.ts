import type { AlbumDetail, ArtistDetail, ArtistRef, ArtistSongOrder, CatalogAlbum, CatalogPage, Playlist, PlaylistPage, Track } from '../types'

// This adapter uses public QQ catalog metadata through Vite's local proxy.
// It sends no account cookies; desktop authentication and audio stay in Rust.
type QQ = Record<string, any>
const searchModule = 'music.search.SearchCgiService'
function check(block: QQ) {
  if (block.code !== 0) throw new Error(`QQ 音乐请求失败（代码 ${block.code}）。`)
}
async function rpc(module: string, method: string, param: QQ, search = false): Promise<QQ> {
  const key = search ? module : 'req'
  const payload = search ? { [key]: { module, method, param } } : { comm: { ct: 24, cv: 0 }, [key]: { module, method, param } }
  for (let attempt = 0; attempt < 5; attempt++) {
    const response = await fetch('/__qq/cgi-bin/musicu.fcg', { method: 'POST', credentials: 'omit', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload) })
    if (!response.ok) throw new Error(`QQ 音乐网络请求失败（${response.status}）。`)
    const body: QQ = await response.json()
    check(body)
    const block = body[key]
    if (!block) throw new Error('QQ 音乐返回的数据缺少请求结果。')
    if (search && block.code === 2001 && attempt < 4) { await new Promise(resolve => setTimeout(resolve, 300)); continue }
    check(block)
    if (!block.data) throw new Error('QQ 音乐返回的数据不完整。')
    if (search) {
      check(block.data)
      if (block.data.meta.is_filter < 0) throw new Error('QQ 音乐暂时无法返回搜索结果，请重试。')
    }
    return block.data
  }
  throw new Error('QQ 音乐搜索暂时不可用。')
}
const list = (value: unknown): QQ[] => { if (!Array.isArray(value)) throw new Error('QQ 音乐列表数据不完整。'); return value }
const required = (value: unknown, name: string): string => { if (typeof value !== 'string' || !value) throw new Error(`QQ 音乐数据缺少 ${name}。`); return value }
const count = (value: unknown): number => { if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) throw new Error('QQ 音乐返回的数量无效。'); return value }
function artist(v: QQ): ArtistRef {
  const mid = v.mid ?? v.singerMID ?? v.singer_mid ?? ''
  const picture: string = v.singerPic ?? v.singer_pic ?? (mid ? `https://y.gtimg.cn/music/photo_new/T001R500x500M000${mid}.jpg` : '')
  return { mid, name: required(v.name ?? v.singerName ?? v.singer_name, '歌手名称'), image: picture.replace(/^http:/, 'https:').replace('R150x150', 'R500x500') }
}
function album(v: QQ, artists: ArtistRef[]): CatalogAlbum {
  const mid = required(v.mid ?? v.albumMid ?? v.albumMID, '专辑标识')
  return { mid, name: required(v.name ?? v.albumName, '专辑名称'), artists, credit: artists.length ? artists.map(a => a.name).join(' / ') : v.singerName, cover: `https://y.gtimg.cn/music/photo_new/T002R500x500M000${v.pmid || mid}.jpg`, publishDate: v.publishDate ?? v.publicTime ?? v.release_time ?? '', albumType: v.albumType ?? '' }
}
function track(raw: QQ): Track {
  const v = raw.songInfo ?? raw
  const artists = list(v.singer).map(artist)
  const albumMid = v.album.mid
  return { id: count(v.id), mid: required(v.mid, '歌曲标识'), mediaMid: v.file?.media_mid ?? '', title: required(v.title ?? v.name, '歌曲名称'), artist: artists.map(a => a.name).join(' / '), artists, album: v.album.title ?? v.album.name ?? '', albumMid, cover: albumMid ? `https://y.gtimg.cn/music/photo_new/T002R500x500M000${albumMid}.jpg` : '', duration: count(v.interval), vip: v.pay?.pay_play > 0 }
}
function page<T>(items: T[], total: number, offset: number): CatalogPage<T> {
  count(total)
  if (!items.length && offset < total) throw new Error('QQ 音乐返回了不完整的列表，请重试。')
  return { items, total, nextOffset: offset + items.length, hasMore: offset + items.length < total }
}
async function search(keyword: string, kind: number, offset: number) {
  return rpc(searchModule, 'DoSearchForQQMusicDesktop', { search_type: kind, query: keyword, page_num: Math.floor(offset / 20) + 1, num_per_page: 20 }, true)
}
function searchPage<T>(items: T[], data: QQ, offset: number): CatalogPage<T> {
  if (!items.length && data.meta.nextpage > 0) throw new Error('QQ 音乐返回了不完整的搜索列表，请重试。')
  return { items, total: count(data.meta.sum), nextOffset: offset + 20, hasMore: data.meta.nextpage > 0 }
}
export const publicCatalogApi = {
  searchTracks: async (keyword: string, offset: number): Promise<CatalogPage<Track>> => {
    const d = await search(keyword, 0, offset)
    return searchPage(list(d.body.song.list).map(track), d, offset)
  },
  searchPlaylists: async (keyword: string, offset: number): Promise<CatalogPage<Playlist>> => {
    const d = await search(keyword, 3, offset)
    return searchPage(list(d.body.songlist.list).map(v => ({ id: String(v.dissid), name: required(v.dissname, '歌单名称'), cover: String(v.imgurl).replace(/^http:/, 'https:'), count: count(v.song_count), kind: 'public' as const, creator: v.creator.name })), d, offset)
  },
  publicPlaylist: async (id: string, offset: number): Promise<PlaylistPage> => {
    const d = await rpc('music.srfDissInfo.DissInfo', 'CgiGetDiss', { disstid: Number(id), dirid: 0, tag: true, song_begin: offset, song_num: 100, userinfo: true, orderlist: true, onlysonglist: false })
    const items = list(d.songlist).map(track)
    const p = page(items, count(d.total_song_num), offset)
    return { tracks: p.items, total: p.total, nextOffset: p.nextOffset, hasMore: p.hasMore }
  },
  search: async (keyword: string, page: number): Promise<Track[]> => {
    const d = await rpc(searchModule, 'DoSearchForQQMusicDesktop', { search_type: 0, query: keyword, page_num: page, num_per_page: 30 }, true)
    return list(d.body.song.list).map(track)
  },
  catalogArtists: async (keyword: string, offset: number, area: number): Promise<CatalogPage<ArtistRef>> => {
    if (keyword) { const d = await search(keyword, 1, offset); return searchPage(list(d.body.singer.list).map(artist), d, offset) }
    const d = await rpc('Music.SingerListServer', 'get_singer_list', { area, sex: -100, genre: -100, index: -100, sin: offset, cur_page: offset / 80 + 1 })
    return page(list(d.singerlist).map(artist), d.total, offset)
  },
  catalogAlbums: async (keyword: string, offset: number, area: number): Promise<CatalogPage<CatalogAlbum>> => {
    if (keyword) { const d = await search(keyword, 2, offset); return searchPage(list(d.body.album.list).map(v => album(v, list(v.singer_list).map(artist))), d, offset) }
    const d = await rpc('newalbum.NewAlbumServer', 'get_new_album_info', { area, start: offset, num: 30 })
    return page(list(d.albums).map(v => album(v, list(v.singers).map(artist))), d.total, offset)
  },
  artistDetail: async (mid: string): Promise<ArtistDetail> => {
    const d = await rpc('music.web_singer_info_svr', 'get_singer_detail_info', { sort: 5, singermid: mid, sin: 0, num: 1 })
    const ref = artist(d.singer_info)
    if (ref.mid !== mid) throw new Error('这位歌手暂时无法查看。')
    return { artist: ref, description: d.singer_brief, totalSongs: count(d.total_song), totalAlbums: count(d.total_album) }
  },
  artistSongs: async (mid: string, offset: number, order: ArtistSongOrder): Promise<CatalogPage<Track>> => {
    const d = await rpc('musichall.song_list_server', 'GetSingerSongList', { singerMid: mid, begin: offset, num: 50, order: order === 'popular' ? 1 : 0 })
    return page(list(d.songList).map(track), d.totalNum, offset)
  },
  artistAlbums: async (mid: string, offset: number): Promise<CatalogPage<CatalogAlbum>> => {
    const d = await rpc('music.musichallAlbum.AlbumListServer', 'GetAlbumList', { singerMid: mid, begin: offset, num: 30, order: 1 })
    return page(list(d.albumList).map(v => album(v, [])), d.total, offset)
  },
  albumDetail: async (mid: string): Promise<AlbumDetail> => {
    const d = await rpc('music.musichallAlbum.AlbumInfoServer', 'GetAlbumDetail', { albumMId: mid })
    const result = album(d.basicInfo, list(d.singer.singerList).map(artist))
    if (result.mid !== mid) throw new Error('这张专辑暂时无法查看。')
    const tracks: Track[] = []
    let offset = 0
    while (true) {
      const s = await rpc('music.musichallAlbum.AlbumSongList', 'GetAlbumSongList', { albumMid: mid, begin: offset, num: 100, order: 2 })
      const p = page(list(s.songList).map(track), s.totalNum, offset)
      tracks.push(...p.items); offset = p.nextOffset
      if (!p.hasMore) break
    }
    return { album: result, description: d.basicInfo.desc, language: d.basicInfo.language ?? '', genre: d.basicInfo.genreNew ?? d.basicInfo.genre ?? '', company: d.company.name ?? '', tracks }
  },
  catalogTrack: async (mid: string): Promise<Track> => {
    const d = await rpc('music.pf_song_detail_svr', 'get_song_detail_yqq', { song_mid: mid })
    const t = track(d.track_info)
    if (t.mid !== mid) throw new Error('QQ 音乐返回的歌曲标识不一致。')
    return t
  },
}
