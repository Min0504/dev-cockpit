//! Shared data models serialized to the frontend (camelCase JSON).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ServiceKind {
    Process,
    Container,
    Startable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum HealthLevel {
    Ok,
    Warn,
    Down,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// TCP connect to the first port succeeded.
    pub tcp: Option<bool>,
    /// Last HTTP status code observed (any response means the server is up).
    pub http_status: Option<u16>,
    #[serde(default)]
    pub level: Option<HealthLevel>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    /// Stable identity used for UI keys and notification diffing.
    pub id: String,
    pub kind: ServiceKind,
    pub name: String,
    /// Human label, e.g. "Vite", "Next.js", "PostgreSQL".
    pub framework: Option<String>,
    /// Machine key for icon/color mapping, e.g. "vite".
    pub framework_key: Option<String>,
    pub runtime: Option<String>,
    pub pid: Option<i32>,
    pub ports: Vec<u16>,
    /// Short display command.
    pub cmd: Option<String>,
    pub cmd_full: Option<String>,
    pub cwd: Option<String>,
    pub cpu: Option<f32>,
    pub mem_bytes: Option<u64>,
    pub started_at_ms: Option<u64>,
    pub health: Health,
    /// True when Dev Cockpit spawned this process itself (logs available).
    pub managed: bool,
    pub managed_id: Option<String>,
    pub container_id: Option<String>,
    pub project_path: Option<String>,
    /// Detected or overridden command used for Start / Restart.
    pub start_command: Option<String>,
    pub is_http: bool,
}

impl Service {
    pub fn first_port(&self) -> Option<u16> {
        self.ports.first().copied()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortBinding {
    pub host: Option<u16>,
    pub container: u16,
    pub proto: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    /// docker state: running / exited / paused / restarting…
    pub state: String,
    pub status_text: String,
    /// healthy / unhealthy / starting (from HEALTHCHECK), if present.
    pub health: Option<String>,
    pub ports: Vec<PortBinding>,
    pub compose_project: Option<String>,
    pub compose_service: Option<String>,
    pub compose_dir: Option<String>,
    pub cpu: Option<f32>,
    pub mem_bytes: Option<u64>,
    pub mem_limit_bytes: Option<u64>,
    pub running_for: Option<String>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub hash: String,
    pub summary: String,
    pub epoch_sec: u64,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub branch: String,
    pub dirty_count: u32,
    pub ahead: i32,
    pub behind: i32,
    pub last_commit: Option<CommitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    /// Canonical absolute path (also the id).
    pub path: String,
    pub name: String,
    pub active: bool,
    /// Frameworks detected from project files.
    pub frameworks: Vec<String>,
    pub git: Option<GitInfo>,
    pub services: Vec<Service>,
    pub containers: Vec<Container>,
    pub ports: Vec<u16>,
    pub has_compose: bool,
    pub compose_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntry {
    pub pid: i32,
    pub process: String,
    pub project: Option<String>,
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortConflict {
    pub port: u16,
    pub entries: Vec<ConflictEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DockerSummary {
    pub available: bool,
    pub reason: Option<String>,
    pub running: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub running_services: u32,
    pub listening_ports: u32,
    pub running_containers: u32,
    pub active_projects: u32,
    pub projects_total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub seq: u64,
    pub generated_at_ms: u64,
    pub scan_ms: u64,
    pub docker: DockerSummary,
    pub projects: Vec<ProjectView>,
    /// Dev-related listeners not linked to a discovered project.
    pub orphan_services: Vec<Service>,
    /// System / non-dev listeners (collapsed in the UI).
    pub other_listeners: Vec<Service>,
    pub unlinked_containers: Vec<Container>,
    pub conflicts: Vec<PortConflict>,
    pub totals: Totals,
    /// Non-fatal subsystem errors, surfaced in the UI footer.
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NotifyConfig {
    pub enabled: bool,
    pub service_stopped: bool,
    pub container_stopped: bool,
    pub health_failed: bool,
    pub port_conflict: bool,
    pub recovered: bool,
    /// Minimum seconds between identical notifications.
    pub cooldown_sec: u64,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_stopped: true,
            container_stopped: true,
            health_failed: true,
            port_conflict: true,
            recovered: true,
            cooldown_sec: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectOverride {
    pub name: Option<String>,
    /// service name -> start command override
    pub commands: BTreeMap<String, String>,
    pub health_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    /// Directories scanned for projects.
    pub roots: Vec<String>,
    pub poll_interval_ms: u64,
    pub docker_interval_ms: u64,
    pub docker_stats_interval_ms: u64,
    pub git_interval_ms: u64,
    pub discovery_interval_ms: u64,
    pub http_health: bool,
    pub notifications: NotifyConfig,
    /// "system" | "dark" | "light"
    pub theme: String,
    pub editor_app: String,
    pub terminal_app: String,
    pub show_other_listeners: bool,
    pub show_idle_projects: bool,
    pub hidden_projects: Vec<String>,
    pub project_overrides: BTreeMap<String, ProjectOverride>,
    pub pinned: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            poll_interval_ms: 3000,
            docker_interval_ms: 5000,
            docker_stats_interval_ms: 15000,
            git_interval_ms: 20000,
            discovery_interval_ms: 600_000,
            http_health: true,
            notifications: NotifyConfig::default(),
            theme: "system".into(),
            editor_app: "Cursor".into(),
            terminal_app: "Terminal".into(),
            show_other_listeners: true,
            show_idle_projects: true,
            hidden_projects: Vec::new(),
            project_overrides: BTreeMap::new(),
            pinned: false,
        }
    }
}

impl AppConfig {
    /// Clamp user-provided values into safe ranges.
    pub fn sanitize(&mut self) {
        self.poll_interval_ms = self.poll_interval_ms.clamp(1000, 60_000);
        self.docker_interval_ms = self.docker_interval_ms.clamp(2000, 120_000);
        self.docker_stats_interval_ms = self.docker_stats_interval_ms.clamp(5000, 300_000);
        self.git_interval_ms = self.git_interval_ms.clamp(5000, 600_000);
        self.discovery_interval_ms = self.discovery_interval_ms.clamp(60_000, 3_600_000);
        self.notifications.cooldown_sec = self.notifications.cooldown_sec.clamp(10, 3600);
        if !matches!(self.theme.as_str(), "system" | "dark" | "light") {
            self.theme = "system".into();
        }
        self.roots.retain(|r| !r.trim().is_empty());
        self.roots.dedup();
    }
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub at_ms: u64,
    pub line: String,
    pub stderr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogSessionInfo {
    pub session_id: String,
    pub title: String,
    pub source: String,
}
