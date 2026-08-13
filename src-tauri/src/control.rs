//! Service control: start / stop / restart processes, compose actions and
//! "open in …" helpers. Every mutating entry point validates its target
//! against the current snapshot — the UI can only act on what was detected.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::models::{ServiceKind, Snapshot};
use crate::scan::procs;
use crate::state::{AppState, ManagedChild};
use crate::util::{now_ms, run_cmd};

/// Notification-suppression key for a service (mirrors notify.rs).
pub fn service_suppress_key(project: &Option<String>, name: &str, port: u16) -> String {
    format!("s|{}|{}|{}", project.as_deref().unwrap_or("-"), name, port)
}

fn find_service_by_pid(snapshot: &Snapshot, pid: i32) -> Option<crate::models::Service> {
    snapshot
        .projects
        .iter()
        .flat_map(|p| p.services.iter())
        .chain(snapshot.orphan_services.iter())
        .find(|s| s.pid == Some(pid) && s.kind == ServiceKind::Process)
        .cloned()
}

// ---------------------------------------------------------------------------
// stop / kill
// ---------------------------------------------------------------------------

/// SIGTERM (or SIGKILL when `force`) a detected dev process. Kills the whole
/// process group when the target leads its own group, so `npm run dev` trees
/// die together. Escalates to SIGKILL automatically after a grace period.
pub fn stop_pid(app: &AppHandle, pid: i32, force: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let me = std::process::id() as i32;
    if pid <= 1 || pid == me {
        return Err("refusing to signal this pid".into());
    }
    let snapshot = state.snapshot_clone();
    let Some(svc) = find_service_by_pid(&snapshot, pid) else {
        return Err("process is not a detected dev service".into());
    };
    // Suppress "service stopped" notifications for this intentional action.
    for port in svc.ports.iter().copied().chain(std::iter::once(0)) {
        state.suppress(&service_suppress_key(&svc.project_path, &svc.name, port), 30);
    }

    signal_tree(pid, if force { libc::SIGKILL } else { libc::SIGTERM })?;

    if !force {
        // Escalate if it ignores SIGTERM.
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if procs::alive(pid) {
                let _ = signal_tree(pid, libc::SIGKILL);
            }
            let state = app2.state::<AppState>();
            state.request_scan();
        });
    }
    state.request_scan();
    Ok(())
}

