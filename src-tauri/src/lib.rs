//! Dev Cockpit — always-visible macOS dev environment dashboard.
//! App wiring: tray, panel window behavior, global shortcut, plugins,
//! background monitor.

pub mod commands;
pub mod config;
pub mod control;
pub mod logs;
pub mod models;
pub mod monitor;
pub mod notify;
pub mod scan;
pub mod state;
pub mod util;

use tauri::menu::{CheckMenuItem, MenuBuilder, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use state::AppState;

const PANEL_WIDTH: f64 = 460.0;

fn panel(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// Show the panel anchored under the tray icon (when a tray rect is
/// available) or at its last position.
fn show_panel(app: &AppHandle, anchor: Option<tauri::Rect>) {
    let Some(win) = panel(app) else { return };
    if let Some(rect) = anchor {
        if let (tauri::Position::Physical(pos), tauri::Size::Physical(size)) =
            (rect.position, rect.size)
        {
            let scale = win
                .current_monitor()
                .ok()
                .flatten()
                .map(|m| m.scale_factor())
                .unwrap_or(2.0);
            let cx = (pos.x as f64 + size.width as f64 / 2.0) / scale;
            let top = (pos.y as f64 + size.height as f64) / scale;
            let mut x = cx - PANEL_WIDTH / 2.0;
            let y = top + 6.0;
            // clamp to the monitor the tray lives on
            if let Ok(Some(mon)) = win.current_monitor() {
                let mpos = mon.position().to_logical::<f64>(mon.scale_factor());
                let msize = mon.size().to_logical::<f64>(mon.scale_factor());
                let max_x = mpos.x + msize.width - PANEL_WIDTH - 8.0;
                x = x.clamp(mpos.x + 8.0, max_x.max(mpos.x + 8.0));
            }
            let _ = win.set_position(tauri::LogicalPosition::new(x, y));
        }
    }
    let _ = win.show();
    let _ = win.set_focus();
    // Fresh data the moment the panel opens.
    let st = app.state::<AppState>();
    st.git_trigger.store(true, std::sync::atomic::Ordering::Relaxed);
    st.request_scan();
}

fn toggle_panel(app: &AppHandle, anchor: Option<tauri::Rect>) {
    let Some(win) = panel(app) else { return };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        show_panel(app, anchor);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_config = config::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_panel(app, None);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["ctrl+alt+d"])
                .expect("register shortcut")
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_panel(app, None);
                    }
                })
                .build(),
        )
        .manage(AppState::new(app_config))
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::force_scan,
            commands::rescan_projects,
            commands::get_config,
            commands::set_config,
            commands::stop_service,
            commands::start_service,
            commands::restart_service,
            commands::docker_action,
            commands::compose_action,
            commands::open_path,
            commands::open_url,
            commands::open_log_session,
            commands::get_log_lines,
            commands::close_log_session,
            commands::get_autostart,
            commands::set_autostart,
            commands::set_paused,
            commands::is_paused,
            commands::hide_panel,
            commands::quit_app,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            build_tray(app.handle())?;

            let state = app.state::<AppState>();
            state.logs.spawn_flusher(app.handle().clone());
            monitor::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let _ = window.hide();
                api.prevent_close();
            }
            tauri::WindowEvent::Focused(false) => {
                let pinned = {
                    let st = window.app_handle().state::<AppState>();
                    let cfg = st.config_clone();
                    cfg.pinned
                };
                if !pinned && window.label() == "main" {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app.state::<AppState>();
                control::shutdown_managed(&state);
            }
        });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Dev Cockpit", true, Some("ctrl+alt+d"))?;
    let scan = MenuItem::with_id(app, "scan", "Rescan Now", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause Monitoring", true, false, None::<&str>)?;
    let autostart = {
        use tauri_plugin_autostart::ManagerExt;
        let enabled = app.autolaunch().is_enabled().unwrap_or(false);
        CheckMenuItem::with_id(app, "autostart", "Launch at Login", true, enabled, None::<&str>)?
    };
    let quit = MenuItem::with_id(app, "quit", "Quit Dev Cockpit", true, Some("cmd+q"))?;
    let menu = MenuBuilder::new(app)
        .item(&open)
        .item(&scan)
        .separator()
        .item(&pause)
        .item(&autostart)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&quit)
        .build()?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state, rect, .. } = event {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    toggle_panel(tray.app_handle(), Some(rect));
                }
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_panel(app, None),
            "scan" => {
                let st = app.state::<AppState>();
                st.request_discovery();
            }
            "pause" => {
                let st = app.state::<AppState>();
                let now = !st.is_paused();
                st.paused.store(now, std::sync::atomic::Ordering::Relaxed);
                if !now {
                    st.request_scan();
                }
                let _ = app.emit("paused", now);
            }
            "autostart" => {
                use tauri_plugin_autostart::ManagerExt;
                let l = app.autolaunch();
                let enabled = l.is_enabled().unwrap_or(false);
                let _ = if enabled { l.disable() } else { l.enable() };
            }
            "quit" => {
                let st = app.state::<AppState>();
                control::shutdown_managed(&st);
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}
