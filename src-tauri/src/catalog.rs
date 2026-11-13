use crate::{
    auth::Credentials,
    error::{AppError, Result},
    models::*,
    qq::{check_code, image_url, map_track, number, text, QqMusic},
};
use reqwest::header;
use serde_json::{json, Value};
use std::time::Duration;

const SEARCH_MODULE: &str = "music.search.SearchCgiService";

pub fn validate_mid(mid: &str) -> Result<()> {
    if mid.is_empty() || mid.len() > 64 || !mid.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(AppError::new(
            "INVALID_CATALOG_ID",
            "歌手或专辑标识无效，请重新打开。",
        ));
    }
    Ok(())
}

fn array<'a>(data: &'a Value, path: &str) -> Result<&'a Vec<Value>> {
    data.pointer(path)
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::protocol(path))
}

fn count(data: &Value, field: &str) -> Result<u64> {
    number(&data[field]).ok_or_else(|| AppError::protocol(field))
}

pub(crate) fn map_artist(v: &Value) -> Result<ArtistRef> {
    let mid = text(v, &["mid", "singerMID", "singer_mid"]).unwrap_or_default();
    let image = text(v, &["singerPic", "singer_pic"])
        .map(image_url)
        .map(|url| url.replace("R150x150", "R500x500"))
        .unwrap_or_else(|| {
            if mid.is_empty() {
                String::new()
            } else {
                format!("https://y.gtimg.cn/music/photo_new/T001R500x500M000{mid}.jpg")
            }
        });
    Ok(ArtistRef {
        mid,
        name: text(v, &["name", "singerName", "singer_name"])
            .ok_or_else(|| AppError::protocol("artist.name"))?,
        image,
    })
}

fn map_album(v: &Value, artists: Vec<ArtistRef>) -> Result<CatalogAlbum> {
    let mid =
        text(v, &["mid", "albumMid", "albumMID"]).ok_or_else(|| AppError::protocol("album.mid"))?;
    validate_mid(&mid)?;
    let pmid = text(v, &["pmid"])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| mid.clone());
    Ok(CatalogAlbum {
        cover: format!("https://y.gtimg.cn/music/photo_new/T002R500x500M000{pmid}.jpg"),
        mid,
        name: text(v, &["name", "albumName"]).ok_or_else(|| AppError::protocol("album.name"))?,
        credit: if artists.is_empty() {
            text(v, &["singerName"]).unwrap_or_default()
        } else {
            artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        },
        artists,
        publish_date: text(v, &["publishDate", "publicTime", "release_time"]).unwrap_or_default(),
        album_type: text(v, &["albumType"]).unwrap_or_default(),
    })
}

fn page<T>(items: Vec<T>, total: u64, offset: u64) -> Result<CatalogPage<T>> {
    let next_offset = offset + items.len() as u64;
    if items.is_empty() && offset < total {
        return Err(AppError::new(
            "INCOMPLETE_CATALOG",
            "QQ 音乐返回了不完整的列表，请重试。",
        ));
    }
    Ok(CatalogPage {
        items,
        total,
        next_offset,
        has_more: next_offset < total,
    })
}

impl QqMusic {
    pub async fn search_tracks_page(
        &self,
        auth: Option<&Credentials>,
        keyword: &str,
        offset: u64,
    ) -> Result<CatalogPage<Track>> {
        let data = self
            .search_catalog(auth, keyword, 0, (offset / 20 + 1) as u32, 20)
            .await?;
        let items = array(&data, "/body/song/list")?
            .iter()
            .map(map_track)
            .collect::<Result<Vec<_>>>()?;
        search_page(items, &data, offset)
    }
    pub async fn search_playlists(
        &self,
        auth: Option<&Credentials>,
        keyword: &str,
        offset: u64,
    ) -> Result<CatalogPage<Playlist>> {
        let data = self
            .search_catalog(auth, keyword, 3, (offset / 20 + 1) as u32, 20)
            .await?;
        let items = array(&data, "/body/songlist/list")?
            .iter()
            .map(|v| crate::qq::map_playlist(v, "public"))
            .collect::<Result<Vec<_>>>()?;
        search_page(items, &data, offset)
    }
    async fn catalog_rpc(
        &self,
        auth: Option<&Credentials>,
        module: &str,
        method: &str,
        param: Value,
    ) -> Result<Value> {
        self.rpc(auth, json!({"ct":24,"cv":0}), module, method, param)
            .await
    }

