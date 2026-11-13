use crate::{
    auth::Credentials,
    error::{AppError, Result},
    models::{Chart, ChartSong, DailyRecommendation, DesktopSongRecommendation, Playlist, PlaylistPage, Track},
    qq::{image_url, map_track, number, text, validate_track, QqMusic},
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

const TOPLIST: &str = "music.musicToplist.Toplist";

pub(crate) fn qq_day(seconds: u64) -> u64 {
    (seconds + 8 * 3600) / 86400
}

pub(crate) fn current_day() -> Result<u64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::new("CLOCK_ERROR", "系统时间异常，无法读取每日推荐。"))?
        .as_secs();
    Ok(qq_day(seconds))
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value[field]
        .as_array()
        .ok_or_else(|| AppError::protocol(field))
}

fn daily_card(feed: &Value, day: u64) -> Result<Playlist> {
    let retcode = feed["retcode"]
        .as_i64()
        .ok_or_else(|| AppError::protocol("retcode"))?;
    if retcode != 0 {
        return Err(AppError::new(
            "QQ_REQUEST_FAILED",
            format!("QQ 音乐暂时无法读取推荐（代码 {retcode}）。"),
        ));
    }
    let mut daily = None;
    for shelf in array(feed, "v_shelf")? {
        for niche in array(shelf, "v_niche")? {
            for card in array(niche, "v_card")? {
                // Other cards include public playlists and radio. Only subtype 510 is the daily mix.
                if number(&card["subtype"]) != Some(510) {
                    continue;
                }
                if daily.is_some() {
                    return Err(AppError::new(
                        "QQ_RESPONSE_CHANGED",
                        "QQ 音乐返回了多份每日推荐，无法确定当前歌单。",
                    ));
                }
                let id = number(&card["id"])
                    .filter(|id| *id > 0)
                    .ok_or_else(|| AppError::protocol("daily.id"))?;
                let time = number(&card["time"]).ok_or_else(|| AppError::protocol("daily.time"))?;
                if qq_day(time) != day {
                    return Err(AppError::new(
                        "DAILY_NOT_UPDATED",
                        "QQ 音乐尚未更新今天的每日推荐，请稍后刷新。",
                    ));
                }
                daily = Some(Playlist {
                    id: id.to_string(),
                    name: text(card, &["title"])
                        .ok_or_else(|| AppError::protocol("daily.title"))?,
                    cover: image_url(
                        text(card, &["cover"]).ok_or_else(|| AppError::protocol("daily.cover"))?,
                    ),
                    count: 0, // Replaced with the actual playlist total after reading the songs.
                    play_count: None,
                    kind: "public".into(),
                    dir_id: None,
                    creator: None,
                    activity_time: None,
                });
            }
        }
    }
    daily.ok_or_else(|| {
        AppError::new(
            "DAILY_UNAVAILABLE",
            "QQ 音乐暂未返回此账户的每日推荐，请稍后刷新。",
        )
    })
}

fn complete_daily(
    mut playlist: Playlist,
    page: PlaylistPage,
    day: u64,
) -> Result<DailyRecommendation> {
    if page.has_more || page.total == 0 || page.total != page.tracks.len() as u64 {
        return Err(AppError::new(
            "DAILY_INCOMPLETE",
            "QQ 音乐未返回完整的每日推荐，请刷新重试。",
        ));
    }
    let mut mids = HashSet::new();
    for track in &page.tracks {
        validate_track(track)?;
        if !mids.insert(&track.mid) {
            return Err(AppError::new(
                "QQ_RESPONSE_CHANGED",
                "QQ 音乐的每日推荐包含重复歌曲，请刷新重试。",
            ));
        }
    }
    playlist.count = page.total;
    Ok(DailyRecommendation {
        playlist,
        tracks: page.tracks,
        day,
    })
}

fn radio_tracks(data: &Value) -> Result<Vec<Track>> {
    let mut mids = HashSet::new();
    let mut tracks = Vec::new();
    for raw in array(data, "tracks")? {
        let track = map_track(raw)?;
        validate_track(&track)?;
        if mids.insert(track.mid.clone()) {
            tracks.push(track);
        }
    }
    if tracks.is_empty() {
        return Err(AppError::new(
            "RADIO_UNAVAILABLE",
            "QQ 音乐暂未返回猜你喜欢，请稍后重试。",
        ));
    }
    Ok(tracks)
}

fn desktop_cards(shelf: &Value) -> Result<Vec<&Value>> {
    let mut cards = Vec::new();
    for niche in array(shelf, "v_niche")? {
        cards.extend(array(niche, "v_card")?);
    }
    if cards.is_empty() {
        return Err(AppError::protocol("v_card"));
    }
    Ok(cards)
}

