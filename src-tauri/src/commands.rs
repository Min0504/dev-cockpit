//! Tauri command handlers — thin, validated wrappers over state & control.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::models::{AppConfig, LogLine, LogSessionInfo, Snapshot};
use crate::state::AppState;
use crate::{config, control};

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.snapshot_clone()
}

#[tauri::command]
pub fn force_scan(state: State<'_, AppState>) {
    state.git_trigger.store(true, std::sync::atomic::Ordering::Relaxed);
    state.request_scan();
}

#[tauri::command]
pub fn rescan_projects(state: State<'_, AppState>) {
    state.request_discovery();
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config_clone()
}

#[tauri::command]
pub fn set_config(
    state: State<'_, AppState>,
    mut config: AppConfig,
) -> Result<AppConfig, String> {
    config.sanitize();
    let roots_changed = {
        let mut guard = state.config.lock().expect("config lock");
        let changed = guard.roots != config.roots;
        *guard = config.clone();
        changed
    };
    config::save(&config)?;
    if roots_changed {
        state.request_discovery();
    } else {
        state.request_scan();
    }
    Ok(config)
}

#[tauri::command]
pub fn stop_service(app: AppHandle, pid: i32, force: bool) -> Result<(), String> {
    control::stop_pid(&app, pid, force)
}

#[tauri::command]
pub fn start_service(
    app: AppHandle,
    project_path: String,
    service_name: String,
) -> Result<String, String> {
    control::start_service(&app, &project_path, &service_name)
}

#[tauri::command]
pub fn restart_service(
    app: AppHandle,
    pid: Option<i32>,
    project_path: Option<String>,
    service_name: String,
) -> Result<(), String> {
    control::restart_service(&app, pid, project_path, service_name)
}

#[tauri::command]
pub fn docker_action(app: AppHandle, id: String, action: String) -> Result<(), String> {
    control::docker_action(&app, &id, &action)
}

#[tauri::command]
pub fn compose_action(
    app: AppHandle,
    project_path: String,
    action: String,
) -> Result<String, String> {
    control::compose_action(&app, &project_path, &action)
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String, target: String) -> Result<(), String> {
    control::open_path(&app, &path, &target)
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    control::open_url(&url)
}

// ---------------------------------------------------------------------------
// logs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsPayload {
    pub info: LogSessionInfo,
    pub lines: Vec<LogLine>,
    pub ended: bool,
}

/// Open (or reuse) a log session. kind: "docker" | "managed".
#[tauri::command]
pub fn open_log_session(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    id: String,
    title: String,
) -> Result<String, String> {
    match kind.as_str() {
        "docker" => state.logs.open_docker(&app, &id, &title),
        "managed" => {
            let managed = state.managed.lock().expect("managed lock");
            managed
                .get(&id)
                .map(|m| m.log_session.clone())
                .or_else(|| {
                    // Session may outlive the registry entry briefly.
                    state.logs.find_by_source(&format!("managed:{id}"))
                })
                .ok_or_else(|| "log session no longer available".to_string())
        }
        "session" => Ok(id),
        _ => Err(format!("unknown log source: {kind}")),
    }
}

#[tauri::command]
pub fn get_log_lines(state: State<'_, AppState>, session: String) -> Result<LogsPayload, String> {
    state
        .logs
        .lines(&session)
        .map(|(info, lines, ended)| LogsPayload { info, lines, ended })
        .ok_or_else(|| "unknown log session".to_string())
}

#[tauri::command]
pub fn close_log_session(state: State<'_, AppState>, session: String) {
    state.logs.close(&session);
}

// ---------------------------------------------------------------------------
// app-level
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let r = if enabled { launcher.enable() } else { launcher.disable() };
    r.map_err(|e| e.to_string())?;
    Ok(launcher.is_enabled().unwrap_or(enabled))
}

#[tauri::command]
pub fn set_paused(state: State<'_, AppState>, paused: bool) {
    state
        .paused
        .store(paused, std::sync::atomic::Ordering::Relaxed);
    if !paused {
        state.request_scan();
    }
}

#[tauri::command]
pub fn is_paused(state: State<'_, AppState>) -> bool {
    state.is_paused()
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    let state = app.state::<AppState>();
    control::shutdown_managed(&state);
    app.exit(0);
}
