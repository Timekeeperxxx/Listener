use crate::{
    auth::Credentials,
    error::{AppError, Result},
    models::*,
};
use reqwest::{header, Client};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};

const GATEWAY: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const PAGE_SIZE: u64 = 100;

#[derive(Clone)]
pub struct QqMusic {
    pub(crate) client: Client,
}

impl QqMusic {
    pub fn new() -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::REFERER,
            header::HeaderValue::from_static("https://y.qq.com/"),
        );
        let client = Client::builder().default_headers(headers)
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15")
            .connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { client })
    }

    pub(crate) async fn rpc(
        &self,
        auth: Option<&Credentials>,
        comm: Value,
        module: &str,
        method: &str,
        param: Value,
    ) -> Result<Value> {
        let mut request = self.client.post(GATEWAY).json(&json!({
            "comm": comm, "req": { "module": module, "method": method, "param": param }
        }));
        if let Some(auth) = auth {
            request = request.header(header::COOKIE, auth.cookie_header());
        }
        let body: Value = request.send().await?.error_for_status()?.json().await?;
        check_code(&body)?;
        let block = body.get("req").ok_or_else(|| AppError::protocol("req"))?;
        check_code(block)?;
        let data = block
            .get("data")
            .ok_or_else(|| AppError::protocol("data"))?;
        if data
            .get("subcode")
            .and_then(Value::as_i64)
            .is_some_and(|c| c != 0)
        {
            return Err(AppError::new(
                "QQ_REQUEST_FAILED",
                "QQ 音乐暂时无法提供这项数据。",
            ));
        }
        Ok(data.clone())
    }

    async fn get(
        &self,
        auth: &Credentials,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> Result<Value> {
        let body: Value = self
            .client
            .get(endpoint)
            .query(params)
            .header(header::COOKIE, auth.cookie_header())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        check_code(&body)?;
        Ok(body)
    }

    pub async fn profile(&self, auth: &Credentials) -> Result<Profile> {
        // Public profile data can remain readable with an expired ticket. Validate
        // the authenticated account before treating a restored session as logged in.
        self.rpc(
            Some(auth),
            auth.comm(),
            "music.UserInfo.userInfoServer",
            "GetLoginUserInfo",
            json!({}),
        )
        .await?;
        let body = self
            .get(
                auth,
                "https://c.y.qq.com/rsc/fcgi-bin/fcg_get_profile_homepage.fcg",
                &[
                    ("cid", "205360838".into()),
                    ("userid", auth.uin.clone()),
                    ("reqfrom", "1".into()),
                    ("g_tk", auth.csrf_token()),
                    ("loginUin", auth.uin.clone()),
                    ("hostUin", "0".into()),
                    ("format", "json".into()),
                    ("inCharset", "utf8".into()),
                    ("outCharset", "utf-8".into()),
                    ("platform", "yqq.json".into()),
                    ("needNewCode", "0".into()),
                ],
            )
            .await?;
        let creator = body
            .pointer("/data/creator")
            .ok_or_else(|| AppError::protocol("creator"))?;
        Ok(Profile {
            uin: auth.uin.clone(),
            nickname: text(creator, &["nick", "nickname"])
                .ok_or_else(|| AppError::protocol("nickname"))?,
            avatar: image_url(text(creator, &["headpic"]).unwrap_or_default()),
            offline: false,
            renewable: auth.refresh_param().is_ok(),
        })
    }

    async fn created_playlists(&self, auth: &Credentials) -> Result<Vec<Playlist>> {
        let body = self
            .get(
                auth,
                "https://c.y.qq.com/rsc/fcgi-bin/fcg_user_created_diss",
                &[
                    ("hostUin", "0".into()),
                    ("hostuin", auth.uin.clone()),
                    ("sin", "0".into()),
                    ("size", "200".into()),
                    ("g_tk", auth.csrf_token()),
                    ("loginUin", auth.uin.clone()),
                    ("format", "json".into()),
                    ("inCharset", "utf8".into()),
                    ("outCharset", "utf-8".into()),
                    ("platform", "yqq.json".into()),
                    ("needNewCode", "0".into()),
                ],
            )
            .await?;
        let list = body
            .pointer("/data/disslist")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::protocol("disslist"))?;
        list.iter()
            .filter(|v| number(&v["dirid"]) != Some(202))
            .map(|v| map_playlist(v, "created"))
            .collect()
    }

    async fn collected_playlists(&self, auth: &Credentials) -> Result<Vec<Playlist>> {
        let body = self
            .get(
                auth,
                "https://c.y.qq.com/fav/fcgi-bin/fcg_get_profile_order_asset.fcg",
                &[
                    ("ct", "20".into()),
                    ("cid", "205360956".into()),
                    ("userid", auth.uin.clone()),
                    ("reqtype", "3".into()),
                    ("sin", "0".into()),
                    ("ein", "199".into()),
                    ("format", "json".into()),
                ],
            )
            .await?;
        let list = body
            .pointer("/data/cdlist")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::protocol("cdlist"))?;
        list.iter().map(|v| map_playlist(v, "collected")).collect()
    }

    pub async fn library(&self, auth: &Credentials) -> Result<Library> {
        let (created, collected) =
            tokio::join!(self.created_playlists(auth), self.collected_playlists(auth));
        let mut playlists = created?;
        let mut warnings = Vec::new();
        match collected {
            Ok(items) => playlists.extend(items),
            Err(error) if error.code == "AUTH_EXPIRED" => return Err(error),
            Err(error) => warnings.push(format!("收藏歌单尚未加载：{}", error.message)),
        }
        let mut seen = HashSet::new();
        playlists.retain(|p| seen.insert(p.id.clone()));
        playlists.sort_by_key(|p| p.kind != "liked");
        Ok(Library {
            playlists,
            warnings,
        })
    }

    pub async fn playlist(
        &self,
        auth: &Credentials,
        id: &str,
        offset: u64,
    ) -> Result<PlaylistPage> {
        let liked = id == "liked";
        let diss_id: u64 = if liked {
            0
        } else {
            id.parse()
                .map_err(|_| AppError::new("INVALID_PLAYLIST", "歌单 ID 无效。"))?
        };
        if offset > 100_000 {
            return Err(AppError::new("INVALID_PAGE", "歌单页码超出范围。"));
        }
        let data = self
            .rpc(
                Some(auth),
                auth.comm(),
                "music.srfDissInfo.DissInfo",
                "CgiGetDiss",
                json!({
                    "disstid": diss_id, "dirid": if liked { 201 } else { 0 }, "tag": true,
                    "song_begin": offset, "song_num": PAGE_SIZE, "userinfo": true,
                    "orderlist": true, "onlysonglist": false
                }),
            )
            .await?;
        let songs = data["songlist"]
            .as_array()
            .ok_or_else(|| AppError::protocol("songlist"))?;
        let tracks = songs.iter().map(map_track).collect::<Result<Vec<_>>>()?;
        let total = [
            "/total_song_num",
            "/songlist_size",
            "/totalNum",
            "/dirinfo/total_song_num",
            "/dirinfo/songlist_size",
            "/dirinfo/totalNum",
        ]
        .iter()
        .find_map(|p| data.pointer(p).and_then(number))
        .ok_or_else(|| AppError::protocol("total_song_num"))?;
        let next_offset = offset + songs.len() as u64;
        let has_more = match data.get("hasmore").or_else(|| data.get("hasMore")) {
            Some(Value::Bool(value)) => *value,
            Some(value) => number(value).ok_or_else(|| AppError::protocol("hasmore"))? > 0,
            None => next_offset < total,
        };
        if has_more && songs.is_empty() {
            return Err(AppError::new(
                "PLAYLIST_INCOMPLETE",
                "QQ 音乐未返回完整的歌单，请刷新后重试。",
            ));
        }
        Ok(PlaylistPage {
            tracks,
            total,
            next_offset,
            has_more,
        })
    }

    pub async fn search(&self, auth: &Credentials, keyword: &str, page: u32) -> Result<Vec<Track>> {
        let keyword = keyword.trim();
        if keyword.is_empty() || keyword.chars().count() > 100 || page == 0 || page > 100 {
            return Err(AppError::new(
                "INVALID_SEARCH",
                "请输入 1–100 个字符的搜索内容。",
            ));
        }
        let data = self
            .search_catalog(Some(auth), keyword, 0, page, 30)
            .await?;
        let list = data
            .pointer("/body/song/list")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::protocol("song.list"))?;
        list.iter().map(map_track).collect()
    }

    pub async fn source(
        &self,
        auth: &Credentials,
        track: &Track,
        quality: AudioQuality,
    ) -> Result<PlaybackSource> {
        validate_track(track)?;
        let media_mid = if track.media_mid.is_empty() {
            &track.mid
        } else {
            &track.media_mid
        };
        let formats: &[(&str, &str, &str)] = match quality {
            AudioQuality::Auto => &[("M800", "mp3", "320 kbps"), ("M500", "mp3", "128 kbps")],
            AudioQuality::Standard => &[("M500", "mp3", "128 kbps")],
            AudioQuality::High => &[("M800", "mp3", "320 kbps")],
            AudioQuality::Lossless => &[("F000", "flac", "无损 FLAC")],
            AudioQuality::Hires => &[("RS01", "flac", "Hi-Res FLAC")],
        };
        let files: Vec<String> = formats
            .iter()
            .map(|(prefix, ext, _)| format!("{prefix}{media_mid}.{ext}"))
            .collect();
        let guid = format!(
            "{}",
            uuid::Uuid::new_v4().as_u128() % 90_000_000 + 10_000_000
        );
        let data = self
            .rpc(
                Some(auth),
                auth.comm(),
                "vkey.GetVkeyServer",
                "CgiGetVkey",
                json!({
                    "guid": guid, "songmid": vec![&track.mid; files.len()], "songtype": vec![0; files.len()],
                    "uin": auth.uin, "loginflag": 1, "platform": "20", "filename": files
                }),
            )
            .await?;
        let infos = data["midurlinfo"]
            .as_array()
            .ok_or_else(|| AppError::protocol("midurlinfo"))?;
        for (index, filename) in files.iter().enumerate() {
            if let Some(info) = infos.iter().find(|v| {
                v["filename"].as_str() == Some(filename.as_str())
                    && v["purl"].as_str().is_some_and(|p| !p.is_empty())
            }) {
                let sip = data["sip"]
                    .as_array()
                    .and_then(|s| s.first())
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::protocol("sip"))?;
                let base = url::Url::parse(&image_url(sip.to_string()))
                    .map_err(|_| AppError::protocol("sip"))?;
                let url = base
                    .join(
                        info["purl"]
                            .as_str()
                            .ok_or_else(|| AppError::protocol("purl"))?,
                    )
                    .map_err(|_| AppError::protocol("purl"))?;
                let host = url
                    .host_str()
                    .ok_or_else(|| AppError::protocol("stream host"))?;
                if url.scheme() != "https" || !host.ends_with(".qq.com") {
                    return Err(AppError::new(
                        "INVALID_STREAM",
                        "QQ 音乐返回了不支持的播放地址。",
                    ));
                }
                return Ok(PlaybackSource {
                    url: url.into(),
                    quality: formats[index].2.into(),
                    local: false,
                });
            }
        }
        let code = infos.first().and_then(|v| v["result"].as_i64());
        if code == Some(104003) {
            Err(AppError::new(
                "PLAYBACK_AUTH_REQUIRED",
                "QQ 音乐未授予播放权限。会员或单独购买的歌曲需要对应权益；如果这是免费歌曲，请重新登录。",
            ))
        } else {
            Err(AppError::new(
                "TRACK_UNAVAILABLE",
                "QQ 音乐未提供这首歌的播放地址，可能需要会员、单独购买或当前地区不支持。",
            ))
        }
    }

    pub async fn lyrics(&self, auth: Option<&Credentials>, track: &Track) -> Result<RawLyrics> {
        validate_track(track)?;
        let word = self.rpc(
            None,
            json!({"ct":11,"cv":"1003006","v":"1003006","tmeAppID":"qqmusiclight","uid":"0"}),
            "music.musichallSong.PlayLyricInfo",
            "GetPlayLyricInfo",
            json!({
                "songID":track.id,"crypt":1,"ct":19,"cv":2111,"qrc":1,"qrc_t":0,"lrc_t":0,
                "trans":1,"trans_t":0,"roma":1,"roma_t":0,"type":0
            }),
        );
        let line = self.rpc(
            auth,
            json!({"ct":24,"cv":0}),
            "music.musichallSong.PlayLyricInfo",
            "GetPlayLyricInfo",
            json!({"songMID":track.mid,"songID":track.id}),
        );
        let (word, line) = tokio::join!(word, line);
        if word.is_err() && line.is_err() {
            return Err(line.expect_err("both requests failed"));
        }
        let mut warnings = Vec::new();
        let word = match word {
            Ok(v) => Some(v),
            Err(e) => {
                warnings.push(format!("逐字歌词暂不可用：{}", e.message));
                None
            }
        };
        let line = match line {
            Ok(v) => Some(v),
            Err(e) => {
                warnings.push(format!("逐行歌词暂不可用：{}", e.message));
                None
            }
        };
        let field = |v: Option<&Value>, name: &str| {
            v.and_then(|v| v[name].as_str()).unwrap_or("").to_string()
        };
        let translation = word
            .as_ref()
            .and_then(|v| v["trans"].as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| line.as_ref().and_then(|v| v["trans"].as_str()))
            .unwrap_or("")
            .to_string();
        Ok(RawLyrics {
            word: field(word.as_ref(), "lyric"),
            line: field(line.as_ref(), "lyric"),
            translation,
            romanization: field(word.as_ref(), "roma"),
            warnings,
        })
    }
}

