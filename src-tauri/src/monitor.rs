//! Monitor orchestrator: the background loop that scans ports, processes,
//! Docker, git and health on independent cadences, assembles snapshots,
//! emits them to the UI only when something changed, and feeds the notifier.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::models::{GitInfo, Health, HealthLevel, ServiceKind, Snapshot};
use crate::notify::Notifier;
use crate::scan::docker::{self, ContainerStats, DockerState};
use crate::scan::link::{self, LinkInput};
use crate::scan::projects::{self, ProjectStatic};
use crate::scan::procs::ProcScanner;
use crate::scan::{git, health, ports};
use crate::state::AppState;
use crate::util::now_ms;

const DOCKER_REPROBE_MS: u64 = 30_000;

struct Core {
    procs: ProcScanner,
    docker: DockerState,
    docker_checked_at: u64,
    stats: HashMap<String, ContainerStats>,
    stats_at: u64,
    git_cache: HashMap<String, (GitInfo, u64)>,
    discovery: Vec<ProjectStatic>,
    discovery_errors: Vec<String>,
    discovered_at: u64,
    notifier: Notifier,
    last_hash: u64,
    seq: u64,
    last_tick_ms: u64,
    last_tray: String,
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(run(app));
}

async fn run(app: AppHandle) {
    let mut core = Core {
        procs: ProcScanner::new(),
        docker: DockerState::default(),
        docker_checked_at: 0,
        stats: HashMap::new(),
        stats_at: 0,
        git_cache: HashMap::new(),
        discovery: Vec::new(),
        discovery_errors: Vec::new(),
        discovered_at: 0,
        notifier: Notifier::default(),
        last_hash: 0,
        seq: 0,
        last_tick_ms: 0,
        last_tray: String::from("\u{0}"), // force first tray update
    };

    loop {
        let state = app.state::<AppState>();
        if state.is_paused() {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(800)) => {},
                _ = state.scan_trigger.notified() => {},
            }
            continue;
        }
        let cfg = state.config_clone();
        let t0 = Instant::now();
        let now = now_ms();
        // Sleep/wake or long stall → resync cycle (no "stopped" alerts).
        let resync =
            core.last_tick_ms > 0 && now.saturating_sub(core.last_tick_ms) > cfg.poll_interval_ms * 3 + 2000;

        tick(&app, &mut core, &cfg, now, resync).await;

        core.last_tick_ms = now;
        let state = app.state::<AppState>();
        let budget =
            Duration::from_millis(cfg.poll_interval_ms).saturating_sub(t0.elapsed());
        tokio::select! {
            _ = tokio::time::sleep(budget) => {},
            _ = state.scan_trigger.notified() => {
                // Debounce bursts of triggers (e.g. stop + escalate).
                tokio::time::sleep(Duration::from_millis(180)).await;
            }
        }
    }
}

