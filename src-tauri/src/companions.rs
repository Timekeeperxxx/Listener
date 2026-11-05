use crate::{
    error::{AppError, Result},
    main_window,
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow};
#[tauri::command]
pub fn publish_companions(window: WebviewWindow, payload: serde_json::Value) -> Result<()> {
    main_window(&window)?;
    crate::desktop::update_tray(window.app_handle(), &payload)?;
    for label in ["mini", "desktop-lyrics"] {
        if let Some(target) = window.app_handle().get_webview_window(label) {
            target.emit("companion-state", &payload)?;
        }
    }
    Ok(())
}
#[tauri::command]
pub fn companion_action(window: WebviewWindow, action: String) -> Result<()> {
    if !matches!(window.label(), "mini" | "desktop-lyrics")
        || !matches!(
            action.as_str(),
            "toggle" | "next" | "previous" | "ready" | "main"
        )
    {
        return Err(AppError::new("FORBIDDEN", "此窗口无权执行该操作。"));
    }
    if action == "main" {
        let main = window
            .app_handle()
            .get_webview_window("main")
            .ok_or_else(|| AppError::new("WINDOW_MISSING", "主窗口已关闭。"))?;
        main.show()?;
        main.set_focus()?;
    } else {
        window
            .app_handle()
            .emit_to("main", "companion-action", action)?;
    }
    Ok(())
}
#[tauri::command]
pub fn show_companion(window: WebviewWindow, kind: String) -> Result<()> {
    main_window(&window)?;
    let (width, height) = match kind.as_str() {
        "mini" => (390., 148.),
        "desktop-lyrics" => (760., 132.),
        _ => return Err(AppError::new("INVALID_WINDOW", "窗口类型无效。")),
    };
    if let Some(existing) = window.app_handle().get_webview_window(&kind) {
        existing.show()?;
        existing.set_focus()?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        window.app_handle(),
        kind.clone(),
        WebviewUrl::App(format!("index.html?view={kind}").into()),
    )
    .title(if kind == "mini" {
        "Listener 迷你播放器"
    } else {
        "Listener 桌面歌词"
    })
    .inner_size(width, height)
    .min_inner_size(320., 100.)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .visible_on_all_workspaces(true)
    .skip_taskbar(true)
    .build()?;
    Ok(())
}
