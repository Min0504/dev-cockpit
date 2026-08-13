//! Docker integration via the `docker` CLI (respects contexts: Docker
//! Desktop, OrbStack, Colima…). Degrades gracefully when the CLI is missing
//! or the daemon is not running.

use std::collections::HashMap;
use std::time::Duration;

use crate::models::{Container, PortBinding};
use crate::util::run_cmd;

const SEP: char = '\u{1f}'; // unit separator, cannot appear in docker fields

#[derive(Debug, Clone, Default)]
pub struct DockerState {
    pub available: bool,
    pub reason: Option<String>,
    pub containers: Vec<Container>,
}

/// One-line-per-container format string. `--format` go-template with explicit
/// label lookups; avoids parsing the comma-joined Labels blob.
fn ps_format() -> String {
    [
        "{{.ID}}",
        "{{.Names}}",
        "{{.Image}}",
        "{{.State}}",
        "{{.Status}}",
        "{{.Ports}}",
        "{{.RunningFor}}",
        r#"{{.Label "com.docker.compose.project"}}"#,
        r#"{{.Label "com.docker.compose.service"}}"#,
        r#"{{.Label "com.docker.compose.project.working_dir"}}"#,
    ]
    .join(&SEP.to_string())
}

pub fn probe() -> DockerState {
    match run_cmd(
        "docker",
        &["version", "--format", "{{.Server.Version}}"],
        Duration::from_secs(4),
    ) {
        Err(_) => DockerState {
            available: false,
            reason: Some("docker CLI not found".into()),
            containers: Vec::new(),
        },
        Ok(out) if out.timed_out => DockerState {
            available: false,
            reason: Some("docker daemon not responding".into()),
            containers: Vec::new(),
        },
        Ok(out) if !out.ok() => DockerState {
            available: false,
            reason: Some("docker daemon not running".into()),
            containers: Vec::new(),
        },
        Ok(_) => DockerState {
            available: true,
            reason: None,
            containers: Vec::new(),
        },
    }
}

pub fn list_containers() -> Result<Vec<Container>, String> {
    let fmt = ps_format();
    let out = run_cmd(
        "docker",
        &["ps", "-a", "--no-trunc", "--format", &fmt],
        Duration::from_secs(8),
    )
    .map_err(|e| format!("docker ps: {e}"))?;
    if out.timed_out {
        return Err("docker ps timed out".into());
    }
    if out.status != Some(0) {
        return Err(format!(
            "docker ps failed: {}",
            out.stderr.lines().next().unwrap_or("unknown error")
        ));
    }
    Ok(parse_ps(&out.stdout))
}

pub fn parse_ps(raw: &str) -> Vec<Container> {
    let mut list = Vec::new();
    for line in raw.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        if f.len() < 10 {
            continue;
        }
        let status_text = f[4].to_string();
        list.push(Container {
            id: f[0].to_string(),
            name: f[1].to_string(),
            image: f[2].to_string(),
            state: f[3].to_string(),
            health: parse_health(&status_text),
            status_text,
            ports: parse_ports(f[5]),
            running_for: none_if_empty(f[6]),
            compose_project: none_if_empty(f[7]),
            compose_service: none_if_empty(f[8]),
            compose_dir: none_if_empty(f[9]),
            cpu: None,
            mem_bytes: None,
            mem_limit_bytes: None,
            project_path: None,
        });
    }
    // Stable order: running first, then by name.
    list.sort_by(|a, b| {
        let ra = a.state != "running";
        let rb = b.state != "running";
        ra.cmp(&rb).then_with(|| a.name.cmp(&b.name))
    });
    list
}

fn none_if_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

/// "Up 2 hours (healthy)" -> healthy; "(health: starting)" -> starting
fn parse_health(status: &str) -> Option<String> {
    let s = status.to_lowercase();
    if s.contains("(healthy)") {
        Some("healthy".into())
    } else if s.contains("(unhealthy)") {
        Some("unhealthy".into())
    } else if s.contains("health: starting") {
        Some("starting".into())
    } else {
        None
    }
}

/// "0.0.0.0:55432->5432/tcp, [::]:55432->5432/tcp, 6379/tcp"
pub fn parse_ports(raw: &str) -> Vec<PortBinding> {
    let mut out: Vec<PortBinding> = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (mapping, proto) = match part.rsplit_once('/') {
            Some((m, p)) => (m, p.to_string()),
            None => (part, "tcp".to_string()),
        };
        let (host, container) = match mapping.split_once("->") {
            Some((h, c)) => {
                let host_port = h.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok());
                (host_port, c.parse::<u16>().unwrap_or(0))
            }
            None => (None, mapping.parse::<u16>().unwrap_or(0)),
        };
        if container == 0 && host.is_none() {
            continue;
        }
        // Dedup v4/v6 duplicates of the same mapping.
        if !out.iter().any(|b: &PortBinding| b.host == host && b.container == container) {
            out.push(PortBinding { host, container, proto });
        }
    }
    out.sort_by_key(|b| (b.host.unwrap_or(u16::MAX), b.container));
    out
}

#[derive(Debug, Clone, Default)]
pub struct ContainerStats {
    pub cpu: Option<f32>,
    pub mem_bytes: Option<u64>,
    pub mem_limit_bytes: Option<u64>,
}

