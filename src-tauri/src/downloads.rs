use crate::{
    error::{AppError, Result},
    main_window,
    models::{Download, Track},
    AppState,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{Emitter, Manager, State, WebviewWindow};
use tokio::io::AsyncWriteExt;

fn stored_path(root: &Path, owner: &str, filename: &str) -> Result<PathBuf> {
    if filename.is_empty()
        || !filename
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        || filename.contains("..")
    {
        return Err(AppError::new(
            "INVALID_DOWNLOAD",
            "本地下载记录中的文件名无效。",
        ));
    }
    Ok(root.join("downloads").join(owner).join(filename))
}
#[tauri::command]
pub fn get_downloads(window: WebviewWindow, state: State<'_, AppState>) -> Result<Vec<Download>> {
    main_window(&window)?;
    state.db.downloads(&state.snapshot()?.credentials.uin)
}
#[tauri::command]
pub fn cancel_download(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
) -> Result<()> {
    main_window(&window)?;
    cancel_download_core(&state, mid)
}

pub(crate) fn cancel_download_core(state: &AppState, mid: String) -> Result<()> {
    let owner = state.snapshot()?.credentials.uin;
    if let Some(cancel) = state
        .downloads
        .lock()
        .map_err(|_| AppError::new("DOWNLOAD_ERROR", "下载状态异常。"))?
        .get(&format!("{owner}:{mid}"))
    {
        cancel.send_replace(true);
    }
    Ok(())
}
#[tauri::command]
pub async fn download_track(
    window: WebviewWindow,
    state: State<'_, AppState>,
    track: Track,
) -> Result<Download> {
    main_window(&window)?;
    let root = window
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| AppError::new("FILE_ERROR", e.to_string()))?;
    download_track_core(&state, &root, track, |owner, mid, received, total| {
        window.emit(
            "download-progress",
            serde_json::json!({"owner":owner,"mid":mid,"received":received,"total":total}),
        )?;
        Ok(())
    }).await
}

pub(crate) async fn download_track_core(
    state: &AppState,
    root: &Path,
    track: Track,
    progress: impl Fn(&str, &str, u64, Option<u64>) -> Result<()> + Send,
) -> Result<Download> {
    crate::qq::validate_track(&track)?;
    let snapshot = state.snapshot()?;
    let owner = &snapshot.credentials.uin;
    if state
        .db
        .downloads(owner)?
        .iter()
        .any(|d| d.track.mid == track.mid)
    {
        return Err(AppError::new(
            "DOWNLOAD_EXISTS",
            "这首歌已下载；更换音质请先移除本地下载。",
        ));
    }
    let token = uuid::Uuid::new_v4().to_string();
    let part = stored_path(root, owner, &format!("{token}.part"))?;
    let key = format!("{owner}:{}", track.mid);
    let (sender, receiver) = tokio::sync::watch::channel(false);
    let cancel = Arc::new(sender);
    {
        let mut tasks = state
            .downloads
            .lock()
            .map_err(|_| AppError::new("DOWNLOAD_ERROR", "下载状态异常。"))?;
        if tasks.contains_key(&key) {
            return Err(AppError::new("DOWNLOAD_EXISTS", "这首歌已在下载队列中。"));
        }
        tasks.insert(key.clone(), cancel.clone());
    }
    let mut target_path = None;
    let result = cancellable_download(receiver, async {
        let _permit = state
            .download_slots
            .acquire()
            .await
            .map_err(|_| AppError::new("DOWNLOAD_ERROR", "下载队列已关闭。"))?;
        let source = state
            .qq
            .source(
                &snapshot.credentials,
                &track,
                state.db.settings()?.audio_quality,
            )
            .await?;
        let ext = if source.quality.contains("FLAC") {
            "flac"
        } else {
            "mp3"
        };
        let filename = format!("{token}.{ext}");
        let target = stored_path(root, owner, &filename)?;
        target_path = Some(target.clone());
        tokio::fs::create_dir_all(part.parent().expect("download parent")).await?;
        let mut response = state
            .qq
            .client
            .get(&source.url)
            .timeout(std::time::Duration::from_secs(1800))
            .send()
            .await?
            .error_for_status()?;
        let total = response.content_length();
        let mut file = tokio::fs::File::create(&part).await?;
        let mut received = 0u64;
        let mut prefix = Vec::new();
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        while let Some(chunk) = response.chunk().await? {
            if prefix.len() < 16 {
                prefix.extend(chunk.iter().take(16 - prefix.len()));
            }
            file.write_all(&chunk).await?;
            received += chunk.len() as u64;
            if last.elapsed() >= std::time::Duration::from_millis(250) {
                progress(owner, &track.mid, received, total)?;
                last = std::time::Instant::now();
            }
        }
        file.sync_all().await?;
        if received == 0
            || total.is_some_and(|size| size != received)
            || !valid_audio_prefix(&prefix, ext)
        {
            return Err(AppError::new(
                "DOWNLOAD_INVALID",
                "下载内容不完整或不是可播放的音频，未加入离线曲库。",
            ));
        }
        state.unchanged(&snapshot)?;
        let download = Download {
            track,
            quality: source.quality,
            filename,
            bytes: received,
        };
        tokio::fs::rename(&part, &target).await?;
        let committed = {
            let session = state.session_lock()?;
            if session.generation != snapshot.generation { Err(AppError::new("SESSION_CHANGED","账户已变化，下载未加入曲库。")) }
            else { state.db.save_download(owner, &download) }
        };
        if let Err(error) = committed {
            tokio::fs::remove_file(&target).await?;
            return Err(error);
        }
        Ok(download)
    }).await;
    state
        .downloads
        .lock()
        .map_err(|_| AppError::new("DOWNLOAD_ERROR", "下载状态异常。"))?
        .remove(&key);
    if part.try_exists()? {
        tokio::fs::remove_file(part).await?;
    }
    if result.is_err() {
        if let Some(path) = target_path {
            if path.try_exists()? {
                tokio::fs::remove_file(path).await?;
            }
        }
    }
    result
}
async fn cancellable_download(
    mut receiver: tokio::sync::watch::Receiver<bool>,
    operation: impl std::future::Future<Output = Result<Download>>,
) -> Result<Download> {
    let cancelled = || AppError::new("DOWNLOAD_CANCELLED", "下载已取消。");
    if *receiver.borrow() {
        return Err(cancelled());
    }
    tokio::select! {
        biased;
        changed = receiver.changed() => {
            changed.map_err(|_|AppError::new("DOWNLOAD_ERROR","下载取消通道已关闭。"))?;
            Err(cancelled())
        },
        result = operation => result,
    }
}
#[tauri::command]
pub async fn remove_download(
    window: WebviewWindow,
    state: State<'_, AppState>,
    mid: String,
) -> Result<()> {
    main_window(&window)?;
    let root = window
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| AppError::new("FILE_ERROR", e.to_string()))?;
    remove_download_core(&state, &root, mid).await
}