pub(crate) fn check_code(value: &Value) -> Result<()> {
    let code = value["code"]
        .as_i64()
        .ok_or_else(|| AppError::protocol("code"))?;
    match code {
        0 => Ok(()),
        301 | 1000 | 1005 | 20001 | 104400 | 104401 => Err(AppError::new(
            "AUTH_EXPIRED",
            "QQ 音乐登录已失效，请重新连接账户。",
        )),
        _ => Err(AppError::new(
            "QQ_REQUEST_FAILED",
            format!("QQ 音乐暂时无法完成请求（代码 {code}）。"),
        )),
    }
}

pub(crate) fn number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}
pub(crate) fn text(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|f| {
        value.get(f).filter(|v| !v.is_null()).and_then(|v| match v {
            Value::String(s) => Some(s.to_owned()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
    })
}
pub(crate) fn image_url(url: String) -> String {
    if let Some(path) = url.strip_prefix("http://") {
        format!("https://{path}")
    } else if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url
    }
}

pub fn validate_track(track: &Track) -> Result<()> {
    if track.id == 0
        || track.mid.is_empty()
        || track.mid.len() > 64
        || !track.mid.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err(AppError::new(
            "INVALID_TRACK",
            "歌曲标识不完整，请刷新歌单后重试。",
        ));
    }
    if !track.media_mid.bytes().all(|b| b.is_ascii_alphanumeric()) || track.media_mid.len() > 64 {
        return Err(AppError::new("INVALID_TRACK", "歌曲媒体标识无效。"));
    }
    Ok(())
}

