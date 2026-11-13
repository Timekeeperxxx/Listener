pub mod auth;
pub mod catalog;
mod cloud;
#[cfg(desktop)]
mod companions;
mod db;
#[cfg(desktop)]
mod desktop;
mod downloads;
pub mod error;
mod favorites;
mod login;
mod mobile;
mod mix_audio;
#[cfg(all(target_os = "ios", feature = "native-ios"))]
mod native_ios;
#[cfg(feature = "native-ios")]
mod native_lyrics;
pub mod models;
pub mod qq;
mod recommendations;
#[cfg(any(target_os = "ios", test))]
mod sms;
#[cfg(desktop)]
mod updates;
mod user_library;

use auth::Credentials;
use db::Database;
use error::{AppError, Result};
use models::*;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
#[cfg(desktop)]
use tauri::Emitter;
use tauri::{Manager, State, WebviewWindow};

struct SessionState {
    credentials: Option<Credentials>,
    generation: u64,
}
#[derive(Default)]
struct LoginState {
    qr: Option<login::QrSession>,
    #[cfg(target_os = "ios")]
    sms: Option<sms::SmsLogin>,
}
struct AppState {
    qq: qq::QqMusic,
    db: Database,
    session: Mutex<SessionState>,
    login_generation: AtomicU64,
    login: Mutex<LoginState>,
    renewal: tokio::sync::Mutex<()>,
    favorites: tokio::sync::Mutex<()>,
    downloads: Mutex<HashMap<String, Arc<tokio::sync::watch::Sender<bool>>>>,
    download_slots: tokio::sync::Semaphore,
}
struct Snapshot {
    credentials: Credentials,
    generation: u64,
}

impl AppState {
    fn open(app_dir: &std::path::Path) -> Result<Self> {
        std::fs::create_dir_all(app_dir)?;
        Ok(Self {
            qq: qq::QqMusic::new()?,
            db: Database::open(&app_dir.join("listener.sqlite3"))?,
            session: Mutex::new(SessionState { credentials: None, generation: 0 }),
            login_generation: AtomicU64::new(0),
            login: Mutex::new(LoginState::default()),
            renewal: tokio::sync::Mutex::new(()),
            favorites: tokio::sync::Mutex::new(()),
            downloads: Mutex::new(HashMap::new()),
            download_slots: tokio::sync::Semaphore::new(2),
        })
    }

    fn login_lock(&self) -> Result<std::sync::MutexGuard<'_, LoginState>> {
        self.login
            .lock()
            .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))
    }

    fn session_lock(&self) -> Result<std::sync::MutexGuard<'_, SessionState>> {
        self.session
            .lock()
            .map_err(|_| AppError::new("SESSION_ERROR", "账户会话异常，请重新启动应用。"))
    }
    fn snapshot(&self) -> Result<Snapshot> {
        let session = self.session_lock()?;
        let credentials = session
            .credentials
            .clone()
            .ok_or_else(|| AppError::new("LOGIN_REQUIRED", "请先连接 QQ 音乐账户。"))?;
        Ok(Snapshot {
            credentials,
            generation: session.generation,
        })
    }
    fn unchanged(&self, snapshot: &Snapshot) -> Result<()> {
        if self.session_lock()?.generation != snapshot.generation {
            return Err(AppError::new("SESSION_CHANGED", "账户已变化，请重新加载。"));
        }
        Ok(())
    }
}

fn main_window(window: &WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err(AppError::new("FORBIDDEN", "此窗口无权访问本地音乐账户。"));
    }
    Ok(())
}

#[tauri::command]
async fn get_session(window: WebviewWindow, state: State<'_, AppState>) -> Result<Option<Profile>> {
    main_window(&window)?;
    get_session_core(&state).await
}

