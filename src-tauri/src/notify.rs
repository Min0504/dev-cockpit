//! macOS notifications with per-event cooldowns and startup / wake grace.
//!
//! Diffs consecutive snapshots and alerts on: service stopped, container
//! stopped, health failures + recovery, new port conflicts, Docker daemon
//! state changes. User-initiated actions register suppression keys so their
//! own stop/start never alarms.

use std::collections::{HashMap, HashSet};

use tauri::AppHandle;

use crate::models::{HealthLevel, NotifyConfig, ServiceKind, Snapshot};
use crate::state::AppState;
use crate::util::now_ms;

#[derive(Debug, Clone, PartialEq)]
struct SvcSnap {
    key: String,
    name: String,
    port: u16,
    project: Option<String>,
    level: Option<HealthLevel>,
}

#[derive(Debug, Default)]
struct Observed {
    services: HashMap<String, SvcSnap>,
    /// container name -> (state, compose project)
    containers: HashMap<String, (String, Option<String>)>,
    conflicts: HashSet<u16>,
    docker_ok: bool,
    docker_known: bool,
}

#[derive(Default)]
pub struct Notifier {
    prev: Option<Observed>,
    ticks: u64,
    cooldowns: HashMap<String, u64>,
    health_fails: HashMap<String, u32>,
    alerted_down: HashSet<String>,
}

pub fn service_key(project: &Option<String>, name: &str, port: u16) -> String {
    format!("s|{}|{}|{}", project.as_deref().unwrap_or("-"), name, port)
}

impl Notifier {
    fn observe(snapshot: &Snapshot) -> Observed {
        let mut o = Observed {
            docker_ok: snapshot.docker.available,
            docker_known: true,
            ..Default::default()
        };
        let mut add_svc = |project: Option<String>, s: &crate::models::Service| {
            if s.kind != ServiceKind::Process {
                return;
            }
            let port = s.first_port().unwrap_or(0);
            if port == 0 {
                return; // "starting" managed services have no port yet
            }
            let key = service_key(&s.project_path, &s.name, port);
            o.services.insert(
                key.clone(),
                SvcSnap { key, name: s.name.clone(), port, project, level: s.health.level },
            );
        };
        for p in &snapshot.projects {
            for s in &p.services {
                add_svc(Some(p.name.clone()), s);
            }
            for c in &p.containers {
                o.containers
                    .insert(c.name.clone(), (c.state.clone(), c.compose_project.clone()));
            }
        }
        for s in &snapshot.orphan_services {
            add_svc(None, s);
        }
        for c in &snapshot.unlinked_containers {
            o.containers
                .insert(c.name.clone(), (c.state.clone(), c.compose_project.clone()));
        }
        o.conflicts = snapshot.conflicts.iter().map(|c| c.port).collect();
        o
    }

