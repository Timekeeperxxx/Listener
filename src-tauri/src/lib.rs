pub mod auth;
pub mod catalog;
mod cloud;
mod companions;
mod db;
mod desktop;
mod downloads;
pub mod error;
mod login;
mod mobile;
mod mix_audio;
pub mod models;
pub mod qq;
mod recommendations;
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
use tauri::{Emitter, Manager, State, WebviewWindow};

struct SessionState {
    credentials: Option<Credentials>,
    generation: u64,
}
struct AppState {
    qq: qq::QqMusic,
    db: Database,
    session: Mutex<SessionState>,
    login_generation: AtomicU64,
    login: Mutex<Option<login::QrSession>>,
    renewal: tokio::sync::Mutex<()>,
    downloads: Mutex<HashMap<String, Arc<tokio::sync::watch::Sender<bool>>>>,
    download_slots: tokio::sync::Semaphore,
}
struct Snapshot {
    credentials: Credentials,
    generation: u64,
}

impl AppState {
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
    let generation = {
        let mut target = state
            .login
            .lock()
            .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))?;
        let generation = state.login_generation.fetch_add(1, Ordering::SeqCst) + 1;
        *target = None;
        generation
    };
    let (session, image) = login::QrSession::create_for(&kind, &state.qq).await?;
    let mut target = state
        .login
        .lock()
        .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))?;
    if state.login_generation.load(Ordering::SeqCst) != generation {
        return Err(AppError::new("LOGIN_CANCELLED", "本次登录已取消。"));
    }
    *target = Some(session);
    Ok(image)
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
    let login = state
        .login
        .lock()
        .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))?
        .clone();
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
    let profile = state.qq.profile(&credentials).await?;
    state.db.cache(&credentials.uin, "profile", &profile)?;
    {
        let mut login = state
            .login
            .lock()
            .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))?;
        if state.login_generation.load(Ordering::SeqCst) != login_generation
            || login.as_ref().map(|qr| qr.id.as_str()) != Some(id.as_str())
        {
            return Ok(LoginPoll::Closed);
        }
        let mut session = state.session_lock()?;
        if session.generation != session_generation {
            return Err(AppError::new("SESSION_CHANGED", "账户已变化，请重新连接。"));
        }
        auth::save(&credentials)?;
        session.credentials = Some(credentials);
        session.generation += 1;
        *login = None;
    }
    Ok(LoginPoll::Complete { profile })
}

#[tauri::command]
async fn cancel_login(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    main_window(&window)?;
    let mut login = state
        .login
        .lock()
        .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))?;
    state.login_generation.fetch_add(1, Ordering::SeqCst);
    *login = None;
    Ok(())
}

#[tauri::command]
async fn logout(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    main_window(&window)?;
    state.login_generation.fetch_add(1, Ordering::SeqCst);
    *state
        .login
        .lock()
        .map_err(|_| AppError::new("SESSION_ERROR", "登录会话异常，请重新启动应用。"))? = None;
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
) -> Result<CatalogPage<Track>> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    let result = state
        .qq
        .artist_songs(Some(&snapshot.credentials), &mid, offset)
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
    let snapshot = state.snapshot()?;
    qq::validate_track(&track)?;
    let root = window
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| AppError::new("FILE_ERROR", e.to_string()))?;
    if let Some(source) =
        downloads::local_source(&state, &root, &snapshot.credentials.uin, &track.mid)?
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
    let previous = state.db.settings()?;
    desktop::apply_shortcuts(window.app_handle(), &settings.shortcuts)?;
    if let Err(error) = state.db.save_settings(&settings) {
        desktop::apply_shortcuts(window.app_handle(), &previous.shortcuts)?;
        return Err(error);
    }
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
fn get_saved_catalog(window: WebviewWindow, state: State<'_, AppState>) -> Result<SavedCatalog> {
    main_window(&window)?;
    state.db.saved_catalog(&state.snapshot()?.credentials.uin)
}

#[tauri::command]
fn save_catalog_artist(
    window: WebviewWindow,
    state: State<'_, AppState>,
    artist: ArtistRef,
    saved: bool,
) -> Result<SavedCatalog> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    state.db.save_catalog(
        &snapshot.credentials.uin,
        "artist",
        &artist.mid,
        &artist,
        saved,
    )?;
    state.db.saved_catalog(&snapshot.credentials.uin)
}

#[tauri::command]
fn save_catalog_album(
    window: WebviewWindow,
    state: State<'_, AppState>,
    album: CatalogAlbum,
    saved: bool,
) -> Result<SavedCatalog> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    state.db.save_catalog(
        &snapshot.credentials.uin,
        "album",
        &album.mid,
        &album,
        saved,
    )?;
    state.db.saved_catalog(&snapshot.credentials.uin)
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::PendingUpdate::default())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            app.manage(AppState {
                qq: qq::QqMusic::new()?,
                db: Database::open(&app_dir.join("listener.sqlite3"))?,
                session: Mutex::new(SessionState {
                    credentials: None,
                    generation: 0,
                }),
                login_generation: AtomicU64::new(0),
                login: Mutex::new(None),
                renewal: tokio::sync::Mutex::new(()),
                downloads: Mutex::new(HashMap::new()),
                download_slots: tokio::sync::Semaphore::new(2),
            });
            desktop::setup(
                app.handle(),
                &app.state::<AppState>().db.settings()?.shortcuts,
            )?;
            Ok(())
        })
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
        })
        .invoke_handler(tauri::generate_handler![
            get_session,
            renew_session,
            mutate_playlist,
            downloads::get_downloads,
            downloads::download_track,
            downloads::cancel_download,
            downloads::remove_download,
            companions::show_companion,
            companions::companion_action,
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
            desktop::get_desktop_status,
            desktop::hide_main,
            desktop::quit_app,
            updates::check_app_update,
            updates::install_app_update,
            get_search_tracks,
            get_search_playlists,
            get_daily_recommendation,
            get_recommendation_radio,
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
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } if !desktop::is_quitting(app) => {
                api.prevent_exit();
                if let Err(error) = app.emit_to("main", "desktop-action", "quit") {
                    eprintln!("Quit request failed: {error}")
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => {
                if let Err(error) = desktop::show_main(app) {
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