async fn get_session_core(state: &AppState) -> Result<Option<Profile>> {
    let generation = state.session_lock()?.generation;
    let Some(credentials) = auth::load()? else {
        return Ok(None);
    };
    let original_key = credentials.music_key.clone();
    let checked = state.qq.profile(&credentials).await;
    let (credentials, profile) = match checked {
        Ok(profile) if !credentials.expires_soon() => (credentials, profile),
        result => {
            if result
                .as_ref()
                .err()
                .is_some_and(|e| !matches!(e.code.as_str(), "AUTH_EXPIRED"))
            {
                let error = result.expect_err("checked error");
                if matches!(error.code.as_str(), "NETWORK_ERROR" | "NETWORK_TIMEOUT") {
                    if let Some(mut profile) =
                        state
                            .db
                            .cached::<Profile>(&credentials.uin, "profile", u64::MAX)?
                    {
                        profile.offline = true;
                        (credentials, profile)
                    } else {
                        return Err(error);
                    }
                } else {
                    return Err(error);
                }
            } else {
                let renewed = state.qq.renew(&credentials).await?;
                let profile = state.qq.profile(&renewed).await?;
                (renewed, profile)
            }
        }
    };
    if !profile.offline {
        state.db.cache(&credentials.uin, "profile", &profile)?;
    }
    let mut session = state.session_lock()?;
    if session.generation != generation {
        return Err(AppError::new("SESSION_CHANGED", "账户已变化，请重新加载。"));
    }
    if credentials.music_key != original_key {
        auth::save(&credentials)?;
    }
    session.credentials = Some(credentials);
    session.generation += 1;
    Ok(Some(profile))
}

#[tauri::command]
async fn renew_session(
    window: WebviewWindow,
    state: State<'_, AppState>,
    force: bool,
) -> Result<()> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let _renewal = state.renewal.lock().await;
    state.unchanged(&snapshot)?;
    let latest = state.snapshot()?;
    if latest.credentials.music_key != snapshot.credentials.music_key {
        return Ok(());
    }
    if !force && !latest.credentials.expires_soon() {
        return Ok(());
    }
    let renewed = state.qq.renew(&latest.credentials).await?;
    let mut session = state.session_lock()?;
    if session.generation != snapshot.generation {
        return Err(AppError::new(
            "SESSION_CHANGED",
            "账户已变化，未保存旧登录续期。",
        ));
    }
    auth::save(&renewed)?;
    session.credentials = Some(renewed);
    Ok(())
}
#[tauri::command]
async fn mutate_playlist(
    window: WebviewWindow,
    state: State<'_, AppState>,
    action: String,
    id: Option<String>,
    name: Option<String>,
    track: Option<Track>,
) -> Result<Library> {
    main_window(&window)?;
    mutate_playlist_core(&state, action, id, name, track).await
}