pub(crate) fn map_track(raw: &Value) -> Result<Track> {
    let v = raw.get("songInfo").unwrap_or(raw);
    let mid = text(v, &["mid"]).ok_or_else(|| AppError::protocol("song.mid"))?;
    let album_mid = text(&v["album"], &["mid"]).unwrap_or_default();
    let singers = v["singer"]
        .as_array()
        .ok_or_else(|| AppError::protocol("singer"))?;
    let artist = singers
        .iter()
        .map(|s| text(s, &["name"]).ok_or_else(|| AppError::protocol("singer.name")))
        .collect::<Result<Vec<_>>>()?
        .join(" / ");
    Ok(Track {
        id: number(&v["id"]).ok_or_else(|| AppError::protocol("song.id"))?,
        mid,
        media_mid: text(&v["file"], &["media_mid"]).unwrap_or_default(),
        title: text(v, &["title", "name"]).ok_or_else(|| AppError::protocol("song.title"))?,
        artist,
        album: text(&v["album"], &["title", "name"]).unwrap_or_default(),
        cover: if album_mid.is_empty() {
            String::new()
        } else {
            format!("https://y.gtimg.cn/music/photo_new/T002R500x500M000{album_mid}.jpg")
        },
        duration: number(&v["interval"]).ok_or_else(|| AppError::protocol("interval"))?,
        vip: ["pay_play", "pay_month"].iter().any(|key| {
            number(&v["pay"][key]).is_some_and(|value| value > 0)
        }),
        artists: singers
            .iter()
            .map(crate::catalog::map_artist)
            .collect::<Result<_>>()?,
        album_mid,
    })
}