fn desktop_playlist_cards(shelf: &Value) -> Result<Vec<Playlist>> {
    desktop_cards(shelf)?
        .into_iter()
        .map(|card| {
            let id = number(&card["id"])
                .filter(|id| *id > 0)
                .ok_or_else(|| AppError::protocol("recommendation.playlist.id"))?;
            Ok(Playlist {
                id: id.to_string(),
                name: text(card, &["title"])
                    .ok_or_else(|| AppError::protocol("recommendation.playlist.title"))?,
                cover: image_url(text(card, &["cover"]).unwrap_or_default()),
                count: 0, // The desktop shelf carries play count. Song count is read in the detail view.
                play_count: number(&card["cnt"]),
                creator: None,
                activity_time: None,
                kind: "public".into(),
                dir_id: None,
            })
        })
        .collect()
}

impl QqMusic {
    async fn desktop_shelf(
        &self,
        auth: &Credentials,
        comm: Value,
        matches: impl Fn(&Value) -> bool,
    ) -> Result<Option<Value>> {
        self.rpc(
            Some(auth),
            auth.comm(),
            "music.UserInfo.userInfoServer",
            "GetLoginUserInfo",
            json!({}),
        )
        .await?;

        let mut loaded = 0;
        for page in 1..=2 {
            let feed = self
                .rpc(
                    Some(auth),
                    comm.clone(),
                    "music.recommend.RecommendFeed",
                    "get_recommend_feed",
                    json!({"direction":page - 1,"page":page,"v_cache":[],"v_uniq":[],"s_num":loaded}),
                )
                .await?;
            let retcode = number(&feed["retcode"]).ok_or_else(|| AppError::protocol("retcode"))?;
            if retcode != 0 {
                return Err(AppError::new(
                    "QQ_REQUEST_FAILED",
                    format!("QQ 音乐暂时无法读取桌面推荐（代码 {retcode}）。"),
                ));
            }
            let shelves = array(&feed, "v_shelf")?;
            if let Some(shelf) = shelves.iter().find(|shelf| matches(shelf)) {
                return Ok(Some(shelf.clone()));
            }
            if shelves.is_empty() {
                break;
            }
            loaded += shelves.len();
        }
        Ok(None)
    }

    pub async fn recommended_playlists(&self, auth: &Credentials) -> Result<Vec<Playlist>> {
        let mut comm = auth.comm();
        comm["ct"] = json!(20);
        comm["cv"] = json!(1770);
        let shelf = self
            .desktop_shelf(auth, comm, |item| number(&item["id"]) == Some(271))
            .await?
            .ok_or_else(|| {
                AppError::new("RECOMMENDATIONS_UNAVAILABLE", "QQ 音乐暂未返回歌单补给站。")
            })?;
        desktop_playlist_cards(&shelf)
    }

    pub async fn desktop_song_recommendation(
        &self,
        auth: &Credentials,
    ) -> Result<DesktopSongRecommendation> {
        let mut comm = auth.comm();
        comm["ct"] = json!(11);
        comm["cv"] = json!(14090008);
        let shelf = self
            .desktop_shelf(auth, comm.clone(), |item| number(&item["id"]) == Some(207))
            .await?
            .ok_or_else(|| {
                AppError::new(
                    "RECOMMENDATIONS_UNAVAILABLE",
                    "QQ 音乐暂未返回桌面推荐歌曲。",
                )
            })?;
        let title = text(&shelf, &["title_template", "title_content"])
            .ok_or_else(|| AppError::protocol("recommendation.title"))?;

        let mut ids = Vec::new();
        let mut types = Vec::new();
        let mut seen = HashSet::new();
        for card in desktop_cards(&shelf)? {
            let id = number(&card["id"])
                .filter(|id| *id > 0)
                .ok_or_else(|| AppError::protocol("recommendation.song.id"))?;
            if seen.insert(id) {
                ids.push(id);
                types.push(
                    number(&card["type"])
                        .ok_or_else(|| AppError::protocol("recommendation.song.type"))?,
                );
            }
        }
        let details = self
            .rpc(
                Some(auth),
                comm,
                "music.trackInfo.UniformRuleCtrl",
                "CgiGetTrackInfo",
                json!({"ids":ids,"types":types,"source":"AiNoFree"}),
            )
            .await?;
        let mut tracks = HashMap::new();
        for raw in array(&details, "tracks")? {
            let track = map_track(raw)?;
            validate_track(&track)?;
            tracks.insert(track.id, track);
        }
        let tracks = ids
            .into_iter()
            .map(|id| {
                tracks
                    .remove(&id)
                    .ok_or_else(|| AppError::protocol("recommendation.song.details"))
            })
            .collect::<Result<_>>()?;
        Ok(DesktopSongRecommendation { title, tracks })
    }