async fn mutate_playlist_core(state: &AppState, action: String, id: Option<String>, name: Option<String>, track: Option<Track>) -> Result<Library> {
    let snapshot = state.snapshot()?;
    let required_id = || {
        id.as_deref()
            .ok_or_else(|| AppError::new("INVALID_PLAYLIST", "请选择歌单。"))
    };
    let required_name = || {
        name.as_deref()
            .ok_or_else(|| AppError::new("INVALID_NAME", "请输入歌单名称。"))
    };
    let result = match action.as_str() {
        "create" => {
            state
                .qq
                .create_playlist(&snapshot.credentials, required_name()?)
                .await?
        }
        "rename" => {
            state
                .qq
                .rename_playlist(&snapshot.credentials, required_id()?, required_name()?)
                .await?
        }
        "delete" => {
            state
                .qq
                .delete_playlist(&snapshot.credentials, required_id()?)
                .await?
        }
        "collect" | "uncollect" => {
            state
                .qq
                .set_favorite_playlist(&snapshot.credentials, required_id()?, action == "collect")
                .await?
        }
        "add" | "remove" => {
            state
                .qq
                .change_playlist_track(
                    &snapshot.credentials,
                    required_id()?,
                    &track.ok_or_else(|| AppError::new("INVALID_TRACK", "请选择歌曲。"))?,
                    action == "add",
                )
                .await?
        }
        _ => return Err(AppError::new("INVALID_OPERATION", "不支持此歌单操作。")),
    };
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn open_login(
    window: WebviewWindow,
    state: State<'_, AppState>,
    kind: String,
) -> Result<login::QrImage> {
    main_window(&window)?;
    open_login_core(&state, kind).await
}

async fn open_login_core(state: &AppState, kind: String) -> Result<login::QrImage> {
    let generation = begin_login(state)?;
    let (session, image) = login::QrSession::create_for(&kind, &state.qq).await?;
    register_login(state, generation, session)?;
    Ok(image)
}

fn begin_login(state: &AppState) -> Result<u64> {
    let mut target = state.login_lock()?;
    let generation = state.login_generation.fetch_add(1, Ordering::SeqCst) + 1;
    *target = LoginState::default();
    Ok(generation)
}

fn register_login(state: &AppState, generation: u64, session: login::QrSession) -> Result<()> {
    let mut target = state.login_lock()?;
    if state.login_generation.load(Ordering::SeqCst) != generation {
        return Err(AppError::new("LOGIN_CANCELLED", "本次登录已取消。"));
    }
    target.qr = Some(session);
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum LoginPoll {
    Waiting,
    Scanned,
    Expired,
    Refused,
    Closed,
    Complete { profile: Profile },
}

#[tauri::command]
async fn poll_login(
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: String,
) -> Result<LoginPoll> {
    main_window(&window)?;
    poll_login_core(&state, id).await
}

async fn poll_login_core(state: &AppState, id: String) -> Result<LoginPoll> {
    let login = state.login_lock()?.qr.clone();
    let Some(login) = login.filter(|login| login.id == id) else {
        return Ok(LoginPoll::Closed);
    };
    let login_generation = state.login_generation.load(Ordering::SeqCst);
    let session_generation = state.session_lock()?.generation;
    let credentials = match login.poll(&state.qq).await? {
        login::QrStatus::Waiting => return Ok(LoginPoll::Waiting),
        login::QrStatus::Scanned => return Ok(LoginPoll::Scanned),
        login::QrStatus::Expired => return Ok(LoginPoll::Expired),
        login::QrStatus::Refused => return Ok(LoginPoll::Refused),
        login::QrStatus::Authorized(credentials) => credentials,
    };
    match complete_login(state, login_generation, session_generation, Some(&id), credentials).await? {
        Some(profile) => Ok(LoginPoll::Complete { profile }),
        None => Ok(LoginPoll::Closed),
    }
}

async fn complete_login(
    state: &AppState,
    login_generation: u64,
    session_generation: u64,
    qr_id: Option<&str>,
    credentials: Credentials,
) -> Result<Option<Profile>> {
    let profile = state.qq.profile(&credentials).await?;
    let mut login = state.login_lock()?;
    if state.login_generation.load(Ordering::SeqCst) != login_generation
        || login.qr.as_ref().map(|qr| qr.id.as_str()) != qr_id
    {
        return Ok(None);
    }
    let mut session = state.session_lock()?;
    if session.generation != session_generation {
        return Err(AppError::new("SESSION_CHANGED", "账户已变化，请重新连接。"));
    }
    state.db.cache(&credentials.uin, "profile", &profile)?;
    auth::save(&credentials)?;
    session.credentials = Some(credentials);
    session.generation += 1;
    *login = LoginState::default();
    Ok(Some(profile))
}

#[tauri::command]
async fn cancel_login(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    main_window(&window)?;
    begin_login(&state)?;
    Ok(())
}

#[tauri::command]
async fn logout(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    main_window(&window)?;
    logout_core(&state).await
}

async fn logout_core(state: &AppState) -> Result<()> {
    let mut login = state.login_lock()?;
    state.login_generation.fetch_add(1, Ordering::SeqCst);
    *login = LoginState::default();
    let mut session = state.session_lock()?;
    auth::delete()?;
    for cancel in state
        .downloads
        .lock()
        .map_err(|_| AppError::new("DOWNLOAD_ERROR", "下载状态异常。"))?
        .values()
    {
        cancel.send_replace(true);
    }
    session.credentials = None;
    session.generation += 1;
    Ok(())
}

#[tauri::command]
async fn get_library(window: WebviewWindow, state: State<'_, AppState>) -> Result<Library> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let library = state.qq.library(&snapshot.credentials).await?;
    state.unchanged(&snapshot)?;
    Ok(library)
}

#[tauri::command]
async fn get_playlist(
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: String,
    offset: u64,
) -> Result<PlaylistPage> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let page = state
        .qq
        .playlist(&snapshot.credentials, &id, offset)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(page)
}

#[tauri::command]
async fn get_catalog_artists(
    window: WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    offset: u64,
    area: i32,
) -> Result<CatalogPage<ArtistRef>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .catalog_artists(Some(&snapshot.credentials), &keyword, offset, area)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_catalog_albums(
    window: WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    offset: u64,
    area: i32,
) -> Result<CatalogPage<CatalogAlbum>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .catalog_albums(Some(&snapshot.credentials), &keyword, offset, area)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_artist_detail(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
) -> Result<ArtistDetail> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .artist_detail(Some(&snapshot.credentials), &mid)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_artist_songs(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
    offset: u64,
    order: ArtistSongOrder,
) -> Result<CatalogPage<Track>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .artist_songs(Some(&snapshot.credentials), &mid, offset, order)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_artist_albums(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
    offset: u64,
) -> Result<CatalogPage<CatalogAlbum>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .artist_albums(Some(&snapshot.credentials), &mid, offset)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_album_meta(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
) -> Result<CatalogAlbum> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .album_meta(Some(&snapshot.credentials), &mid)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_album_detail(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
) -> Result<AlbumDetail> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .album_detail(Some(&snapshot.credentials), &mid)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_catalog_track(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
) -> Result<Track> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .catalog_track(Some(&snapshot.credentials), &mid)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn search_tracks(
    window: WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: u32,
) -> Result<Vec<Track>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let tracks = state
        .qq
        .search(&snapshot.credentials, &keyword, page)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(tracks)
}