pub(crate) async fn remove_download_core(state: &AppState, root: &Path, mid: String) -> Result<()> {
    let snapshot = state.snapshot()?;
    let record = state
        .db
        .downloads(&snapshot.credentials.uin)?
        .into_iter()
        .find(|d| d.track.mid == mid)
        .ok_or_else(|| AppError::new("DOWNLOAD_MISSING", "没有找到这首歌的本地下载。"))?;
    let path = stored_path(root, &snapshot.credentials.uin, &record.filename)?;
    if path.try_exists()? {
        tokio::fs::remove_file(path).await?;
    }
    state.unchanged(&snapshot)?;
    state.db.remove_download(&snapshot.credentials.uin, &mid)
}
pub(crate) fn local_source(
    state: &AppState,
    root: &Path,
    owner: &str,
    mid: &str,
) -> Result<Option<crate::models::PlaybackSource>> {
    let Some(record) = state
        .db
        .downloads(owner)?
        .into_iter()
        .find(|d| d.track.mid == mid)
    else {
        return Ok(None);
    };
    let path = stored_path(root, owner, &record.filename)?;
    if !path.try_exists()? {
        return Err(AppError::new(
            "OFFLINE_FILE_MISSING",
            "本地音频已被移走，请移除下载记录后重新下载。",
        ));
    }
    Ok(Some(crate::models::PlaybackSource {
        url: path.to_string_lossy().into_owned(),
        quality: format!("{} · 本地", record.quality),
        local: true,
    }))
}
fn valid_audio_prefix(bytes: &[u8], ext: &str) -> bool {
    if ext == "flac" {
        bytes.starts_with(b"fLaC")
    } else {
        bytes.starts_with(b"ID3")
            || (bytes.len() > 1 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancel_before_a_queued_download_does_not_start_io() {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        sender.send_replace(true);
        let mut started = false;
        let result = cancellable_download(receiver, async {
            started = true;
            std::future::pending().await
        })
        .await;
        assert_eq!(result.unwrap_err().code, "DOWNLOAD_CANCELLED");
        assert!(!started);
    }
    #[tokio::test]
    async fn cancel_interrupts_stalled_work_without_waiting_for_another_chunk() {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        let task = tokio::spawn(cancellable_download(receiver, std::future::pending()));
        sender.send_replace(true);
        let result = tokio::time::timeout(std::time::Duration::from_millis(100), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.unwrap_err().code, "DOWNLOAD_CANCELLED");
    }
    #[test]
    fn rejects_non_audio_and_traversal() {
        assert!(valid_audio_prefix(b"fLaC123", "flac"));
        assert!(!valid_audio_prefix(b"<html>error", "mp3"));
        assert!(stored_path(Path::new("/tmp"), "123", "../secret").is_err());
    }
}