    pub async fn featured_charts(&self) -> Result<Vec<Chart>> {
        let data = self
            .rpc(None, recommendation_comm(), TOPLIST, "GetAll", json!({}))
            .await?;
        let group = array(&data, "group")?
            .iter()
            .find(|group| group["groupName"] == "巅峰榜")
            .ok_or_else(|| AppError::protocol("巅峰榜"))?;
        let charts = array(group, "toplist")?;
        if charts.is_empty() {
            return Err(AppError::new(
                "CHARTS_UNAVAILABLE",
                "QQ 音乐暂未返回排行榜。",
            ));
        }
        charts
            .iter()
            .take(3)
            .map(|item| {
                let id = number(&item["topId"])
                    .filter(|id| *id > 0)
                    .ok_or_else(|| AppError::protocol("chart.topId"))?;
                let preview = array(item, "song")?
                    .iter()
                    .take(3)
                    .map(|song| {
                        Ok(ChartSong {
                            title: text(song, &["title"])
                                .ok_or_else(|| AppError::protocol("chart.song.title"))?,
                            artist: text(song, &["singerName"]).unwrap_or_default(),
                        })
                    })
                    .collect::<Result<_>>()?;
                Ok(Chart {
                    playlist: Playlist {
                        id: format!("chart:{id}"),
                        name: text(item, &["title"])
                            .ok_or_else(|| AppError::protocol("chart.title"))?,
                        cover: image_url(
                            text(item, &["frontPicUrl", "headPicUrl"]).unwrap_or_default(),
                        ),
                        count: number(&item["totalNum"])
                            .ok_or_else(|| AppError::protocol("chart.totalNum"))?,
                        play_count: None,
                        kind: "chart".into(),
                        dir_id: None,
                        creator: None,
                        activity_time: None,
                    },
                    preview,
                })
            })
            .collect()
    }

    pub(crate) async fn chart_page(
        &self,
        id: u64,
        offset: u64,
        limit: u64,
    ) -> Result<PlaylistPage> {
        let data = self
            .rpc(
                None,
                recommendation_comm(),
                TOPLIST,
                "GetDetail",
                json!({"topId":id,"offset":offset,"num":limit}),
            )
            .await?;
        if number(&data["data"]["topId"]) != Some(id) {
            return Err(AppError::protocol("chart.topId"));
        }
        let total = number(&data["data"]["totalNum"])
            .ok_or_else(|| AppError::protocol("chart.totalNum"))?;
        let tracks = array(&data, "songInfoList")?
            .iter()
            .map(map_track)
            .collect::<Result<Vec<_>>>()?;
        let next_offset = offset + tracks.len() as u64;
        let has_more = next_offset < total;
        if tracks.is_empty() && (offset == 0 || has_more) {
            return Err(AppError::new(
                "CHART_INCOMPLETE",
                "QQ 音乐未返回排行榜歌曲，请刷新后重试。",
            ));
        }
        Ok(PlaylistPage {
            tracks,
            total,
            next_offset,
            has_more,
            dates: Default::default(),
        })
    }

    pub async fn daily_recommendation(&self, auth: &Credentials) -> Result<DailyRecommendation> {
        let day = current_day()?;
        // The feed also returns code 0 anonymously. Verify authentication before reading personal cards.
        self.rpc(
            Some(auth),
            auth.comm(),
            "music.UserInfo.userInfoServer",
            "GetLoginUserInfo",
            json!({}),
        )
        .await?;
        let feed = self
            .rpc(
                Some(auth),
                auth.comm(),
                "music.recommend.RecommendFeed",
                "get_recommend_feed",
                json!({"direction":0,"page":1,"v_cache":[],"v_uniq":[],"s_num":0}),
            )
            .await?;
        let playlist = daily_card(&feed, day)?;
        let page = self.playlist(auth, &playlist.id, 0).await?;
        if current_day()? != day {
            return Err(AppError::new("DAILY_CHANGED", "每日推荐已跨日，请刷新。"));
        }
        complete_daily(playlist, page, day)
    }

    pub async fn recommendation_radio(&self, auth: &Credentials) -> Result<Vec<Track>> {
        self.recommendation_radio_with_limit(auth, 10).await
    }

    pub async fn recommendation_radio_with_limit(&self, auth: &Credentials, limit: u64) -> Result<Vec<Track>> {
        let data = self.rpc(Some(auth), auth.comm(), "music.radioProxy.MbTrackRadioSvr", "get_radio_track",
            json!({"id":99,"num":limit.clamp(1, 30),"from":0,"scene":0,"song_ids":[],"ext":{"bluetooth":""},"should_count_down":1})).await?;
        radio_tracks(&data)
    }
}