    // Search uses the module name as its envelope key and rejects a top-level comm.
    pub(crate) async fn search_catalog(
        &self,
        auth: Option<&Credentials>,
        keyword: &str,
        kind: u8,
        page: u32,
        num: u32,
    ) -> Result<Value> {
        if keyword.trim().is_empty() || keyword.chars().count() > 100 || page == 0 {
            return Err(AppError::new(
                "INVALID_QUERY",
                "请输入 1–100 个字的搜索内容。",
            ));
        }
        let payload = json!({SEARCH_MODULE:{"module":SEARCH_MODULE,"method":"DoSearchForQQMusicDesktop",
            "param":{"search_type":kind,"query":keyword.trim(),"page_num":page,"num_per_page":num}}});
        for attempt in 0..5 {
            let mut request = self.client.post("https://u.y.qq.com/cgi-bin/musicu.fcg")
                .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .json(&payload);
            if let Some(auth) = auth {
                request = request.header(header::COOKIE, auth.cookie_header());
            }
            let body: Value = request.send().await?.error_for_status()?.json().await?;
            check_code(&body)?;
            let block = body
                .get(SEARCH_MODULE)
                .ok_or_else(|| AppError::protocol("search envelope"))?;
            // QQ intermittently returns business code 2001 for the same valid query.
            if block["code"].as_i64() == Some(2001) && attempt < 4 {
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
            check_code(block)?;
            let data = block
                .get("data")
                .ok_or_else(|| AppError::protocol("search.data"))?;
            check_code(data)?;
            if data
                .pointer("/meta/is_filter")
                .and_then(Value::as_i64)
                .is_some_and(|c| c < 0)
            {
                return Err(AppError::new(
                    "QQ_SEARCH_FILTERED",
                    "QQ 音乐暂时无法返回搜索结果，请稍后重试。",
                ));
            }
            return Ok(data.clone());
        }
        unreachable!("the final attempt returns success or an error")
    }

    pub async fn catalog_artists(
        &self,
        auth: Option<&Credentials>,
        keyword: &str,
        offset: u64,
        area: i32,
    ) -> Result<CatalogPage<ArtistRef>> {
        if !keyword.trim().is_empty() {
            let data = self
                .search_catalog(auth, keyword, 1, (offset / 20 + 1) as u32, 20)
                .await?;
            let items = array(&data, "/body/singer/list")?
                .iter()
                .map(map_artist)
                .collect::<Result<Vec<_>>>()?;
            return search_page(items, &data, offset);
        }
        if ![-100, 200, 2, 5, 4, 3, 6].contains(&area) || !offset.is_multiple_of(80) {
            return Err(AppError::new(
                "INVALID_CATALOG_PAGE",
                "歌手分类或页码无效。",
            ));
        }
        let data = self.catalog_rpc(auth, "Music.SingerListServer", "get_singer_list", json!({
            "area":area,"sex":-100,"genre":-100,"index":-100,"sin":offset,"cur_page":offset/80+1
        })).await?;
        let items = array(&data, "/singerlist")?
            .iter()
            .map(map_artist)
            .collect::<Result<Vec<_>>>()?;
        page(items, count(&data, "total")?, offset)
    }

    pub async fn catalog_albums(
        &self,
        auth: Option<&Credentials>,
        keyword: &str,
        offset: u64,
        area: i32,
    ) -> Result<CatalogPage<CatalogAlbum>> {
        if !keyword.trim().is_empty() {
            let data = self
                .search_catalog(auth, keyword, 2, (offset / 20 + 1) as u32, 20)
                .await?;
            let items = array(&data, "/body/album/list")?
                .iter()
                .map(|v| {
                    map_album(
                        v,
                        array(v, "/singer_list")?
                            .iter()
                            .map(map_artist)
                            .collect::<Result<_>>()?,
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            return search_page(items, &data, offset);
        }
        if !(1..=6).contains(&area) {
            return Err(AppError::new("INVALID_CATALOG_AREA", "专辑地区无效。"));
        }
        let data = self
            .catalog_rpc(
                auth,
                "newalbum.NewAlbumServer",
                "get_new_album_info",
                json!({"area":area,"start":offset,"num":30}),
            )
            .await?;
        let items = array(&data, "/albums")?
            .iter()
            .map(|v| {
                map_album(
                    v,
                    array(v, "/singers")?
                        .iter()
                        .map(map_artist)
                        .collect::<Result<_>>()?,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        page(items, count(&data, "total")?, offset)
    }

    pub async fn artist_detail(
        &self,
        auth: Option<&Credentials>,
        mid: &str,
    ) -> Result<ArtistDetail> {
        validate_mid(mid)?;
        let data = self
            .catalog_rpc(
                auth,
                "music.web_singer_info_svr",
                "get_singer_detail_info",
                json!({"sort":5,"singermid":mid,"sin":0,"num":1}),
            )
            .await?;
        let artist = map_artist(&data["singer_info"])?;
        if artist.mid != mid {
            return Err(AppError::new("CATALOG_NOT_FOUND", "这位歌手暂时无法查看。"));
        }
        Ok(ArtistDetail {
            artist,
            description: text(&data, &["singer_brief"])
                .ok_or_else(|| AppError::protocol("singer_brief"))?,
            total_songs: count(&data, "total_song")?,
            total_albums: count(&data, "total_album")?,
        })
    }

    pub async fn artist_songs(
        &self,
        auth: Option<&Credentials>,
        mid: &str,
        offset: u64,
        order: ArtistSongOrder,
    ) -> Result<CatalogPage<Track>> {
        validate_mid(mid)?;
        let order = match order {
            ArtistSongOrder::Popular => 1,
            ArtistSongOrder::Newest => 0,
        };
        let data = self
            .catalog_rpc(
                auth,
                "musichall.song_list_server",
                "GetSingerSongList",
                json!({"singerMid":mid,"begin":offset,"num":50,"order":order}),
            )
            .await?;
        let items = array(&data, "/songList")?
            .iter()
            .map(map_track)
            .collect::<Result<Vec<_>>>()?;
        page(items, count(&data, "totalNum")?, offset)
    }

    pub async fn artist_albums(
        &self,
        auth: Option<&Credentials>,
        mid: &str,
        offset: u64,
    ) -> Result<CatalogPage<CatalogAlbum>> {
        validate_mid(mid)?;
        let data = self
            .catalog_rpc(
                auth,
                "music.musichallAlbum.AlbumListServer",
                "GetAlbumList",
                json!({"singerMid":mid,"begin":offset,"num":30,"order":1}),
            )
            .await?;
        // This endpoint supplies a credit string, not the IDs of all collaborators.
        let items = array(&data, "/albumList")?
            .iter()
            .map(|v| map_album(v, vec![]))
            .collect::<Result<Vec<_>>>()?;
        page(items, count(&data, "total")?, offset)
    }

    pub async fn album_meta(
        &self,
        auth: Option<&Credentials>,
        mid: &str,
    ) -> Result<CatalogAlbum> {
        validate_mid(mid)?;
        let data = self
            .catalog_rpc(
                auth,
                "music.musichallAlbum.AlbumInfoServer",
                "GetAlbumDetail",
                json!({"albumMId":mid}),
            )
            .await?;
        let artists = array(&data, "/singer/singerList")?
            .iter()
            .map(map_artist)
            .collect::<Result<Vec<_>>>()?;
        let album = map_album(&data["basicInfo"], artists)?;
        if album.mid != mid {
            return Err(AppError::new(
                "CATALOG_NOT_FOUND",
                "这张专辑暂时无法查看。",
            ));
        }
        Ok(album)
    }

    pub async fn album_detail(&self, auth: Option<&Credentials>, mid: &str) -> Result<AlbumDetail> {
        validate_mid(mid)?;
        let data = self
            .catalog_rpc(
                auth,
                "music.musichallAlbum.AlbumInfoServer",
                "GetAlbumDetail",
                json!({"albumMId":mid}),
            )
            .await?;
        let basic = &data["basicInfo"];
        let artists = array(&data, "/singer/singerList")?
            .iter()
            .map(map_artist)
            .collect::<Result<_>>()?;
        let album = map_album(basic, artists)?;
        if album.mid != mid {
            return Err(AppError::new("CATALOG_NOT_FOUND", "这张专辑暂时无法查看。"));
        }
        let mut tracks = Vec::new();
        let mut offset = 0;
        loop {
            let songs = self
                .catalog_rpc(
                    auth,
                    "music.musichallAlbum.AlbumSongList",
                    "GetAlbumSongList",
                    json!({"albumMid":mid,"begin":offset,"num":100,"order":2}),
                )
                .await?;
            let items = array(&songs, "/songList")?
                .iter()
                .map(map_track)
                .collect::<Result<Vec<_>>>()?;
            let page = page(items, count(&songs, "totalNum")?, offset)?;
            offset = page.next_offset;
            tracks.extend(page.items);
            if !page.has_more {
                break;
            }
        }
        Ok(AlbumDetail {
            album,
            tracks,
            description: text(basic, &["desc"]).ok_or_else(|| AppError::protocol("album.desc"))?,
            language: text(basic, &["language"]).unwrap_or_default(),
            genre: text(basic, &["genreNew", "genre"]).unwrap_or_default(),
            company: text(&data["company"], &["name"]).unwrap_or_default(),
        })
    }

    pub async fn catalog_track(&self, auth: Option<&Credentials>, mid: &str) -> Result<Track> {
        validate_mid(mid)?;
        let data = self
            .catalog_rpc(
                auth,
                "music.pf_song_detail_svr",
                "get_song_detail_yqq",
                json!({"song_mid":mid}),
            )
            .await?;
        let track = map_track(&data["track_info"])?;
        if track.mid != mid {
            return Err(AppError::protocol("song.mid mismatch"));
        }
        Ok(track)
    }
}

pub(crate) fn search_page<T>(items: Vec<T>, data: &Value, offset: u64) -> Result<CatalogPage<T>> {
    let meta = &data["meta"];
    let total = count(meta, "sum")?;
    // Search can omit an unavailable result: its page span remains 20, even with 19 items.
    let next_offset = offset + 20;
    let next = meta["nextpage"]
        .as_i64()
        .ok_or_else(|| AppError::protocol("search.nextpage"))?;
    if items.is_empty() && next > 0 {
        return Err(AppError::new(
            "INCOMPLETE_CATALOG",
            "QQ 音乐返回了不完整的搜索结果，请重试。",
        ));
    }
    Ok(CatalogPage {
        items,
        total,
        next_offset,
        has_more: next > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_envelope_uses_module_key() {
        let value = json!({SEARCH_MODULE:{"module":SEARCH_MODULE}});
        assert_eq!(value[SEARCH_MODULE]["module"], SEARCH_MODULE);
    }
    #[test]
    fn refuses_empty_intermediate_page_and_invalid_ids() {
        assert!(page::<Track>(vec![], 1013, 50).is_err());
        assert!(!page::<Track>(vec![], 0, 0).unwrap().has_more);
        assert!(validate_mid("../album").is_err());
    }
    #[test]
    fn search_uses_upstream_page_span_not_visible_count() {
        let result = search_page(vec![1; 19], &json!({"meta":{"sum":47,"nextpage":2}}), 0).unwrap();
        assert_eq!(result.next_offset, 20);
        assert!(result.has_more);
    }
    #[test]
    fn restores_saved_tracks_without_inventing_catalog_identity() {
        let track:Track=serde_json::from_value(json!({"id":1,"mid":"song1","mediaMid":"","title":"Song","artist":"Artist","album":"Album","cover":"","duration":10,"vip":false})).unwrap();
        assert!(track.artists.is_empty());
        assert!(track.album_mid.is_empty());
    }
    #[test]
    fn album_duration_is_never_a_track_count() {
        let album = map_album(
            &json!({"albumMid":"realmid","albumName":"Album","albumDuration":48}),
            vec![],
        )
        .unwrap();
        assert_eq!(album.mid, "realmid");
        assert!(map_album(&json!({"albumName":"Missing identity"}), vec![]).is_err());
    }
}