    /// Compare with the previous cycle and fire notifications.
    /// `resync` suppresses "down" alerts right after sleep/wake gaps.
    pub fn process(
        &mut self,
        app: &AppHandle,
        state: &AppState,
        cfg: &NotifyConfig,
        snapshot: &Snapshot,
        resync: bool,
    ) {
        let cur = Self::observe(snapshot);
        self.ticks += 1;
        let grace = self.ticks <= 2 || resync;
        let Some(prev) = self.prev.take() else {
            self.prev = Some(cur);
            return;
        };
        if grace || !cfg.enabled {
            self.prev = Some(cur);
            return;
        }

        let mut fire: Vec<(String, String, String)> = Vec::new(); // (cool-key, title, body)

        // --- services stopped ---
        if cfg.service_stopped {
            for (key, svc) in &prev.services {
                if cur.services.contains_key(key) {
                    continue;
                }
                if state.is_suppressed(key) || state.is_suppressed(&service_key(&None, &svc.name, 0)) {
                    continue;
                }
                let place = svc.project.clone().unwrap_or_else(|| "unlinked".into());
                fire.push((
                    format!("stop|{key}"),
                    "Service stopped".into(),
                    format!("{} :{} — {}", svc.name, svc.port, place),
                ));
            }
        }

        // --- health transitions ---
        for (key, svc) in &cur.services {
            match svc.level {
                Some(HealthLevel::Down) => {
                    let n = self.health_fails.entry(key.clone()).or_insert(0);
                    *n += 1;
                    let was_ok = prev
                        .services
                        .get(key)
                        .map(|p| !matches!(p.level, Some(HealthLevel::Down)))
                        .unwrap_or(false);
                    if *n >= 2 && was_ok && cfg.health_failed && !state.is_suppressed(key) {
                        self.alerted_down.insert(key.clone());
                        fire.push((
                            format!("health|{key}"),
                            "Health check failed".into(),
                            format!(
                                "{} :{} is not accepting connections{}",
                                svc.name,
                                svc.port,
                                svc.project
                                    .as_ref()
                                    .map(|p| format!(" — {p}"))
                                    .unwrap_or_default()
                            ),
                        ));
                    }
                }
                _ => {
                    self.health_fails.remove(key);
                    if self.alerted_down.remove(key) && cfg.recovered {
                        fire.push((
                            format!("recover|{key}"),
                            "Recovered".into(),
                            format!("{} :{} is healthy again", svc.name, svc.port),
                        ));
                    }
                }
            }
        }
        self.health_fails.retain(|k, _| cur.services.contains_key(k));

        // --- containers ---
        if cfg.container_stopped {
            for (name, (prev_state, compose)) in &prev.containers {
                if prev_state != "running" {
                    continue;
                }
                let now_running = cur
                    .containers
                    .get(name)
                    .map(|(s, _)| s == "running")
                    .unwrap_or(false);
                if now_running {
                    continue;
                }
                let suppressed = state.is_suppressed(&format!("c|{name}"))
                    || compose
                        .as_ref()
                        .map(|cp| state.is_suppressed(&format!("cp|{}", cp.to_lowercase())))
                        .unwrap_or(false);
                if suppressed {
                    continue;
                }
                fire.push((
                    format!("cstop|{name}"),
                    "Container stopped".into(),
                    format!("{name} is no longer running"),
                ));
            }
        }

        // --- new port conflicts ---
        if cfg.port_conflict {
            for port in cur.conflicts.difference(&prev.conflicts) {
                let detail = snapshot
                    .conflicts
                    .iter()
                    .find(|c| c.port == *port)
                    .map(|c| {
                        c.entries
                            .iter()
                            .map(|e| e.process.as_str())
                            .collect::<Vec<_>>()
                            .join(" · ")
                    })
                    .unwrap_or_default();
                fire.push((
                    format!("conflict|{port}"),
                    format!("Port conflict :{port}"),
                    detail,
                ));
            }
        }

        // --- docker daemon ---
        if prev.docker_known && prev.docker_ok != cur.docker_ok {
            if cur.docker_ok {
                fire.push(("docker|up".into(), "Docker is back".into(), "Daemon reachable again".into()));
            } else {
                fire.push((
                    "docker|down".into(),
                    "Docker unreachable".into(),
                    snapshot.docker.reason.clone().unwrap_or_else(|| "daemon stopped".into()),
                ));
            }
        }

        // --- send with cooldown ---
        let now = now_ms();
        let cooldown_ms = cfg.cooldown_sec * 1000;
        for (key, title, body) in fire {
            let due = self
                .cooldowns
                .get(&key)
                .map(|last| now.saturating_sub(*last) >= cooldown_ms)
                .unwrap_or(true);
            if due {
                self.cooldowns.insert(key, now);
                send(app, &title, &body);
            }
        }
        self.cooldowns.retain(|_, last| now.saturating_sub(*last) < 24 * 3600 * 1000);
        self.prev = Some(cur);
    }
}

/// Deliver a notification. Release builds use the notification plugin
/// (proper app attribution); dev builds fall back to osascript because
/// unbundled binaries cannot post UNUserNotifications.
pub fn send(app: &AppHandle, title: &str, body: &str) {
    #[cfg(debug_assertions)]
    {
        let _ = app;
        osascript_notify(title, body);
    }
    #[cfg(not(debug_assertions))]
    {
        use tauri_plugin_notification::NotificationExt;
        let r = app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
        if r.is_err() {
            osascript_notify(title, body);
        }
    }
}

fn osascript_notify(title: &str, body: &str) {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        esc(body),
        esc(title)
    );
    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