/// `docker stats --no-stream` — one sample, ~0.5-2s. Run on a slower cadence.
pub fn sample_stats() -> Result<HashMap<String, ContainerStats>, String> {
    let out = run_cmd(
        "docker",
        &[
            "stats",
            "--no-stream",
            "--format",
            "{{.ID}}\u{1f}{{.CPUPerc}}\u{1f}{{.MemUsage}}",
        ],
        Duration::from_secs(15),
    )
    .map_err(|e| format!("docker stats: {e}"))?;
    if out.timed_out || out.status != Some(0) {
        return Err("docker stats failed".into());
    }
    Ok(parse_stats(&out.stdout))
}

pub fn parse_stats(raw: &str) -> HashMap<String, ContainerStats> {
    let mut map = HashMap::new();
    for line in raw.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        if f.len() < 3 {
            continue;
        }
        let cpu = f[1].trim().trim_end_matches('%').parse::<f32>().ok();
        let (mem, limit) = match f[2].split_once('/') {
            Some((m, l)) => (parse_mem(m), parse_mem(l)),
            None => (parse_mem(f[2]), None),
        };
        map.insert(
            f[0].to_string(),
            ContainerStats { cpu, mem_bytes: mem, mem_limit_bytes: limit },
        );
    }
    map
}

/// "10.55MiB" / "1.2GiB" / "512KiB" / "1.5GB" -> bytes
fn parse_mem(s: &str) -> Option<u64> {
    let t = s.trim();
    let split = t.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = t.split_at(split);
    let v: f64 = num.trim().parse().ok()?;
    let mult: f64 = match unit.trim().to_lowercase().as_str() {
        "b" => 1.0,
        "kb" => 1e3,
        "kib" => 1024.0,
        "mb" => 1e6,
        "mib" => 1024.0 * 1024.0,
        "gb" => 1e9,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1e12,
        "tib" => 1024f64.powi(4),
        _ => return None,
    };
    Some((v * mult) as u64)
}

/// start / stop / restart a container. `stop` uses a short timeout so the UI
/// stays responsive; docker falls back to SIGKILL itself.
pub fn container_action(id: &str, action: &str) -> Result<(), String> {
    let args: Vec<&str> = match action {
        "start" => vec!["start", id],
        "stop" => vec!["stop", "-t", "5", id],
        "restart" => vec!["restart", "-t", "5", id],
        _ => return Err(format!("unsupported docker action: {action}")),
    };
    let out = run_cmd("docker", &args, Duration::from_secs(30))
        .map_err(|e| format!("docker {action}: {e}"))?;
    if out.timed_out {
        return Err(format!("docker {action} timed out"));
    }
    if out.status != Some(0) {
        return Err(format!(
            "docker {action} failed: {}",
            out.stderr.lines().next().unwrap_or("unknown error")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ps_line() {
        let raw = format!(
            "7670ce{s}acme-dev-postgres{s}postgres:16-alpine{s}running{s}Up 2 hours (healthy){s}0.0.0.0:55432->5432/tcp, [::]:55432->5432/tcp{s}2 hours ago{s}acme{s}postgres{s}/Users/x/Dev/acme{s}\nabc{s}one-off{s}redis:7{s}exited{s}Exited (0) 3 days ago{s}{s}3 days ago{s}{s}{s}{s}\n",
            s = SEP
        );
        let cs = parse_ps(&raw);
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].name, "acme-dev-postgres");
        assert_eq!(cs[0].health.as_deref(), Some("healthy"));
        assert_eq!(cs[0].ports, vec![PortBinding { host: Some(55432), container: 5432, proto: "tcp".into() }]);
        assert_eq!(cs[0].compose_project.as_deref(), Some("acme"));
        assert_eq!(cs[0].compose_dir.as_deref(), Some("/Users/x/Dev/acme"));
        assert_eq!(cs[1].state, "exited");
        assert!(cs[1].compose_project.is_none());
    }

    #[test]
    fn parses_port_strings() {
        assert_eq!(
            parse_ports("0.0.0.0:3000->3000/tcp, [::]:3000->3000/tcp"),
            vec![PortBinding { host: Some(3000), container: 3000, proto: "tcp".into() }]
        );
        assert_eq!(
            parse_ports("6379/tcp"),
            vec![PortBinding { host: None, container: 6379, proto: "tcp".into() }]
        );
        assert!(parse_ports("").is_empty());
    }

    #[test]
    fn parses_stats() {
        let raw = format!("abc123{s}0.15%{s}10.55MiB / 7.653GiB\n", s = SEP);
        let m = parse_stats(&raw);
        let st = &m["abc123"];
        assert_eq!(st.cpu, Some(0.15));
        assert_eq!(st.mem_bytes, Some((10.55 * 1024.0 * 1024.0) as u64));
        assert!(st.mem_limit_bytes.unwrap() > 8_000_000_000 / 2);
    }

    #[test]
    fn parses_mem_units() {
        assert_eq!(parse_mem("512KiB"), Some(512 * 1024));
        assert_eq!(parse_mem("1.5GB"), Some(1_500_000_000));
        assert_eq!(parse_mem("junk"), None);
    }
}
