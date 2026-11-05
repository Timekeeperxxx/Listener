use crate::{
    error::{AppError, Result},
    main_window,
};
use std::time::Duration;
use tauri::{Emitter, Manager, State, WebviewWindow};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct PendingUpdate(pub tokio::sync::Mutex<Option<Update>>);
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    notes: String,
}
#[tauri::command]
pub async fn check_app_update(
    window: WebviewWindow,
    state: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>> {
    main_window(&window)?;
    let mut pending = state
        .0
        .try_lock()
        .map_err(|_| AppError::new("UPDATE_BUSY", "更新操作正在进行。"))?;
    let update = window
        .app_handle()
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(update_error)?
        .check()
        .await
        .map_err(update_error)?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone().unwrap_or_default(),
    });
    *pending = update;
    Ok(info)
}
#[tauri::command]
pub async fn install_app_update(
    window: WebviewWindow,
    state: State<'_, PendingUpdate>,
) -> Result<()> {
    main_window(&window)?;
    let mut pending = state
        .0
        .try_lock()
        .map_err(|_| AppError::new("UPDATE_BUSY", "更新操作正在进行。"))?;
    let update = pending
        .as_ref()
        .ok_or_else(|| AppError::new("UPDATE_NOT_CHECKED", "请先检查可用更新。"))?;
    let mut received = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                received += chunk as u64;
                if let Err(error) = window.emit(
                    "update-progress",
                    serde_json::json!({"received":received,"total":total}),
                ) {
                    eprintln!("Update progress failed: {error}")
                }
            },
            || {},
        )
        .await
        .map_err(update_error)?;
    *pending = None;
    crate::desktop::prepare_exit(window.app_handle());
    window.app_handle().restart();
}
fn update_error(error: impl ToString) -> AppError {
    AppError::new(
        "UPDATE_ERROR",
        format!("应用更新失败：{}", error.to_string()),
    )
}