#[tauri::command]
async fn get_source(
    window: WebviewWindow,
    state: State<'_, AppState>,
    track: Track,
) -> Result<PlaybackSource> {
    main_window(&window)?;
    let root = window
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| AppError::new("FILE_ERROR", e.to_string()))?;
    get_source_core(&state, &root, track).await
}

async fn get_source_core(state: &AppState, root: &std::path::Path, track: Track) -> Result<PlaybackSource> {
    let snapshot = state.snapshot()?;
    qq::validate_track(&track)?;
    if let Some(source) =
        downloads::local_source(state, root, &snapshot.credentials.uin, &track.mid)?
    {
        return Ok(source);
    }
    let source = state
        .qq
        .source(
            &snapshot.credentials,
            &track,
            state.db.settings()?.audio_quality,
        )
        .await?;
    state.unchanged(&snapshot)?;
    Ok(source)
}

#[tauri::command]
async fn get_lyrics(
    window: WebviewWindow,
    state: State<'_, AppState>,
    track: Track,
) -> Result<RawLyrics> {
    main_window(&window)?;
    get_lyrics_core(&state, track).await
}

async fn get_lyrics_core(state: &AppState, track: Track) -> Result<RawLyrics> {
    qq::validate_track(&track)?;
    let snapshot = state.snapshot()?;
    let cache_key = format!("lyrics:{}", track.id);
    if let Some(lyrics) = state
        .db
        .cached(&snapshot.credentials.uin, &cache_key, 7 * 24 * 3600)?
    {
        return Ok(lyrics);
    }
    let lyrics = state.qq.lyrics(Some(&snapshot.credentials), &track).await?;
    state.unchanged(&snapshot)?;
    if lyrics.warnings.is_empty() && (!lyrics.word.is_empty() || !lyrics.line.is_empty()) {
        state
            .db
            .cache(&snapshot.credentials.uin, &cache_key, &lyrics)?;
    }
    Ok(lyrics)
}

#[tauri::command]
fn get_history(window: WebviewWindow, state: State<'_, AppState>) -> Result<Vec<Track>> {
    main_window(&window)?;
    state.db.history(&state.snapshot()?.credentials.uin)
}

#[tauri::command]
fn record_play(window: WebviewWindow, state: State<'_, AppState>, track: Track) -> Result<()> {
    main_window(&window)?;
    qq::validate_track(&track)?;
    state.db.record(&state.snapshot()?.credentials.uin, &track)
}

#[tauri::command]
fn get_playback(
    window: WebviewWindow,
    state: State<'_, AppState>,
    owner: String,
) -> Result<Option<PlaybackState>> {
    main_window(&window)?;
    let session = state.session_lock()?;
    verify_playback_owner(&session, &owner)?;
    state.db.playback(&owner)
}