fn recommendation_comm() -> Value {
    json!({"ct":24,"cv":4747474,"uin":0,"format":"json","platform":"yqq.json"})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: Value, subtype: u64, time: u64) -> Value {
        json!({"id":id,"subtype":subtype,"time":time,"title":"每日30首","cover":"http://y.gtimg.cn/daily.jpg"})
    }
    fn feed(cards: Vec<Value>) -> Value {
        json!({"retcode":0,"v_shelf":[{"v_niche":[{"v_card":cards}]}]})
    }
    fn song(id: u64) -> Value {
        json!({"id":id,"mid":format!("mid{id}"),"title":"Song","interval":180,"singer":[{"mid":"artist1","name":"Artist"}],"album":{"mid":"album1","title":"Album"},"file":{"media_mid":format!("media{id}")}})
    }
    #[test]
    fn day_rolls_over_at_midnight_in_shanghai() {
        assert_eq!(qq_day(57599), 0);
        assert_eq!(qq_day(57600), 1);
    }
    #[test]
    fn selects_the_current_accounts_daily_card_instead_of_public_or_radio_cards() {
        let playlist = daily_card(
            &feed(vec![
                card(json!("123"), 0, 57600),
                card(json!("456"), 510, 57600),
                card(json!("radio"), 950, 0),
            ]),
            1,
        )
        .unwrap();
        assert_eq!(playlist.id, "456");
        assert_eq!(playlist.cover, "https://y.gtimg.cn/daily.jpg");
    }
    #[test]
    fn does_not_substitute_public_feed_for_missing_daily_mix() {
        assert_eq!(
            daily_card(&feed(vec![card(json!("123"), 0, 57600)]), 1)
                .unwrap_err()
                .code,
            "DAILY_UNAVAILABLE"
        );
        assert_eq!(
            daily_card(&json!({"retcode":0}), 1).unwrap_err().code,
            "QQ_RESPONSE_CHANGED"
        );
    }
    #[test]
    fn rejects_outdated_ambiguous_or_invalid_daily_cards() {
        assert_eq!(
            daily_card(&feed(vec![card(json!("123"), 510, 57599)]), 1)
                .unwrap_err()
                .code,
            "DAILY_NOT_UPDATED"
        );
        assert_eq!(
            daily_card(
                &feed(vec![
                    card(json!("123"), 510, 57600),
                    card(json!("456"), 510, 57600)
                ]),
                1
            )
            .unwrap_err()
            .code,
            "QQ_RESPONSE_CHANGED"
        );
        assert_eq!(
            daily_card(&feed(vec![card(json!("invalid"), 510, 57600)]), 1)
                .unwrap_err()
                .code,
            "QQ_RESPONSE_CHANGED"
        );
        assert_eq!(
            daily_card(&json!({"retcode":7}), 1).unwrap_err().code,
            "QQ_REQUEST_FAILED"
        );
    }
    #[test]
    fn rejects_incomplete_or_empty_daily_playlists() {
        let playlist = daily_card(&feed(vec![card(json!("123"), 510, 57600)]), 1).unwrap();
        let page = |total, has_more| PlaylistPage {
            tracks: vec![],
            total,
            next_offset: 0,
            has_more,
            dates: Default::default(),
        };
        assert!(complete_daily(playlist.clone(), page(30, true), 1).is_err());
        assert!(complete_daily(playlist, page(0, false), 1).is_err());
    }
    #[test]
    fn preserves_daily_order_and_real_catalog_identity() {
        let playlist = daily_card(&feed(vec![card(json!("123"), 510, 57600)]), 1).unwrap();
        let tracks = vec![map_track(&song(2)).unwrap(), map_track(&song(1)).unwrap()];
        let result = complete_daily(
            playlist,
            PlaylistPage {
                tracks,
                total: 2,
                next_offset: 2,
                has_more: false,
                dates: Default::default(),
            },
            1,
        )
        .unwrap();
        assert_eq!(result.playlist.count, 2);
        assert_eq!(result.tracks[0].id, 2);
        assert_eq!(result.tracks[0].album_mid, "album1");
        assert_eq!(result.tracks[0].artists[0].mid, "artist1");
    }
    #[test]
    fn radio_deduplicates_in_order_and_never_hides_missing_data() {
        let tracks = radio_tracks(&json!({"tracks":[song(2),song(2),song(1)]})).unwrap();
        assert_eq!(tracks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(
            radio_tracks(&json!({"tracks":[]})).unwrap_err().code,
            "RADIO_UNAVAILABLE"
        );
        assert_eq!(
            radio_tracks(&json!({"tracks":null})).unwrap_err().code,
            "QQ_RESPONSE_CHANGED"
        );
        assert!(radio_tracks(&json!({"tracks":[{"id":1}]})).is_err());
    }
}