pub(crate) fn map_playlist(v: &Value, kind: &str) -> Result<Playlist> {
    let liked = kind == "created" && number(&v["dirid"]) == Some(201);
    Ok(Playlist {
        id: if liked {
            "liked".into()
        } else {
            text(v, &["dissid", "tid", "diss_id", "id"])
                .ok_or_else(|| AppError::protocol("playlist.id"))?
        },
        name: if liked {
            "我喜欢的音乐".into()
        } else {
            text(v, &["diss_name", "dissname", "name", "title"])
                .ok_or_else(|| AppError::protocol("playlist.name"))?
        },
        cover: image_url(
            text(v, &["diss_cover", "logo", "picurl", "cover", "imgurl"]).unwrap_or_default(),
        ),
        creator: text(&v["creator"], &["name"]),
        count: ["song_cnt", "songnum", "total_song_num", "song_count"]
            .iter()
            .find_map(|k| number(&v[k]))
            .unwrap_or(0),
        kind: if liked { "liked".into() } else { kind.into() },
        dir_id: if liked {
            Some(201)
        } else {
            number(&v["dirid"])
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_error_is_never_an_empty_success() {
        assert_eq!(
            check_code(&json!({"code":1000})).unwrap_err().code,
            "AUTH_EXPIRED"
        );
        assert!(check_code(&json!({})).is_err());
    }
    #[test]
    fn maps_modern_song_fields() {
        let track = map_track(&json!({"id":12,"mid":"abc12","title":"Song","singer":[{"name":"Artist"}],"album":{"mid":"album1","title":"Album"},"file":{"media_mid":"media1"},"interval":180,"pay":{"pay_play":1}})).unwrap();
        assert_eq!(track.duration, 180);
        assert_eq!(track.media_mid, "media1");
        assert!(track.vip);
        let monthly = map_track(&json!({"id":13,"mid":"abc13","title":"Song","singer":[{"name":"Artist"}],"album":{"mid":"album1","title":"Album"},"file":{"media_mid":"media1"},"interval":180,"pay":{"pay_month":1}})).unwrap();
        assert!(monthly.vip);
        assert!(map_track(&json!({"id":12})).is_err());
    }
    #[test]
    fn collected_playlist_201_is_not_my_likes() {
        let collected = map_playlist(
            &json!({"dirid":201,"dissid":"12345","dissname":"Other likes","songnum":4}),
            "collected",
        )
        .unwrap();
        assert_eq!(collected.id, "12345");
        assert_eq!(collected.kind, "collected");
    }
}