#[tauri::command]
fn save_playback(
    window: WebviewWindow,
    state: State<'_, AppState>,
    owner: String,
    playback: PlaybackState,
) -> Result<()> {
    main_window(&window)?;
    let session = state.session_lock()?;
    verify_playback_owner(&session, &owner)?;
    state.db.save_playback(&owner, &playback)
}

fn verify_playback_owner(session: &SessionState, owner: &str) -> Result<()> {
    let credentials = session
        .credentials
        .as_ref()
        .ok_or_else(|| AppError::new("LOGIN_REQUIRED", "请先连接 QQ 音乐账户。"))?;
    if credentials.uin != owner {
        return Err(AppError::new(
            "SESSION_CHANGED",
            "账户已变化，未读写其他账户的播放队列。",
        ));
    }
    Ok(())
}

#[tauri::command]
fn get_settings(window: WebviewWindow, state: State<'_, AppState>) -> Result<Settings> {
    main_window(&window)?;
    state.db.settings()
}

#[tauri::command]
fn save_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<()> {
    main_window(&window)?;
    #[cfg(desktop)]
    {
        let previous = state.db.settings()?;
        desktop::apply_shortcuts(window.app_handle(), &settings.shortcuts)?;
        if let Err(error) = state.db.save_settings(&settings) {
            desktop::apply_shortcuts(window.app_handle(), &previous.shortcuts)?;
            return Err(error);
        }
    }
    #[cfg(mobile)]
    state.db.save_settings(&settings)?;
    if let Some(credentials) = &state.session_lock()?.credentials {
        state.db.prune_cache(
            &credentials.uin,
            u64::from(settings.cache_limit_mb) * 1024 * 1024,
        )?;
    }
    Ok(())
}

#[tauri::command]
async fn get_search_tracks(
    window: WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    offset: u64,
) -> Result<CatalogPage<Track>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .search_tracks_page(Some(&snapshot.credentials), &keyword, offset)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_search_playlists(
    window: WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    offset: u64,
) -> Result<CatalogPage<Playlist>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .search_playlists(Some(&snapshot.credentials), &keyword, offset)
        .await?;
    state.unchanged(&snapshot)?;
    Ok(result)
}

