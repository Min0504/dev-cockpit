//! Git status collection. Read-only, never takes locks
//! (GIT_OPTIONAL_LOCKS=0), hard 4s timeout per repository.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::models::{CommitInfo, GitInfo};

fn git(path: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let deadline = std::time::Instant::now() + timeout;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    });
    loop {
        match child.try_wait().ok()? {
            Some(st) => {
                let out = reader.join().unwrap_or_default();
                return if st.success() { Some(out) } else { None };
            }
            None => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Collect branch / dirty count / ahead-behind / last commit for a repo.
/// Returns None when the path is not a git repository (or was deleted).
pub fn collect(path: &str) -> Option<GitInfo> {
    if !Path::new(path).join(".git").exists() {
        return None;
    }
    let status = git(
        path,
        &["status", "--porcelain=v2", "--branch"],
        Duration::from_secs(4),
    )?;
    let mut info = parse_status(&status);

    if let Some(log) = git(
        path,
        &["log", "-1", "--format=%H\u{1f}%s\u{1f}%an\u{1f}%ct"],
        Duration::from_secs(4),
    ) {
        info.last_commit = parse_log(&log);
    }
    Some(info)
}

pub fn parse_status(raw: &str) -> GitInfo {
    let mut branch = String::from("(unknown)");
    let mut ahead = 0i32;
    let mut behind = 0i32;
    let mut dirty = 0u32;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            for tok in rest.split_whitespace() {
                if let Some(n) = tok.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = tok.strip_prefix('-') {
                    behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            dirty += 1;
        }
    }
    GitInfo { branch, dirty_count: dirty, ahead, behind, last_commit: None }
}

pub fn parse_log(raw: &str) -> Option<CommitInfo> {
    let f: Vec<&str> = raw.trim().split('\u{1f}').collect();
    if f.len() < 4 {
        return None;
    }
    Some(CommitInfo {
        hash: f[0].chars().take(10).collect(),
        summary: f[1].to_string(),
        author: f[2].to_string(),
        epoch_sec: f[3].parse().unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_v2() {
        let raw = "\
# branch.oid 0f0e1c2\n\
# branch.head feature/dashboard\n\
# branch.upstream origin/feature/dashboard\n\
# branch.ab +2 -1\n\
1 .M N... 100644 100644 100644 aaa bbb src/App.tsx\n\
? new-file.ts\n";
        let g = parse_status(raw);
        assert_eq!(g.branch, "feature/dashboard");
        assert_eq!(g.ahead, 2);
        assert_eq!(g.behind, 1);
        assert_eq!(g.dirty_count, 2);
    }

    #[test]
    fn parses_detached() {
        let g = parse_status("# branch.oid abc\n# branch.head (detached)\n");
        assert_eq!(g.branch, "(detached)");
        assert_eq!(g.dirty_count, 0);
    }

    #[test]
    fn parses_log_line() {
        let c = parse_log("deadbeefcafe1234\u{1f}fix: things\u{1f}Alice\u{1f}1723500000\n").unwrap();
        assert_eq!(c.hash, "deadbeefca");
        assert_eq!(c.summary, "fix: things");
        assert_eq!(c.epoch_sec, 1723500000);
    }
}