/// Send a signal to the pid's process group when it leads its own group,
/// otherwise to the single pid. Never touches our own group.
fn signal_tree(pid: i32, sig: i32) -> Result<(), String> {
    let my_pgid = unsafe { libc::getpgid(0) };
    let target_pgid = procs::pgid_of(pid);
    let r = unsafe {
        if target_pgid > 1 && target_pgid != my_pgid {
            libc::killpg(target_pgid, sig)
        } else {
            libc::kill(pid, sig)
        }
    };
    if r != 0 {
        let err = std::io::Error::last_os_error();
        // ESRCH = already gone: that's success for a stop action.
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        return Err(format!("signal failed: {err}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// start
// ---------------------------------------------------------------------------

/// Spawn a start command in its own process group, with output captured into
/// a log session. The command runs through the user's login shell so nvm /
/// pnpm / pyenv installs resolve exactly like in a terminal.
pub fn start_service(
    app: &AppHandle,
    project_path: &str,
    service_name: &str,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot_clone();
    let project = snapshot
        .projects
        .iter()
        .find(|p| p.path == project_path)
        .ok_or_else(|| "unknown project".to_string())?;
    let svc = project
        .services
        .iter()
        .find(|s| s.name == service_name && s.kind == ServiceKind::Startable)
        .or_else(|| project.services.iter().find(|s| s.name == service_name))
        .ok_or_else(|| format!("no service named {service_name}"))?;
    let command = svc
        .start_command
        .clone()
        .ok_or_else(|| "no start command detected — set one in project settings".to_string())?;
    let dir = svc.cwd.clone().unwrap_or_else(|| project_path.to_string());
    spawn_managed(app, &command, &dir, service_name, Some(project_path.to_string()))
}

pub fn spawn_managed(
    app: &AppHandle,
    command: &str,
    dir: &str,
    name: &str,
    project_path: Option<String>,
) -> Result<String, String> {
    if command.trim().is_empty() {
        return Err("empty command".into());
    }
    if !Path::new(dir).is_dir() {
        return Err(format!("directory not found: {dir}"));
    }
    let state = app.state::<AppState>();

    let mut cmd = Command::new("/bin/zsh");
    cmd.args(["-ilc", command])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("FORCE_COLOR", "0")
        .env("NO_COLOR", "1");
    #[allow(unused_imports)]
    use std::os::unix::process::CommandExt;
    cmd.process_group(0); // own group → clean tree kill, survives our stdout

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let pid = child.id() as i32;
    let managed_id = format!("{}-{pid}", now_ms() % 1_000_000);

    let session = state.logs.open_managed(
        app,
        &managed_id,
        &format!("{name} — {command}"),
        child.stdout.take(),
        child.stderr.take(),
    );

    {
        let mut managed = state.managed.lock().expect("managed lock");
        managed.insert(
            managed_id.clone(),
            ManagedChild {
                id: managed_id.clone(),
                pid,
                pgid: pid, // process_group(0) → leader
                name: name.to_string(),
                project_path,
                dir: dir.to_string(),
                command: command.to_string(),
                started_ms: now_ms(),
                log_session: session.clone(),
            },
        );
    }

    // Reap + lifecycle: wait for the direct child, then watch the group.
    // The log readers keep draining until every pipe writer is gone.
    let app2 = app.clone();
    let mid = managed_id.clone();
    std::thread::Builder::new()
        .name(format!("managed-wait-{pid}"))
        .spawn(move || {
            let _ = child.wait();
            // The direct child died; grandchildren may keep the group alive.
            loop {
                if !procs::group_alive(pid) {
                    break;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            let state = app2.state::<AppState>();
            let session = {
                let mut managed = state.managed.lock().expect("managed lock");
                managed.remove(&mid).map(|m| m.log_session)
            };
            if let Some(sid) = session {
                state.logs.push(&app2, &sid, "── process exited ──", false);
                state.logs.mark_ended(&app2, &sid);
            }
            let _ = app2.emit("managed-exited", &mid);
            state.request_scan();
        })
        .map_err(|e| e.to_string())?;

    state.request_scan();
    Ok(session)
}

/// Stop then start. Works for processes with a known start command.
pub fn restart_service(
    app: &AppHandle,
    pid: Option<i32>,
    project_path: Option<String>,
    service_name: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot_clone();
    let svc = pid
        .and_then(|p| find_service_by_pid(&snapshot, p))
        .or_else(|| {
            project_path.as_ref().and_then(|pp| {
                snapshot
                    .projects
                    .iter()
                    .find(|p| &p.path == pp)
                    .and_then(|p| p.services.iter().find(|s| s.name == service_name).cloned())
            })
        })
        .ok_or_else(|| "service not found".to_string())?;

    let command = svc
        .start_command
        .clone()
        .ok_or_else(|| "no start command known — stop/start manually or set an override".to_string())?;
    let dir = svc.cwd.clone().unwrap_or_else(|| {
        svc.project_path.clone().unwrap_or_else(crate::util::home_dir)
    });
    let name = svc.name.clone();
    let project = svc.project_path.clone();

    if let Some(p) = svc.pid {
        stop_pid(app, p, false)?;
    }

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        // Wait for the old process (and its ports) to disappear.
        let old_pid = svc.pid.unwrap_or(0);
        for _ in 0..24 {
            if old_pid == 0 || !procs::alive(old_pid) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
        if let Err(e) = spawn_managed(&app2, &command, &dir, &name, project) {
            let _ = app2.emit("toast", format!("Restart failed: {e}"));
        }
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// docker / compose
// ---------------------------------------------------------------------------

pub fn docker_action(app: &AppHandle, id: &str, action: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot_clone();
    let container = snapshot
        .projects
        .iter()
        .flat_map(|p| p.containers.iter())
        .chain(snapshot.unlinked_containers.iter())
        .find(|c| c.id == id || c.id.starts_with(id))
        .ok_or_else(|| "unknown container".to_string())?;
    state.suppress(&format!("c|{}", container.name), 40);
    if let Some(cp) = &container.compose_project {
        state.suppress(&format!("cp|{cp}"), 40);
    }

    let id = container.id.clone();
    let action = action.to_string();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let r = crate::scan::docker::container_action(&id, &action);
        let state = app2.state::<AppState>();
        if let Err(e) = &r {
            let _ = app2.emit("toast", format!("Docker {action} failed: {e}"));
        }
        state.request_scan();
    });
    Ok(())
}

/// `docker compose up -d` / `down` for a project, output captured to a log
/// session so the user can watch progress.
pub fn compose_action(app: &AppHandle, project_path: &str, action: &str) -> Result<String, String> {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot_clone();
    let project = snapshot
        .projects
        .iter()
        .find(|p| p.path == project_path)
        .ok_or_else(|| "unknown project".to_string())?;
    if !project.has_compose {
        return Err("project has no compose file".into());
    }
    let cmd = match action {
        "up" => "docker compose up -d",
        "down" => "docker compose down",
        "restart" => "docker compose restart",
        _ => return Err(format!("unsupported compose action: {action}")),
    };
    // Suppress container notifications for everything in this project.
    let dir_name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    state.suppress(&format!("cp|{dir_name}"), 60);
    for c in &project.containers {
        state.suppress(&format!("c|{}", c.name), 60);
        if let Some(cp) = &c.compose_project {
            state.suppress(&format!("cp|{cp}"), 60);
        }
    }
    spawn_managed(app, cmd, project_path, &format!("compose {action}"), Some(project_path.into()))
}

// ---------------------------------------------------------------------------
// open in …
// ---------------------------------------------------------------------------

pub fn open_path(app: &AppHandle, path: &str, target: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Err(format!("path does not exist: {path}"));
    }
    let state = app.state::<AppState>();
    let cfg = state.config_clone();
    let out = match target {
        "terminal" => run_cmd("open", &["-a", &cfg.terminal_app, path], Duration::from_secs(6)),
        "editor" => run_cmd("open", &["-a", &cfg.editor_app, path], Duration::from_secs(6)),
        "finder" => run_cmd("open", &[path], Duration::from_secs(6)),
        _ => return Err(format!("unknown open target: {target}")),
    }
    .map_err(|e| e.to_string())?;
    if !out.ok() {
        return Err(format!(
            "open failed: {}",
            out.stderr.lines().next().unwrap_or("app not found?")
        ));
    }
    Ok(())
}

pub fn open_url(url: &str) -> Result<(), String> {
    let ok = ["http://localhost", "http://127.0.0.1", "https://localhost", "https://127.0.0.1", "http://[::1]"]
        .iter()
        .any(|p| url.starts_with(p));
    if !ok {
        return Err("only localhost URLs can be opened".into());
    }
    let out = run_cmd("open", &[url], Duration::from_secs(6)).map_err(|e| e.to_string())?;
    if !out.ok() {
        return Err("open failed".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// shutdown
// ---------------------------------------------------------------------------

/// Terminate every process we spawned (predictable quit semantics: externally
/// started processes are never touched).
pub fn shutdown_managed(state: &AppState) {
    let children: Vec<ManagedChild> = {
        let managed = state.managed.lock().expect("managed lock");
        managed.values().cloned().collect()
    };
    for c in &children {
        unsafe {
            libc::killpg(c.pgid, libc::SIGTERM);
        }
    }
    if !children.is_empty() {
        std::thread::sleep(Duration::from_millis(1200));
        for c in &children {
            if procs::group_alive(c.pgid) {
                unsafe {
                    libc::killpg(c.pgid, libc::SIGKILL);
                }
            }
        }
    }
}