#[tauri::command]
async fn get_daily_recommendation(
    window: WebviewWindow,
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<DailyRecommendation> {
    main_window(&window)?;
    get_daily_recommendation_core(&state, refresh).await
}

async fn get_daily_recommendation_core(state: &AppState, refresh: bool) -> Result<DailyRecommendation> {
    let snapshot = state.snapshot()?;
    let day = recommendations::current_day()?;
    if !refresh {
        if let Some(daily) = state.db.cached::<DailyRecommendation>(
            &snapshot.credentials.uin,
            "recommendations:daily",
            24 * 3600,
        )? {
            if daily.day == day {
                state.unchanged(&snapshot)?;
                return Ok(daily);
            }
        }
    }
    let daily = state.qq.daily_recommendation(&snapshot.credentials).await?;
    state.unchanged(&snapshot)?;
    if daily.day != recommendations::current_day()? {
        return Err(AppError::new("DAILY_CHANGED", "每日推荐已跨日，请刷新。"));
    }
    state
        .db
        .cache(&snapshot.credentials.uin, "recommendations:daily", &daily)?;
    Ok(daily)
}

#[tauri::command]
async fn get_recommendation_radio(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<Track>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let tracks = state.qq.recommendation_radio(&snapshot.credentials).await?;
    state.unchanged(&snapshot)?;
    Ok(tracks)
}

#[tauri::command]
async fn get_recommended_playlists(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<Playlist>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let playlists = state.qq.recommended_playlists(&snapshot.credentials).await?;
    state.unchanged(&snapshot)?;
    Ok(playlists)
}

#[tauri::command]
async fn get_desktop_song_recommendation(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<DesktopSongRecommendation> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let recommendation = state.qq.desktop_song_recommendation(&snapshot.credentials).await?;
    state.unchanged(&snapshot)?;
    Ok(recommendation)
}

#[tauri::command]
async fn get_featured_charts(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<Chart>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let charts = state.qq.featured_charts().await?;
    state.unchanged(&snapshot)?;
    Ok(charts)
}

#[tauri::command]
fn get_search_history(window: WebviewWindow, state: State<'_, AppState>) -> Result<Vec<String>> {
    main_window(&window)?;
    state.db.search_history(&state.snapshot()?.credentials.uin)
}

#[tauri::command]
fn record_search(window: WebviewWindow, state: State<'_, AppState>, keyword: String) -> Result<()> {
    main_window(&window)?;
    state
        .db
        .record_search(&state.snapshot()?.credentials.uin, &keyword)
}

#[tauri::command]
fn clear_search_history(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    main_window(&window)?;
    state
        .db
        .clear_search_history(&state.snapshot()?.credentials.uin)
}

#[tauri::command]
async fn get_saved_catalog(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<SavedCatalog> {
    main_window(&window)?;
    get_saved_catalog_core(&state).await
}

async fn get_saved_catalog_core(state: &AppState) -> Result<SavedCatalog> {
    let _favorites = state.favorites.lock().await;
    let snapshot = state.snapshot()?;
    let catalog = state.qq.favorite_catalog(&snapshot.credentials).await?;
    state.unchanged(&snapshot)?;
    state
        .db
        .replace_saved_catalog(&snapshot.credentials.uin, &catalog)?;
    Ok(catalog)
}

#[tauri::command]
async fn save_catalog_artist(
    window: WebviewWindow,
    state: State<'_, AppState>,
    artist: ArtistRef,
    saved: bool,
) -> Result<SavedCatalog> {
    main_window(&window)?;
    save_catalog_artist_core(&state, artist, saved).await
}

async fn save_catalog_artist_core(state: &AppState, artist: ArtistRef, saved: bool) -> Result<SavedCatalog> {
    let _favorites = state.favorites.lock().await;
    let snapshot = state.snapshot()?;
    state
        .qq
        .set_favorite_artist(&snapshot.credentials, &artist, saved)
        .await?;
    let catalog = state.qq.favorite_catalog(&snapshot.credentials).await?;
    if catalog.artists.iter().any(|item| item.mid == artist.mid) != saved {
        return Err(AppError::new(
            "QQ_WRITE_UNCONFIRMED",
            "QQ 音乐没有确认歌手关注状态，请刷新后重试。",
        ));
    }
    state.unchanged(&snapshot)?;
    state
        .db
        .replace_saved_catalog(&snapshot.credentials.uin, &catalog)?;
    Ok(catalog)
}

#[tauri::command]
async fn save_catalog_album(
    window: WebviewWindow,
    state: State<'_, AppState>,
    album: CatalogAlbum,
    saved: bool,
) -> Result<SavedCatalog> {
    main_window(&window)?;
    save_catalog_album_core(&state, album, saved).await
}

async fn save_catalog_album_core(state: &AppState, album: CatalogAlbum, saved: bool) -> Result<SavedCatalog> {
    let _favorites = state.favorites.lock().await;
    let snapshot = state.snapshot()?;
    state
        .qq
        .set_favorite_album(&snapshot.credentials, &album, saved)
        .await?;
    let catalog = state.qq.favorite_catalog(&snapshot.credentials).await?;
    if catalog.albums.iter().any(|item| item.mid == album.mid) != saved {
        return Err(AppError::new(
            "QQ_WRITE_UNCONFIRMED",
            "QQ 音乐没有确认专辑收藏状态，请刷新后重试。",
        ));
    }
    state.unchanged(&snapshot)?;
    state
        .db
        .replace_saved_catalog(&snapshot.credentials.uin, &catalog)?;
    Ok(catalog)
}

#[tauri::command]
fn get_lyric_offset(window: WebviewWindow, state: State<'_, AppState>, mid: String) -> Result<i64> {
    main_window(&window)?;
    state
        .db
        .lyric_offset(&state.snapshot()?.credentials.uin, &mid)
}

#[tauri::command]
fn save_lyric_offset(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
    offset: i64,
) -> Result<()> {
    main_window(&window)?;
    state
        .db
        .save_lyric_offset(&state.snapshot()?.credentials.uin, &mid, offset)
}

#[tauri::command]
fn get_storage_usage(window: WebviewWindow, state: State<'_, AppState>) -> Result<StorageUsage> {
    main_window(&window)?;
    let path = window
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| AppError::new("FILE_ERROR", e.to_string()))?
        .join("listener.sqlite3");
    let bytes = std::fs::metadata(path)
        .map_err(|e| AppError::new("FILE_ERROR", e.to_string()))?
        .len();
    state
        .db
        .storage_usage(&state.snapshot()?.credentials.uin, bytes)
}

#[tauri::command]
fn clear_lyric_cache(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    main_window(&window)?;
    state
        .db
        .clear_lyric_cache(&state.snapshot()?.credentials.uin)
}

#[cfg(desktop)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::PendingUpdate::default());
    let builder = builder
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            app.manage(AppState::open(&app_dir)?);
            #[cfg(desktop)]
            desktop::setup(
                app.handle(),
                &app.state::<AppState>().db.settings()?.shortcuts,
            )?;
            Ok(())
        });
    #[cfg(desktop)]
    let builder = builder
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                for label in ["mini", "desktop-lyrics"] {
                    if let Some(companion) = window.app_handle().get_webview_window(label) {
                        if let Err(error) = companion.destroy() {
                            eprintln!("Closing {label} failed: {error}");
                        }
                    }
                }
            }
        });
    builder
        .invoke_handler(tauri::generate_handler![
            get_session,
            renew_session,
            mutate_playlist,
            downloads::get_downloads,
            downloads::download_track,
            downloads::cancel_download,
            downloads::remove_download,
            #[cfg(desktop)]
            companions::show_companion,
            #[cfg(desktop)]
            companions::companion_action,
            #[cfg(desktop)]
            companions::publish_companions,
            open_login,
            poll_login,
            cancel_login,
            logout,
            get_library,
            get_playlist,
            search_tracks,
            get_catalog_artists,
            get_catalog_albums,
            get_artist_detail,
            get_artist_songs,
            get_artist_albums,
            get_album_meta,
            get_album_detail,
            get_catalog_track,
            get_source,
            mix_audio::get_qq_mix_plan,
            get_lyrics,
            get_history,
            record_play,
            get_playback,
            save_playback,
            get_settings,
            save_settings,
            #[cfg(desktop)]
            desktop::get_desktop_status,
            #[cfg(desktop)]
            desktop::hide_main,
            #[cfg(desktop)]
            desktop::quit_app,
            #[cfg(desktop)]
            updates::check_app_update,
            #[cfg(desktop)]
            updates::install_app_update,
            get_search_tracks,
            get_search_playlists,
            get_daily_recommendation,
            get_recommendation_radio,
            get_recommended_playlists,
            get_desktop_song_recommendation,
            get_featured_charts,
            get_search_history,
            record_search,
            clear_search_history,
            get_saved_catalog,
            save_catalog_artist,
            save_catalog_album,
            get_lyric_offset,
            save_lyric_offset,
            get_storage_usage,
            clear_lyric_cache
        ])
        .build(tauri::generate_context!())
        .expect("Listener could not start")
        .run(|_app, event| match event {
            #[cfg(desktop)]
            tauri::RunEvent::ExitRequested { api, .. } if !desktop::is_quitting(_app) => {
                api.prevent_exit();
                if let Err(error) = _app.emit_to("main", "desktop-action", "quit") {
                    eprintln!("Quit request failed: {error}")
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => {
                if let Err(error) = desktop::show_main(_app) {
                    eprintln!("Reopen failed: {error:?}")
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_requires_the_current_account() {
        let mut session = SessionState {
            credentials: None,
            generation: 0,
        };
        assert_eq!(
            verify_playback_owner(&session, "123").unwrap_err().code,
            "LOGIN_REQUIRED"
        );
        session.credentials = Some(Credentials {
            cookie: String::new(),
            uin: "123".into(),
            music_key: String::new(),
            login_type: 2,
            refresh: None,
        });
        assert!(verify_playback_owner(&session, "123").is_ok());
        assert_eq!(
            verify_playback_owner(&session, "456").unwrap_err().code,
            "SESSION_CHANGED"
        );
    }
}
