//! Shared application state managed by Tauri.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::logs::LogRegistry;
use crate::models::{AppConfig, Snapshot};
use crate::util::now_ms;

/// A child process spawned by Dev Cockpit (Start button / compose action).
#[derive(Debug, Clone)]
pub struct ManagedChild {
    pub id: String,
    pub pid: i32,
    pub pgid: i32,
    pub name: String,
    pub project_path: Option<String>,
    pub dir: String,
    pub command: String,
    pub started_ms: u64,
    pub log_session: String,
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub snapshot: Mutex<Snapshot>,
    pub managed: Mutex<HashMap<String, ManagedChild>>,
    pub logs: LogRegistry,
    /// Notification suppression after user-initiated actions: key -> expiry.
    recent_actions: Mutex<HashMap<String, u64>>,
    pub paused: AtomicBool,
    /// Wakes the monitor loop for an immediate rescan.
    pub scan_trigger: tokio::sync::Notify,
    pub discovery_trigger: AtomicBool,
    pub git_trigger: AtomicBool,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Mutex::new(config),
            snapshot: Mutex::new(Snapshot::default()),
            managed: Mutex::new(HashMap::new()),
            logs: LogRegistry::new(),
            recent_actions: Mutex::new(HashMap::new()),
            paused: AtomicBool::new(false),
            scan_trigger: tokio::sync::Notify::new(),
            discovery_trigger: AtomicBool::new(true),
            git_trigger: AtomicBool::new(false),
        }
    }

    pub fn config_clone(&self) -> AppConfig {
        self.config.lock().expect("config lock").clone()
    }

    pub fn snapshot_clone(&self) -> Snapshot {
        self.snapshot.lock().expect("snapshot lock").clone()
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn request_scan(&self) {
        self.scan_trigger.notify_one();
    }

    pub fn request_discovery(&self) {
        self.discovery_trigger.store(true, Ordering::Relaxed);
        self.scan_trigger.notify_one();
    }

    /// Suppress notifications for `key` for `secs` seconds (user actions
    /// should not produce "service stopped" alerts).
    pub fn suppress(&self, key: &str, secs: u64) {
        let mut m = self.recent_actions.lock().expect("suppress lock");
        m.insert(key.to_string(), now_ms() + secs * 1000);
        m.retain(|_, exp| *exp > now_ms());
    }

    pub fn is_suppressed(&self, key: &str) -> bool {
        let m = self.recent_actions.lock().expect("suppress lock");
        m.get(key).map(|exp| *exp > now_ms()).unwrap_or(false)
    }

    pub fn managed_views(&self) -> Vec<crate::scan::link::ManagedView> {
        self.managed
            .lock()
            .expect("managed lock")
            .values()
            .map(|m| crate::scan::link::ManagedView {
                id: m.id.clone(),
                pid: m.pid,
                pgid: m.pgid,
                name: m.name.clone(),
                project_path: m.project_path.clone(),
                dir: m.dir.clone(),
                command: m.command.clone(),
                started_ms: m.started_ms,
            })
            .collect()
    }
}
