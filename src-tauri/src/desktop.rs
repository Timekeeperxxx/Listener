use crate::{
    error::{AppError, Result},
    main_window,
    models::ShortcutSettings,
};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WebviewWindow,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct DesktopState {
    bindings: Mutex<Vec<(Shortcut, String)>>,
    configuration: Mutex<()>,
    pub shortcut_error: Mutex<String>,
    pub quitting: AtomicBool,
    play_item: MenuItem<tauri::Wry>,
    last_track: Mutex<(String, bool)>,
}
fn desktop_error(message: impl ToString) -> AppError {
    AppError::new("DESKTOP_ERROR", message.to_string())
}
pub fn show_main(app: &AppHandle) -> Result<()> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::new("WINDOW_MISSING", "主窗口已关闭。"))?;
    main.unminimize()?;
    main.show()?;
    main.set_focus()?;
    Ok(())
}
fn emit_action(app: &AppHandle, action: &str) -> Result<()> {
    if action == "main" {
        show_main(app)
    } else {
        app.emit_to("main", "desktop-action", action)?;
        Ok(())
    }
}
pub fn parse_shortcuts(settings: &ShortcutSettings) -> Result<Vec<(Shortcut, String)>> {
    if !settings.enabled {
        return Ok(vec![]);
    }
    let mut ids = HashSet::new();
    [
        ("toggle", &settings.toggle),
        ("next", &settings.next),
        ("previous", &settings.previous),
        ("main", &settings.main),
    ]
    .into_iter()
    .map(|(action, text)| {
        let shortcut: Shortcut = text
            .parse()
            .map_err(|_| AppError::new("INVALID_SHORTCUT", "快捷键格式无效，请重新录入。"))?;
        if shortcut.mods.bits().count_ones() < 2 || !ids.insert(shortcut.id()) {
            return Err(AppError::new(
                "INVALID_SHORTCUT",
                "每个全局快捷键需要至少两个修饰键，且不能重复。",
            ));
        }
        Ok((shortcut, action.to_owned()))
    })
    .collect()
}
pub fn apply_shortcuts(app: &AppHandle, settings: &ShortcutSettings) -> Result<()> {
    let next = parse_shortcuts(settings)?;
    let state = app.state::<DesktopState>();
    let _configuration = state.configuration.lock().map_err(desktop_error)?;
    let previous = state.bindings.lock().map_err(desktop_error)?.clone();
    let old_ids: HashSet<_> = previous.iter().map(|(key, _)| key.id()).collect();
    let new_ids: HashSet<_> = next.iter().map(|(key, _)| key.id()).collect();
    let mut added = vec![];
    for (key, _) in &next {
        if old_ids.contains(&key.id()) {
            continue;
        }
        if let Err(error) = app.global_shortcut().register(*key) {
            for registered in added {
                app.global_shortcut()
                    .unregister(registered)
                    .map_err(desktop_error)?;
            }
            return Err(AppError::new(
                "SHORTCUT_CONFLICT",
                format!("快捷键注册失败，可能已被占用：{error}"),
            ));
        }
        added.push(*key);
    }
    let mut removed = vec![];
    for (key, _) in previous
        .iter()
        .filter(|(key, _)| !new_ids.contains(&key.id()))
    {
        if let Err(error) = app.global_shortcut().unregister(*key) {
            let mut failures = vec![];
            for registered in added {
                if let Err(cause) = app.global_shortcut().unregister(registered) {
                    failures.push(cause.to_string());
                }
            }
            for old in removed {
                if let Err(cause) = app.global_shortcut().register(old) {
                    failures.push(cause.to_string());
                }
            }
            let message = if failures.is_empty() {
                format!("快捷键未更改：{error}")
            } else {
                format!(
                    "快捷键恢复失败：{}；请退出后重新打开 Listener。",
                    failures.join("；")
                )
            };
            *state.shortcut_error.lock().map_err(desktop_error)? = message.clone();
            return Err(desktop_error(message));
        }
        removed.push(*key);
    }
    *state.bindings.lock().map_err(desktop_error)? = next;
    *state.shortcut_error.lock().map_err(desktop_error)? = String::new();
    Ok(())
}
pub fn setup(app: &AppHandle, settings: &ShortcutSettings) -> Result<()> {
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, key, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                let state = app.state::<DesktopState>();
                let action = state
                    .bindings
                    .lock()
                    .expect("desktop bindings poisoned")
                    .iter()
                    .find(|(binding, _)| binding.id() == key.id())
                    .map(|(_, action)| action.clone());
                if let Some(action) = action {
                    if let Err(error) = emit_action(app, &action) {
                        eprintln!("Desktop action failed: {error:?}")
                    }
                }
            })
            .build(),
    )
    .map_err(desktop_error)?;
    let main = MenuItem::with_id(app, "main", "打开 Listener", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "toggle", "播放 / 暂停", false, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", "上一首", true, None::<&str>)?;
    let lyrics = MenuItem::with_id(app, "lyrics", "歌词页面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Listener", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&main, &play, &next, &previous, &lyrics, &quit])?;
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for (i, height) in [8, 16, 26, 18, 24, 10].into_iter().enumerate() {
        for x in (3 + i * 5)..(5 + i * 5) {
            for y in (16 - height / 2)..(16 + height / 2) {
                pixels[(y * 32 + x) * 4 + 3] = 255;
            }
        }
    }
    TrayIconBuilder::with_id("listener")
        .icon(tauri::image::Image::new_owned(pixels, 32, 32))
        .icon_as_template(true)
        .tooltip("Listener")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            if let Err(error) = emit_action(app, event.id.as_ref()) {
                eprintln!("Menu action failed: {error:?}")
            }
        })
        .build(app)?;
    app.manage(DesktopState {
        bindings: Mutex::new(vec![]),
        configuration: Mutex::new(()),
        shortcut_error: Mutex::new(String::new()),
        quitting: AtomicBool::new(false),
        play_item: play,
        last_track: Mutex::new((String::new(), false)),
    });
    if let Err(error) = apply_shortcuts(app, settings) {
        *app.state::<DesktopState>()
            .shortcut_error
            .lock()
            .map_err(desktop_error)? = error.message;
    }
    Ok(())
}
pub fn update_tray(app: &AppHandle, payload: &serde_json::Value) -> Result<()> {
    let state = app.state::<DesktopState>();
    let mid = payload
        .pointer("/current/mid")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let playing = payload["playing"]
        .as_bool()
        .ok_or_else(|| AppError::protocol("companion.playing"))?;
    let mut last = state.last_track.lock().map_err(desktop_error)?;
    if last.0 == mid && last.1 == playing {
        return Ok(());
    }
    state
        .play_item
        .set_text(if playing { "暂停" } else { "播放" })?;
    state.play_item.set_enabled(!mid.is_empty())?;
    *last = (mid.to_owned(), playing);
    Ok(())
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatus {
    shortcut_error: String,
}
#[tauri::command]
pub fn get_desktop_status(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<DesktopStatus> {
    main_window(&window)?;
    Ok(DesktopStatus {
        shortcut_error: state.shortcut_error.lock().map_err(desktop_error)?.clone(),
    })
}
#[tauri::command]
pub fn hide_main(window: WebviewWindow) -> Result<()> {
    main_window(&window)?;
    window.hide()?;
    Ok(())
}
#[tauri::command]
pub fn quit_app(window: WebviewWindow, state: State<'_, DesktopState>) -> Result<()> {
    main_window(&window)?;
    state.quitting.store(true, Ordering::SeqCst);
    window.app_handle().exit(0);
    Ok(())
}
pub fn prepare_exit(app: &AppHandle) {
    app.state::<DesktopState>()
        .quitting
        .store(true, Ordering::SeqCst);
}
pub fn is_quitting(app: &AppHandle) -> bool {
    app.state::<DesktopState>().quitting.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hotkeys_require_modifiers_and_distinct_bindings() {
        let mut settings = ShortcutSettings::default();
        assert_eq!(parse_shortcuts(&settings).unwrap().len(), 4);
        settings.next = settings.toggle.clone();
        assert!(parse_shortcuts(&settings).is_err());
        settings.next = "Space".into();
        assert!(parse_shortcuts(&settings).is_err());
    }
}
