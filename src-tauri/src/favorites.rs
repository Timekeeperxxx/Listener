use crate::{
    auth::Credentials,
    catalog::validate_mid,
    error::{AppError, Result},
    models::{ArtistRef, CatalogAlbum, Library, SavedCatalog},
    qq::{image_url, number, text, timestamp, QqMusic},
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const RELATION_MODULE: &str = "music.concern.RelationList";
const CONCERN_MODULE: &str = "Concern.ConcernSystemServer";
const ALBUM_READ_MODULE: &str = "music.musicasset.AlbumFavRead";
const ALBUM_WRITE_MODULE: &str = "music.musicasset.AlbumFavWrite";
const ARTIST_PAGE: u64 = 100;
const ALBUM_PAGE: u64 = 20;
const MAX_FAVORITES: u64 = 2000;

fn web_comm(auth: &Credentials) -> Result<Value> {
    let token: u64 = auth.csrf_token().parse().map_err(|_| {
        AppError::new(
            "AUTH_INVALID",
            "QQ 登录状态缺少可用的请求令牌，请重新登录。",
        )
    })?;
    Ok(json!({
        "ct": 24,
        "cv": 4747474,
        "format": "json",
        "inCharset": "utf-8",
        "outCharset": "utf-8",
        "notice": 0,
        "platform": "yqq.json",
        "needNewCode": 1,
        "uin": auth.uin,
        "authst": auth.music_key,
        "tmeLoginType": auth.login_type,
        "g_tk": token,
        "g_tk_new_20200303": token,
    }))
}

fn desktop_comm(auth: &Credentials) -> Value {
    // The web profile (ct 24) rejects this write with 10018. The desktop
    // profile is accepted by QQ; every write is followed by a read-back.
    json!({
        "ct": 20,
        "cv": 1770,
        "format": "json",
        "uin": auth.uin,
        "authst": auth.music_key,
        "tmeAppID": "qqmusic",
        "tmeLoginType": auth.login_type,
    })
}

fn flag(value: &Value) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| number(value).is_some_and(|number| number > 0))
}

fn artist_image(mid: &str) -> String {
    format!("https://y.gtimg.cn/music/photo_new/T001R500x500M000{mid}.jpg")
}

fn album_cover(value: &str, mid: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") || value.starts_with("//") {
        image_url(value.to_owned())
    } else if value.is_empty() {
        format!("https://y.gtimg.cn/music/photo_new/T002R500x500M000{mid}.jpg")
    } else {
        format!("https://y.gtimg.cn/music/photo_new/T002R500x500M000{value}.jpg")
    }
}

