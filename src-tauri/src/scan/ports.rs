//! Listening TCP port scan via `lsof` (macOS).
//!
//! Uses `-F` machine-readable output so process names containing spaces
//! ("Google Chrome Helper") parse correctly — a bug in the old port-map.

use std::collections::HashSet;
use std::time::Duration;

use crate::util::run_cmd;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub pid: i32,
    pub process: String,
    /// Bind address as reported by lsof, e.g. `*:5173`, `127.0.0.1:8080`, `[::1]:6379`.
    pub addr: String,
    pub port: u16,
    pub ipv6: bool,
    /// Loopback-only bind (127.0.0.1 / ::1) vs wildcard / external interface.
    pub loopback: bool,
}

pub fn scan() -> Result<Vec<Listener>, String> {
    let out = run_cmd(
        "lsof",
        &["-nP", "-iTCP", "-sTCP:LISTEN", "-FpcnL"],
        Duration::from_secs(6),
    )
    .map_err(|e| format!("lsof: {e}"))?;
    if out.timed_out {
        return Err("lsof timed out".into());
    }
    // lsof exits 1 when nothing matches — treat empty output as empty result.
    Ok(parse(&out.stdout))
}

pub fn parse(raw: &str) -> Vec<Listener> {
    let mut result = Vec::new();
    let mut seen: HashSet<(i32, u16, String)> = HashSet::new();
    let mut pid: i32 = -1;
    let mut cmd = String::new();

    for line in raw.lines() {
        let Some(tag) = line.chars().next() else { continue };
        let val = &line[1..];
        match tag {
            'p' => {
                pid = val.parse().unwrap_or(-1);
                cmd.clear();
            }
            'c' => cmd = val.to_string(),
            'n' => {
                if pid <= 0 {
                    continue;
                }
                let Some(port) = parse_port(val) else { continue };
                let key = (pid, port, val.to_string());
                if !seen.insert(key) {
                    continue;
                }
                let ipv6 = val.starts_with('[');
                let host = val.rsplit_once(':').map(|(h, _)| h).unwrap_or("");
                let loopback = matches!(host, "127.0.0.1" | "[::1]" | "localhost");
                result.push(Listener {
                    pid,
                    process: decode_lsof_name(&cmd),
                    addr: val.to_string(),
                    port,
                    ipv6,
                    loopback,
                });
            }
            _ => {}
        }
    }
    result.sort_by_key(|l| (l.port, l.pid, l.ipv6));
    result
}

/// lsof -F encodes some characters as `\xNN`; command names may also contain
/// literal spaces which are preserved in field mode.
fn decode_lsof_name(s: &str) -> String {
    if !s.contains("\\x") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() && bytes[i + 1] == b'x' {
            if let Ok(code) = u8::from_str_radix(&s[i + 2..i + 4], 16) {
                out.push(code as char);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn parse_port(addr: &str) -> Option<u16> {
    // formats: `*:8080`, `127.0.0.1:3000`, `[::1]:6379`, `[fe80::1]:80`
    let (_, port) = addr.rsplit_once(':')?;
    port.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "p461\ncrapportd\nLdev\nf8\nn*:49152\np1234\ncnode\nLdev\nf22\nn*:5173\nf23\nn*:5173\np5678\ncGoogle Chrome H\nLdev\nf30\nn127.0.0.1:9222\np9012\ncpostgres\nLdev\nf7\nn127.0.0.1:5432\nf8\nn[::1]:5432\n";

    #[test]
    fn parses_field_output() {
        let ls = parse(SAMPLE);
        // node dedupes to one entry for identical (pid, port, addr)
        let node: Vec<_> = ls.iter().filter(|l| l.process == "node").collect();
        assert_eq!(node.len(), 1);
        assert_eq!(node[0].port, 5173);
        assert!(!node[0].loopback);

        let pg: Vec<_> = ls.iter().filter(|l| l.process == "postgres").collect();
        assert_eq!(pg.len(), 2); // v4 + v6 binds
        assert!(pg.iter().all(|l| l.port == 5432 && l.loopback));
        assert!(pg.iter().any(|l| l.ipv6));

        let chrome: Vec<_> = ls.iter().filter(|l| l.pid == 5678).collect();
        assert_eq!(chrome[0].process, "Google Chrome H");
        assert_eq!(chrome[0].port, 9222);
    }

    #[test]
    fn parses_ports() {
        assert_eq!(parse_port("*:8080"), Some(8080));
        assert_eq!(parse_port("[::]:3000"), Some(3000));
        assert_eq!(parse_port("127.0.0.1:65535"), Some(65535));
        assert_eq!(parse_port("garbage"), None);
    }

    #[test]
    fn decodes_escapes() {
        assert_eq!(decode_lsof_name("foo\\x20bar"), "foo bar");
        assert_eq!(decode_lsof_name("plain"), "plain");
    }
}