async fn tick(
    app: &AppHandle,
    core: &mut Core,
    cfg: &crate::models::AppConfig,
    now: u64,
    resync: bool,
) {
    let state = app.state::<AppState>();
    let mut errors: Vec<String> = Vec::new();

    // ---- 1. project discovery (slow cadence) ----
    let discovery_due = core.discovered_at == 0
        || now.saturating_sub(core.discovered_at) > cfg.discovery_interval_ms
        || state.discovery_trigger.swap(false, Ordering::Relaxed);
    if discovery_due {
        let roots = cfg.roots.clone();
        if let Ok(res) =
            tauri::async_runtime::spawn_blocking(move || projects::discover(&roots)).await
        {
            core.discovery = res.projects;
            core.discovery_errors = res.errors;
            core.discovered_at = now;
        }
    }
    errors.extend(core.discovery_errors.iter().cloned());

    // ---- 2. listening ports ----
    let listeners = match tauri::async_runtime::spawn_blocking(ports::scan).await {
        Ok(Ok(l)) => l,
        Ok(Err(e)) => {
            errors.push(e);
            Vec::new()
        }
        Err(e) => {
            errors.push(format!("port scan task failed: {e}"));
            Vec::new()
        }
    };

    // ---- 3. process enrichment (only pids that matter) ----
    let managed_views = state.managed_views();
    let mut pids: Vec<i32> = listeners.iter().map(|l| l.pid).collect();
    pids.extend(managed_views.iter().map(|m| m.pid));
    pids.sort_unstable();
    pids.dedup();
    let procs_map = core.procs.refresh(&pids);

    // ---- 4. docker (own cadence, cheap when daemon is down) ----
    let docker_due = if core.docker.available {
        now.saturating_sub(core.docker_checked_at) >= cfg.docker_interval_ms
    } else {
        core.docker_checked_at == 0
            || now.saturating_sub(core.docker_checked_at) >= DOCKER_REPROBE_MS
    };
    if docker_due {
        core.docker_checked_at = now;
        let refreshed = tauri::async_runtime::spawn_blocking(|| {
            let mut st = docker::probe();
            if st.available {
                match docker::list_containers() {
                    Ok(cs) => st.containers = cs,
                    Err(e) => {
                        st.available = false;
                        st.reason = Some(e);
                    }
                }
            }
            st
        })
        .await;
        if let Ok(st) = refreshed {
            core.docker = st;
        }
        // stats on a slower cadence, only when containers run
        let any_running = core.docker.containers.iter().any(|c| c.state == "running");
        if core.docker.available
            && any_running
            && now.saturating_sub(core.stats_at) >= cfg.docker_stats_interval_ms
        {
            core.stats_at = now;
            if let Ok(Ok(stats)) =
                tauri::async_runtime::spawn_blocking(docker::sample_stats).await
            {
                core.stats = stats;
            }
        }
    }
    // merge stats into container list
    let mut docker_state = core.docker.clone();
    for c in docker_state.containers.iter_mut() {
        let key = docker_state_stats_key(&core.stats, &c.id);
        if let Some(st) = key {
            c.cpu = st.cpu.map(|v| (v * 10.0).round() / 10.0);
            c.mem_bytes = st.mem_bytes.map(round_mem);
            c.mem_limit_bytes = st.mem_limit_bytes;
        }
        // running_for changes every minute; quantize display server-side to
        // keep snapshots stable (UI recomputes from status_text rarely).
        c.running_for = None;
    }

    // ---- 5. git (active projects fast lane, idle slow lane) ----
    let force_git = state.git_trigger.swap(false, Ordering::Relaxed);
    let active_paths: HashSet<String> = {
        let snap = state.snapshot_clone();
        snap.projects
            .iter()
            .filter(|p| p.active)
            .map(|p| p.path.clone())
            .collect()
    };
    let mut due_git: Vec<String> = Vec::new();
    for p in &core.discovery {
        if !p.has_git {
            continue;
        }
        let interval = if active_paths.contains(&p.path) {
            cfg.git_interval_ms
        } else {
            cfg.git_interval_ms * 6
        };
        let due = match core.git_cache.get(&p.path) {
            Some((_, at)) => now.saturating_sub(*at) > interval || force_git,
            None => true,
        };
        if due {
            due_git.push(p.path.clone());
        }
        if due_git.len() >= 6 && !force_git {
            break;
        }
    }
    if !due_git.is_empty() {
        let handles: Vec<_> = due_git
            .into_iter()
            .map(|path| {
                tauri::async_runtime::spawn_blocking(move || {
                    let info = git::collect(&path);
                    (path, info)
                })
            })
            .collect();
        for h in handles {
            if let Ok((path, info)) = h.await {
                match info {
                    Some(g) => {
                        core.git_cache.insert(path, (g, now));
                    }
                    None => {
                        core.git_cache.remove(&path);
                    }
                }
            }
        }
    }
    let discovery_paths: HashSet<&str> = core.discovery.iter().map(|p| p.path.as_str()).collect();
    core.git_cache.retain(|k, _| discovery_paths.contains(k.as_str()));
    let git_map: HashMap<String, GitInfo> = core
        .git_cache
        .iter()
        .map(|(k, (g, _))| (k.clone(), g.clone()))
        .collect();

    // ---- 6. assemble ----
    let mut snap = link::assemble(LinkInput {
        listeners: &listeners,
        procs: &procs_map,
        docker: &docker_state,
        projects: &core.discovery,
        git: &git_map,
        config: cfg,
        managed: &managed_views,
        self_pid: std::process::id() as i32,
        errors,
    });

    // ---- 7. health checks (parallel, bounded by service count) ----
    run_health_checks(&mut snap, cfg.http_health).await;

    snap.scan_ms = now_ms().saturating_sub(now);

    // ---- 8. notify (every tick, before dedup-emit) ----
    core.notifier
        .process(app, &state, &cfg.notifications, &snap, resync);

    // ---- 9. emit only on change ----
    let hash = {
        // seq/scan_ms/generated_at are volatile — hash the rest.
        let mut probe = snap.clone();
        probe.seq = 0;
        probe.generated_at_ms = 0;
        probe.scan_ms = 0;
        let json = serde_json::to_string(&probe).unwrap_or_default();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        json.hash(&mut h);
        h.finish()
    };
    if hash != core.last_hash {
        core.last_hash = hash;
        core.seq += 1;
        snap.seq = core.seq;
        *state.snapshot.lock().expect("snapshot lock") = snap.clone();
        let _ = app.emit("snapshot", &snap);
        update_tray(app, core, &snap);
    } else {
        // Keep the stored snapshot fresh for control validation.
        snap.seq = core.seq;
        *state.snapshot.lock().expect("snapshot lock") = snap;
    }
}