fn date_from_seconds(seconds: u64) -> String {
    // Howard Hinnant's civil-from-days algorithm; avoids a date-time dependency.
    let days = (seconds / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

fn publish_date(value: String) -> String {
    let value = value.trim();
    let numeric = value.chars().all(|character| character.is_ascii_digit());
    if numeric && value.len() >= 9 {
        if let Ok(seconds) = value.parse::<u64>() {
            let seconds = if seconds >= 1_000_000_000_000 {
                seconds / 1000
            } else {
                seconds
            };
            if seconds > 0 {
                return date_from_seconds(seconds);
            }
        }
    }
    value.to_owned()
}

fn favorite_artist(raw: &Value) -> Result<ArtistRef> {
    let mid = text(raw, &["MID", "mid"]).ok_or_else(|| AppError::protocol("favorite.artist.mid"))?;
    validate_mid(&mid)?;
    let name =
        text(raw, &["Name", "name"]).ok_or_else(|| AppError::protocol("favorite.artist.name"))?;
    let image = text(raw, &["AvatarUrl", "avatarUrl", "singerPic", "singer_pic"])
        .map(image_url)
        .unwrap_or_else(|| artist_image(&mid));
    Ok(ArtistRef { mid, name, image })
}

fn favorite_album_artist(raw: &Value) -> Result<ArtistRef> {
    let mid = text(raw, &["mid", "MID", "singerMID", "singer_mid"])
        .ok_or_else(|| AppError::protocol("favorite.album.artist.mid"))?;
    validate_mid(&mid)?;
    let name = text(raw, &["name", "Name", "singerName", "singer_name"])
        .ok_or_else(|| AppError::protocol("favorite.album.artist.name"))?;
    let image = text(raw, &["singerPic", "singer_pic", "AvatarUrl", "avatarUrl"])
        .map(image_url)
        .unwrap_or_else(|| artist_image(&mid));
    Ok(ArtistRef { mid, name, image })
}

fn favorite_album(raw: &Value) -> Result<CatalogAlbum> {
    let mid = text(raw, &["albumMid", "mid", "albumMID"])
        .ok_or_else(|| AppError::protocol("favorite.album.mid"))?;
    validate_mid(&mid)?;
    let name = text(raw, &["albumName", "name", "title"])
        .ok_or_else(|| AppError::protocol("favorite.album.name"))?;
    let artists = raw
        .get("v_singer")
        .or_else(|| raw.get("singerList"))
        .or_else(|| raw.get("singer"))
        .and_then(Value::as_array)
        .map(|items| items.iter().map(favorite_album_artist).collect::<Result<Vec<_>>>())
        .transpose()?
        .unwrap_or_default();
    let cover = album_cover(
        &text(raw, &["albumPic", "pic", "cover"]).unwrap_or_default(),
        &mid,
    );
    let credit = if artists.is_empty() {
        text(raw, &["singerName", "singer_name", "singer"]).unwrap_or_default()
    } else {
        artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    };
    Ok(CatalogAlbum {
        mid,
        name,
        artists,
        credit,
        cover,
        publish_date: text(
            raw,
            &["pubTime", "publishDate", "publicTime", "release_time"],
        )
        .map(publish_date)
        .unwrap_or_default(),
        album_type: text(raw, &["albumType"]).unwrap_or_default(),
    })
}

fn follow_param(artist_mid: &str, saved: bool) -> Value {
    json!({
        "opertype": if saved { 0 } else { 1 },
        "source": 0,
        "userinfo": { "usertype": 1, "userid": artist_mid },
        "encrypt_singerid": 1,
    })
}

fn album_param(uin: &str, album_mid: &str) -> Value {
    json!({ "uin": uin, "v_albumMid": [album_mid] })
}

impl QqMusic {
    pub async fn set_favorite_playlist(
        &self,
        auth: &Credentials,
        id: &str,
        saved: bool,
    ) -> Result<Library> {
        let playlist_id = id
            .parse::<u64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| AppError::new("INVALID_PLAYLIST", "这张歌单无法收藏。"))?;
        let uin = auth.encrypt_uin();
        if uin.is_empty() {
            return Err(AppError::new(
                "AUTH_INVALID",
                "当前登录信息缺少歌单收藏所需的账户标识，请重新扫码登录。",
            ));
        }
        let data = self
            .rpc(
                Some(auth),
                web_comm(auth)?,
                "music.musicasset.PlaylistFavWrite",
                if saved {
                    "FavPlaylist"
                } else {
                    "CancelFavPlaylist"
                },
                json!({ "uin": uin, "v_playlistId": [playlist_id] }),
            )
            .await?;
        let result = number(&data["result"])
            .ok_or_else(|| AppError::protocol("favorite.playlist.result"))?;
        if result != 0
            || data["v_failedPlaylistId"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| number(id) == Some(playlist_id)))
        {
            return Err(AppError::new(
                "CLOUD_WRITE_FAILED",
                "QQ 音乐拒绝了这次歌单收藏操作。",
            ));
        }
        let library = self.library(auth).await?;
        let confirmed = library
            .playlists
            .iter()
            .any(|item| item.id == id && item.kind == "collected");
        if !library.warnings.is_empty() || confirmed != saved {
            return Err(AppError::new(
                "CLOUD_VERIFY_FAILED",
                "QQ 音乐已接收请求，但收藏状态尚未确认，请刷新后查看。",
            ));
        }
        Ok(library)
    }

    async fn favorite_album_page(&self, auth: &Credentials, offset: u64) -> Result<Value> {
        let comm = web_comm(auth)?;
        let mut attempts = Vec::new();
        let encrypt_uin = auth.encrypt_uin();
        if !encrypt_uin.is_empty() {
            attempts.push(json!({ "euin": encrypt_uin }));
        }
        attempts.push(json!({ "uin": auth.uin }));
        attempts.push(json!({}));
        let mut last = None;
        for identity in attempts {
            let mut param = json!({ "offset": offset, "size": ALBUM_PAGE });
            if let Some(identity) = identity.as_object() {
                for (key, value) in identity {
                    param[key] = value.clone();
                }
            }
            match self
                .rpc(
                    Some(auth),
                    comm.clone(),
                    ALBUM_READ_MODULE,
                    "CgiGetAlbumFavInfo",
                    param,
                )
                .await
            {
                Ok(data) => return Ok(data),
                Err(error)
                    if matches!(
                        error.code.as_str(),
                        "AUTH_EXPIRED" | "NETWORK_ERROR" | "NETWORK_TIMEOUT" | "HTTP_ERROR"
                    ) =>
                {
                    return Err(error)
                }
                Err(error) => last = Some(error),
            }
        }
        Err(last.unwrap_or_else(|| {
            AppError::new(
                "QQ_REQUEST_FAILED",
                "QQ 音乐暂时无法读取收藏的专辑，请稍后重试。",
            )
        }))
    }

    async fn favorite_artists(&self, auth: &Credentials) -> Result<(Vec<ArtistRef>, HashMap<String, u64>)> {
        let mut artists = Vec::new();
        let mut saved_at = HashMap::new();
        let mut seen = HashSet::new();
        let mut offset = 0u64;
        loop {
            let data = self
                .rpc(
                    Some(auth),
                    web_comm(auth)?,
                    RELATION_MODULE,
                    "GetFollowSingerList",
                    json!({ "From": offset, "Size": ARTIST_PAGE }),
                )
                .await?;
            let list = data
                .get("List")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let total = number(&data["Total"]).unwrap_or(0);
            if list.is_empty() {
                if offset < total {
                    return Err(AppError::new(
                        "INCOMPLETE_FAVORITES",
                        "QQ 音乐返回了不完整的关注歌手列表，请重试。",
                    ));
                }
                break;
            }
            for raw in &list {
                let artist = favorite_artist(raw)?;
                if seen.insert(artist.mid.clone()) {
                    if let Some(time) = timestamp(raw, &["FollowTime", "followTime", "AddTime", "addTime"]) {
                        saved_at.insert(artist.mid.clone(), time);
                    }
                    artists.push(artist);
                }
            }
            offset += list.len() as u64;
            if !flag(&data["HasMore"]) || offset >= total || offset >= MAX_FAVORITES {
                break;
            }
        }
        Ok((artists, saved_at))
    }

    async fn favorite_albums(&self, auth: &Credentials) -> Result<(Vec<CatalogAlbum>, HashMap<String, u64>)> {
        let mut albums = Vec::new();
        let mut saved_at = HashMap::new();
        let mut seen = HashSet::new();
        let mut offset = 0u64;
        loop {
            let data = self.favorite_album_page(auth, offset).await?;
            let list = data
                .get("v_list")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let total = number(&data["total"]).unwrap_or(0);
            if list.is_empty() {
                if offset < total {
                    return Err(AppError::new(
                        "INCOMPLETE_FAVORITES",
                        "QQ 音乐返回了不完整的收藏专辑列表，请重试。",
                    ));
                }
                break;
            }
            for raw in &list {
                let album = favorite_album(raw)?;
                if seen.insert(album.mid.clone()) {
                    if let Some(time) = timestamp(raw, &["favTime", "fav_time", "addTime", "collectTime"]) {
                        saved_at.insert(album.mid.clone(), time);
                    }
                    albums.push(album);
                }
            }
            offset += list.len() as u64;
            if !flag(&data["hasmore"]) || offset >= total || offset >= MAX_FAVORITES {
                break;
            }
        }
        Ok((albums, saved_at))
    }

    pub async fn favorite_catalog(&self, auth: &Credentials) -> Result<SavedCatalog> {
        let ((artists, artist_saved_at), (albums, album_saved_at)) = tokio::try_join!(
            self.favorite_artists(auth),
            self.favorite_albums(auth)
        )?;
        Ok(SavedCatalog { artists, albums, artist_saved_at, album_saved_at })
    }

    pub async fn set_favorite_artist(
        &self,
        auth: &Credentials,
        artist: &ArtistRef,
        saved: bool,
    ) -> Result<()> {
        validate_mid(&artist.mid)?;
        let data = self
            .rpc(
                Some(auth),
                desktop_comm(auth),
                CONCERN_MODULE,
                "cgi_concern_user_v2",
                follow_param(&artist.mid, saved),
            )
            .await?;
        let code = number(&data["code"]).ok_or_else(|| AppError::protocol("favorite.artist.code"))?;
        if code != 0 {
            return Err(AppError::new(
                "QQ_REQUEST_FAILED",
                format!(
                    "QQ 音乐未能{}这位歌手（代码 {code}）。",
                    if saved { "关注" } else { "取消关注" }
                ),
            ));
        }
        Ok(())
    }

    pub async fn set_favorite_album(
        &self,
        auth: &Credentials,
        album: &CatalogAlbum,
        saved: bool,
    ) -> Result<()> {
        validate_mid(&album.mid)?;
        let data = self
            .rpc(
                Some(auth),
                web_comm(auth)?,
                ALBUM_WRITE_MODULE,
                if saved { "FavAlbum" } else { "CancelFavAlbum" },
                album_param(&auth.uin, &album.mid),
            )
            .await?;
        let result =
            number(&data["result"]).ok_or_else(|| AppError::protocol("favorite.album.result"))?;
        let failed = ["v_failedAlbumId", "v_failedAlbumMid"]
            .iter()
            .any(|key| {
                data.get(key)
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty())
            });
        if result != 0 || failed {
            return Err(AppError::new(
                "QQ_REQUEST_FAILED",
                "QQ 音乐未能更新收藏的专辑，请稍后重试。",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_followed_singers_from_the_relation_list() {
        let artist = favorite_artist(&json!({
            "MID": "0025NhlN2yWrP4",
            "Name": "周杰伦",
            "AvatarUrl": "http://y.gtimg.cn/music/photo_new/T001R300x300M000abc.jpg"
        }))
        .unwrap();
        assert_eq!(artist.mid, "0025NhlN2yWrP4");
        assert_eq!(artist.name, "周杰伦");
        assert!(artist.image.starts_with("https://"));
        assert!(favorite_artist(&json!({"Name": "缺少 MID"})).is_err());
    }

    #[test]
    fn maps_favorite_albums_with_real_ids_and_artists() {
        let album = favorite_album(&json!({
            "albumMid": "004YCGQa3qOG8u",
            "albumName": "经典全纪录",
            "albumPic": "http://y.gtimg.cn/music/photo_new/T002R300x300M000004YCGQa3qOG8u.jpg",
            "pubTime": "2020-01-01",
            "v_singer": [{"mid": "0025NhlN2yWrP4", "name": "周杰伦"}]
        }))
        .unwrap();
        assert_eq!(album.mid, "004YCGQa3qOG8u");
        assert_eq!(album.name, "经典全纪录");
        assert_eq!(album.credit, "周杰伦");
        assert_eq!(album.artists[0].mid, "0025NhlN2yWrP4");
        assert!(album.cover.starts_with("https://"));
        assert!(favorite_album(&json!({"albumName": "缺少 MID"})).is_err());
    }

    #[test]
    fn normalizes_favorite_album_dates_and_covers() {
        let album = favorite_album(&json!({
            "albumMid": "004YCGQa3qOG8u",
            "albumName": "经典全纪录",
            "albumPic": "004YCGQa3qOG8u",
            "pubTime": 1_577_836_800_u64,
            "v_singer": [{"mid": "0025NhlN2yWrP4", "name": "周杰伦"}]
        }))
        .unwrap();
        assert_eq!(album.publish_date, "2020-01-01");
        assert_eq!(
            album.cover,
            "https://y.gtimg.cn/music/photo_new/T002R500x500M000004YCGQa3qOG8u.jpg"
        );
        assert_eq!(publish_date("2020-01-01".into()), "2020-01-01");
    }

    #[test]
    fn build_write_parameters_from_real_catalog_ids() {
        assert_eq!(
            follow_param("0025NhlN2yWrP4", true)["userinfo"]["userid"],
            "0025NhlN2yWrP4"
        );
        assert_eq!(follow_param("0025NhlN2yWrP4", true)["opertype"], 0);
        assert_eq!(follow_param("0025NhlN2yWrP4", false)["opertype"], 1);
        assert_eq!(
            album_param("1234", "004YCGQa3qOG8u")["v_albumMid"][0],
            "004YCGQa3qOG8u"
        );
        assert_eq!(album_param("1234", "004YCGQa3qOG8u")["uin"], "1234");
    }
}
