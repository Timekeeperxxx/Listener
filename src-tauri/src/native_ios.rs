use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{ffi::c_void, path::PathBuf, slice, sync::OnceLock};

struct NativeApp {
    runtime: tokio::runtime::Runtime,
    state: AppState,
    root: PathBuf,
    quick_login: Mutex<Option<QuickLogin>>,
}

struct QuickLogin {
    generation: u64,
    session_generation: u64,
    oauth_state: String,
}

static APP: OnceLock<Result<NativeApp>> = OnceLock::new();

extern "C" {
    fn listener_resume_qq_login(url: *const u8, length: usize);
}

#[derive(Deserialize)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Request {
    Session,
    Logout,
    Qr {
        kind: String,
    },
    Poll {
        id: String,
    },
    CancelLogin,
    QuickLogin,
    FinishQuickLogin {
        callback: String,
        generation: u64,
    },
    SendSms {
        phone: String,
    },
    SmsChallenge {
        generation: u64,
    },
    VerifySms {
        generation: u64,
        code: String,
    },
    Library,
    LikedMids,
    Playlist {
        id: String,
        offset: u64,
    },
    PlaylistChange {
        action: String,
        id: Option<String>,
        name: Option<String>,
        track: Option<Track>,
    },
    Daily {
        refresh: bool,
    },
    Radio { limit: Option<u64> },
    RadioMore,
    Recommendations,
    Charts,
    Search {
        query: String,
        offset: u64,
    },
    SearchPlaylists {
        query: String,
        offset: u64,
    },
    Artists {
        query: String,
        offset: u64,
    },
    Albums {
        query: String,
        offset: u64,
    },
    Artist {
        mid: String,
    },
    ArtistSongs {
        mid: String,
        offset: u64,
        order: ArtistSongOrder,
    },
    ArtistAlbums {
        mid: String,
        offset: u64,
    },
    Album {
        mid: String,
    },
    Saved,
    SaveArtist {
        artist: ArtistRef,
        saved: bool,
    },
    SaveAlbum {
        album: CatalogAlbum,
        saved: bool,
    },
    Source {
        track: Track,
    },
    MixPlan {
        from: Track,
        to: Track,
    },
    Lyrics {
        track: Track,
    },
    History,
    Record {
        track: Track,
    },
    Playback {
        owner: String,
    },
    SavePlayback {
        owner: String,
        playback: PlaybackState,
    },
    Settings,
    SaveSettings {
        settings: NativeSettings,
    },
    Downloads,
    Download {
        track: Track,
    },
    CancelDownload {
        mid: String,
    },
    RemoveDownload {
        mid: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeSettings {
    audio_quality: AudioQuality,
    lyric_offset: i64,
    animated_background: bool,
    lyrics: LyricSettings,
}

type Callback = extern "C" fn(*mut c_void, bool, *const u8, usize);

#[no_mangle]
pub unsafe extern "C" fn listener_native_initialize(root: *const u8, length: usize) {
    let path = slice::from_raw_parts(root, length);
    APP.get_or_init(|| {
        let root = std::str::from_utf8(path)
            .map_err(|_| AppError::new("FILE_ERROR", "本地资料库路径无效。"))?;
        Ok(NativeApp {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .thread_name("listener-core")
                .build()?,
            state: AppState::open(std::path::Path::new(root))?,
            root: root.into(),
            quick_login: Mutex::new(None),
        })
    });
}

#[no_mangle]
pub unsafe extern "C" fn listener_native_request(
    bytes: *const u8,
    length: usize,
    context: *mut c_void,
    reply: Callback,
) {
    let app = match APP.get() {
        Some(Ok(app)) => app,
        Some(Err(error)) => return respond(reply, context, Err(error.clone())),
        None => {
            return respond(
                reply,
                context,
                Err(AppError::new("STARTUP", "资料库尚未初始化。")),
            )
        }
    };
    let input = match serde_json::from_slice::<Request>(slice::from_raw_parts(bytes, length)) {
        Ok(input) => input,
        Err(error) => return respond(reply, context, Err(error.into())),
    };
    let context = context as usize;
    app.runtime.spawn(async move {
        let result = app.dispatch(input).await;
        respond(reply, context as *mut c_void, result);
    });
}

fn respond(reply: Callback, context: *mut c_void, result: Result<Value>) {
    let (ok, bytes) = match result {
        Ok(value) => (true, value.to_string()),
        Err(error) => (false, serde_json::json!(error).to_string()),
    };
    reply(context, ok, bytes.as_ptr(), bytes.len());
}

#[no_mangle]
pub unsafe extern "C" fn listener_native_open_url(bytes: *const u8, length: usize) {
    let Ok(text) = std::str::from_utf8(slice::from_raw_parts(bytes, length)) else { return };
    let Ok(url) = url::Url::parse(text) else { return };
    let Some(destination) = login::qq_resume_url(&url) else { return };
    // Swift calls this on the main actor. WebKit copies the URL before returning.
    listener_resume_qq_login(destination.as_str().as_ptr(), destination.as_str().len());
}

fn json<T: Serialize>(value: T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

impl NativeApp {
    fn quick(&self) -> Result<std::sync::MutexGuard<'_, Option<QuickLogin>>> {
        self.quick_login
            .lock()
            .map_err(|_| AppError::new("SESSION_ERROR", "授权状态异常，请重新启动应用。"))
    }

    async fn dispatch(&self, request: Request) -> Result<Value> {
        let state = &self.state;
        let request = match request {
            Request::Session => return json(get_session_core(state).await?),
            Request::Logout => return json(logout_core(state).await?),
            Request::Qr { kind } => return json(open_login_core(state, kind).await?),
            Request::Poll { id } => return json(poll_login_core(state, id).await?),
            Request::CancelLogin => {
                begin_login(state)?;
                return json(());
            }
            Request::QuickLogin => {
                let generation = begin_login(state)?;
                let oauth_state = uuid::Uuid::new_v4().to_string();
                let url = login::qq_authorize_url(&oauth_state);
                *self.quick()? = Some(QuickLogin {
                    generation,
                    oauth_state,
                    session_generation: state.session_lock()?.generation,
                });
                return Ok(serde_json::json!({"url":url.as_str(), "generation":generation}));
            }
            Request::FinishQuickLogin {
                callback,
                generation,
            } => {
                let pending = {
                    let mut login = self.quick()?;
                    if login
                        .as_ref()
                        .is_none_or(|request| request.generation != generation)
                    {
                        return Err(AppError::new("LOGIN_CANCELLED", "本次授权已取消。"));
                    }
                    login.take().unwrap()
                };
                let credentials =
                    login::authorize_from_callback(&state.qq, &callback, &pending.oauth_state)
                        .await?;
                return json(
                    complete_login(
                        state,
                        generation,
                        pending.session_generation,
                        None,
                        credentials,
                    )
                    .await?,
                );
            }
            Request::Settings => return json(state.db.settings()?),
            Request::SendSms { phone } => {
                return json(sms::send_login_sms_core(state, phone).await?)
            }
            Request::SmsChallenge { generation } => {
                return json(sms::sms_challenge(state, generation)?.as_str())
            }
            Request::VerifySms { generation, code } => {
                return json(sms::verify_login_sms_core(state, generation, code).await?)
            }
            Request::SaveSettings { settings } => {
                let mut value = state.db.settings()?;
                value.audio_quality = settings.audio_quality;
                value.lyric_offset = settings.lyric_offset;
                value.animated_background = settings.animated_background;
                value.lyrics = settings.lyrics;
                return json(state.db.save_settings(&value)?);
            }
            Request::Playback { owner } => {
                verify_playback_owner(&*state.session_lock()?, &owner)?;
                return json(state.db.playback(&owner)?);
            }
            Request::SavePlayback { owner, playback } => {
                let session = state.session_lock()?;
                verify_playback_owner(&session, &owner)?;
                return json(state.db.save_playback(&owner, &playback)?);
            }
            other => other,
        };
        let snapshot = state.snapshot()?;
        let credentials = &snapshot.credentials;
        let auth = Some(credentials);
        let qq = &state.qq;
        let response = match request {
            Request::Library => json(qq.library(credentials).await?),
            Request::LikedMids => {
                let mut mids = std::collections::HashSet::new();
                let mut offset = 0;
                loop {
                    let page = qq.playlist(credentials, "liked", offset).await?;
                    mids.extend(page.tracks.into_iter().map(|track| track.mid));
                    if !page.has_more { break; }
                    state.unchanged(&snapshot)?;
                    offset = page.next_offset;
                }
                json(mids)
            }
            Request::Playlist { id, offset } => json(qq.playlist(credentials, &id, offset).await?),
            Request::PlaylistChange {
                action,
                id,
                name,
                track,
            } => json(mutate_playlist_core(state, action, id, name, track).await?),
            Request::Daily { refresh } => {
                json(get_daily_recommendation_core(state, refresh).await?)
            }
            Request::Radio { limit } => json(qq.recommendation_radio_with_limit(credentials, limit.unwrap_or(10)).await?),
            Request::RadioMore => json(qq.desktop_song_recommendation(credentials).await?.tracks),
            Request::Recommendations => json(qq.recommended_playlists(credentials).await?),
            Request::Charts => json(qq.featured_charts().await?),
            Request::Search { query, offset } => {
                json(qq.search_tracks_page(auth, &query, offset).await?)
            }
            Request::SearchPlaylists { query, offset } => {
                json(qq.search_playlists(auth, &query, offset).await?)
            }
            Request::Artists { query, offset } => {
                json(qq.catalog_artists(auth, &query, offset, -100).await?)
            }
            Request::Albums { query, offset } => {
                json(qq.catalog_albums(auth, &query, offset, 1).await?)
            }
            Request::Artist { mid } => json(qq.artist_detail(auth, &mid).await?),
            Request::ArtistSongs { mid, offset, order } => {
                json(qq.artist_songs(auth, &mid, offset, order).await?)
            }
            Request::ArtistAlbums { mid, offset } => {
                json(qq.artist_albums(auth, &mid, offset).await?)
            }
            Request::Album { mid } => json(qq.album_detail(auth, &mid).await?),
            Request::Saved => json(get_saved_catalog_core(state).await?),
            Request::SaveArtist { artist, saved } => {
                json(save_catalog_artist_core(state, artist, saved).await?)
            }
            Request::SaveAlbum { album, saved } => {
                json(save_catalog_album_core(state, album, saved).await?)
            }
            Request::Source { track } => json(get_source_core(state, &self.root, track).await?),
            Request::MixPlan { from, to } => {
                qq::validate_track(&from)?;
                qq::validate_track(&to)?;
                json(qq.mix_plan(&from.mid, &to.mid).await?)
            }
            Request::Lyrics { track } => {
                let duration = track.duration;
                let offset = state.db.lyric_offset(&credentials.uin, &track.mid)?;
                json(native_lyrics::normalize(
                    get_lyrics_core(state, track).await?,
                    duration,
                    offset,
                )?)
            }
            Request::History => json(state.db.history(&credentials.uin)?),
            Request::Record { track } => {
                qq::validate_track(&track)?;
                json(state.db.record(&credentials.uin, &track)?)
            }
            Request::Downloads => json(state.db.downloads(&credentials.uin)?),
            Request::Download { track } => json(
                downloads::download_track_core(state, &self.root, track, |_, _, _, _| Ok(()))
                    .await?,
            ),
            Request::CancelDownload { mid } => json(downloads::cancel_download_core(state, mid)?),
            Request::RemoveDownload { mid } => {
                json(downloads::remove_download_core(state, &self.root, mid).await?)
            }
            _ => unreachable!("session and settings requests handled above"),
        }?;
        state.unchanged(&snapshot)?;
        Ok(response)
    }
}