fn docker_state_stats_key<'a>(
    stats: &'a HashMap<String, ContainerStats>,
    id: &str,
) -> Option<&'a ContainerStats> {
    stats.get(id).or_else(|| {
        // `docker stats` reports short ids; `docker ps --no-trunc` long ones.
        stats
            .iter()
            .find(|(k, _)| id.starts_with(k.as_str()) || k.starts_with(id))
            .map(|(_, v)| v)
    })
}

fn round_mem(bytes: u64) -> u64 {
    const MB: u64 = 1024 * 1024;
    ((bytes + MB / 2) / MB) * MB
}

/// TCP + HTTP probes for every running service with a port.
async fn run_health_checks(snap: &mut Snapshot, http_enabled: bool) {
    struct Check {
        id: String,
        port: u16,
        is_http: bool,
    }
    let mut checks: Vec<Check> = Vec::new();
    for s in snap
        .projects
        .iter()
        .flat_map(|p| p.services.iter())
        .chain(snap.orphan_services.iter())
    {
        if s.kind == ServiceKind::Process {
            if let Some(port) = s.first_port() {
                checks.push(Check { id: s.id.clone(), port, is_http: s.is_http });
            }
        }
    }
    if checks.is_empty() {
        return;
    }
    let handles: Vec<_> = checks
        .into_iter()
        .map(|c| {
            tauri::async_runtime::spawn_blocking(move || {
                let tcp = health::tcp_check(c.port);
                let http = if tcp && http_enabled && c.is_http {
                    health::http_check(c.port)
                } else {
                    None
                };
                (c.id, tcp, http)
            })
        })
        .collect();
    let mut results: HashMap<String, (bool, Option<u16>)> = HashMap::new();
    for h in handles {
        if let Ok((id, tcp, http)) = h.await {
            results.insert(id, (tcp, http));
        }
    }
    let apply = |s: &mut crate::models::Service| {
        if let Some((tcp, http)) = results.get(&s.id) {
            let level = if !tcp {
                HealthLevel::Down
            } else if matches!(http, Some(code) if *code >= 500) {
                HealthLevel::Warn
            } else {
                HealthLevel::Ok
            };
            s.health = Health {
                tcp: Some(*tcp),
                http_status: *http,
                level: Some(level),
                detail: if !tcp {
                    Some("port not accepting connections".into())
                } else {
                    None
                },
            };
        }
    };
    for p in snap.projects.iter_mut() {
        for s in p.services.iter_mut() {
            apply(s);
        }
    }
    for s in snap.orphan_services.iter_mut() {
        apply(s);
    }
}

fn update_tray(app: &AppHandle, core: &mut Core, snap: &Snapshot) {
    let n = snap.totals.running_services + snap.totals.running_containers;
    let title = if n == 0 { String::new() } else { format!(" {n}") };
    let tooltip = format!(
        "Dev Cockpit — {} services · {} containers · {} ports{}",
        snap.totals.running_services,
        snap.totals.running_containers,
        snap.totals.listening_ports,
        if snap.conflicts.is_empty() {
            String::new()
        } else {
            format!(" · {} conflict(s)", snap.conflicts.len())
        }
    );
    if core.last_tray != title {
        core.last_tray = title.clone();
        if let Some(tray) = app.tray_by_id("main-tray") {
            let _ = tray.set_title(Some(title.as_str()));
            let _ = tray.set_tooltip(Some(tooltip.as_str()));
        }
    }
}
