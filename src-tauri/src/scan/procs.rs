//! Process enrichment via sysinfo: cmdline, cwd, CPU, memory, uptime.
//!
//! Only the PIDs that currently hold listening sockets (plus managed
//! children) are refreshed — never the full process table.

use std::collections::HashMap;
use std::path::PathBuf;

use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone)]
pub struct ProcInfo {
    #[allow(dead_code)] // useful in Debug output / tests
    pub pid: i32,
    pub name: String,
    pub cmd: String,
    pub cwd: Option<PathBuf>,
    pub cpu: f32,
    pub mem_bytes: u64,
    pub run_time_sec: u64,
    pub zombie: bool,
}

pub struct ProcScanner {
    sys: System,
}

impl Default for ProcScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcScanner {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    /// Refresh the given pids and return their info.
    pub fn refresh(&mut self, pids: &[i32]) -> HashMap<i32, ProcInfo> {
        if pids.is_empty() {
            return HashMap::new();
        }
        let sys_pids: Vec<Pid> = pids
            .iter()
            .filter(|&&p| p > 0)
            .map(|&p| Pid::from_u32(p as u32))
            .collect();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&sys_pids),
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet),
        );

        let mut out = HashMap::new();
        for pid in &sys_pids {
            let Some(p) = self.sys.process(*pid) else { continue };
            let cmd = if p.cmd().is_empty() {
                p.name().to_string_lossy().to_string()
            } else {
                p.cmd()
                    .iter()
                    .map(|c| c.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            out.insert(
                pid.as_u32() as i32,
                ProcInfo {
                    pid: pid.as_u32() as i32,
                    name: p.name().to_string_lossy().to_string(),
                    cmd,
                    cwd: p.cwd().map(|c| c.to_path_buf()),
                    cpu: p.cpu_usage(),
                    mem_bytes: p.memory(),
                    run_time_sec: p.run_time(),
                    zombie: matches!(p.status(), ProcessStatus::Zombie),
                },
            );
        }
        out
    }
}

/// Process group id for a pid (0 on failure). Used to associate a scanned
/// listener with a managed child we spawned (same process group).
pub fn pgid_of(pid: i32) -> i32 {
    if pid <= 0 {
        return 0;
    }
    let r = unsafe { libc::getpgid(pid) };
    if r < 0 { 0 } else { r }
}

/// Is the process still alive? (signal 0 probe)
pub fn alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Is any process in the group still alive?
pub fn group_alive(pgid: i32) -> bool {
    if pgid <= 1 {
        return false;
    }
    unsafe { libc::kill(-pgid, 0) == 0 }
}
