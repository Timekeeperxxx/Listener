use crate::{
    auth::Credentials,
    error::{AppError, Result},
    models::{DailyRecommendation, Playlist, PlaylistPage, Track},
    qq::{image_url, map_track, number, text, validate_track, QqMusic},
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

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
                    kind: "public".into(),
                    dir_id: None,
                    creator: None,
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

impl QqMusic {
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
        let data = self.rpc(Some(auth), auth.comm(), "music.radioProxy.MbTrackRadioSvr", "get_radio_track",
            json!({"id":99,"num":10,"from":0,"scene":0,"song_ids":[],"ext":{"bluetooth":""},"should_count_down":1})).await?;
        radio_tracks(&data)
    }
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
