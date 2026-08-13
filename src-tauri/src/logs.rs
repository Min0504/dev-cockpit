//! Log streaming: ring-buffered sessions fed by `docker logs -f` or by the
//! stdout/stderr of processes Dev Cockpit spawned. Lines are pushed to the
//! frontend in small batches via events (`logs://<session-id>`).

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::models::{LogLine, LogSessionInfo};
use crate::util::now_ms;

const MAX_LINES: usize = 2000;
const MAX_LINE_CHARS: usize = 4000;
const FLUSH_INTERVAL: Duration = Duration::from_millis(120);

struct Session {
    info: LogSessionInfo,
    buf: VecDeque<LogLine>,
    seq: u64,
    ended: bool,
    /// pid of a `docker logs -f` reader child (killed on close).
    reader_pid: Option<i32>,
    pending: Vec<LogLine>,
    last_flush: Instant,
}

#[derive(Clone)]
pub struct LogRegistry {
    inner: Arc<Mutex<HashMap<String, Session>>>,
}

impl Default for LogRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LogRegistry {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Background flusher — emits pending batches even when a stream goes
    /// quiet between reads. Spawn once at app setup.
    pub fn spawn_flusher(&self, app: AppHandle) {
        let inner = Arc::clone(&self.inner);
        std::thread::Builder::new()
            .name("log-flusher".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_millis(150));
                let mut sessions = match inner.lock() {
                    Ok(g) => g,
                    Err(_) => continue,
                };
                for (id, s) in sessions.iter_mut() {
                    if !s.pending.is_empty() && s.last_flush.elapsed() >= FLUSH_INTERVAL {
                        let batch = std::mem::take(&mut s.pending);
                        s.last_flush = Instant::now();
                        let _ = app.emit(&format!("logs://{id}"), &batch);
                    }
                }
            })
            .expect("spawn log flusher");
    }

    fn create(&self, id: &str, title: &str, source: &str, reader_pid: Option<i32>) {
        let mut sessions = self.inner.lock().expect("logs lock");
        sessions.insert(
            id.to_string(),
            Session {
                info: LogSessionInfo {
                    session_id: id.to_string(),
                    title: title.to_string(),
                    source: source.to_string(),
                },
                buf: VecDeque::with_capacity(256),
                seq: 0,
                ended: false,
                reader_pid,
                pending: Vec::new(),
                last_flush: Instant::now(),
            },
        );
        // Bound the number of live sessions.
        if sessions.len() > 12 {
            let victim = sessions
                .iter()
                .filter(|(_, s)| s.ended)
                .map(|(k, _)| k.clone())
                .next();
            if let Some(k) = victim {
                sessions.remove(&k);
            }
        }
    }

    pub fn push(&self, app: &AppHandle, id: &str, raw: &str, stderr: bool) {
        let mut sessions = self.inner.lock().expect("logs lock");
        let Some(s) = sessions.get_mut(id) else { return };
        s.seq += 1;
        let line = LogLine {
            seq: s.seq,
            at_ms: now_ms(),
            line: sanitize_line(raw),
            stderr,
        };
        if s.buf.len() >= MAX_LINES {
            s.buf.pop_front();
        }
        s.buf.push_back(line.clone());
        s.pending.push(line);
        if s.last_flush.elapsed() >= FLUSH_INTERVAL || s.pending.len() >= 64 {
            let batch = std::mem::take(&mut s.pending);
            s.last_flush = Instant::now();
            let _ = app.emit(&format!("logs://{id}"), &batch);
        }
    }

    pub fn mark_ended(&self, app: &AppHandle, id: &str) {
        let mut sessions = self.inner.lock().expect("logs lock");
        if let Some(s) = sessions.get_mut(id) {
            s.ended = true;
            let batch = std::mem::take(&mut s.pending);
            drop(sessions);
            if !batch.is_empty() {
                let _ = app.emit(&format!("logs://{id}"), &batch);
            }
            let _ = app.emit(&format!("logs-ended://{id}"), ());
        }
    }

    pub fn lines(&self, id: &str) -> Option<(LogSessionInfo, Vec<LogLine>, bool)> {
        let sessions = self.inner.lock().expect("logs lock");
        sessions
            .get(id)
            .map(|s| (s.info.clone(), s.buf.iter().cloned().collect(), s.ended))
    }

    pub fn find_by_source(&self, source: &str) -> Option<String> {
        let sessions = self.inner.lock().expect("logs lock");
        sessions
            .iter()
            .find(|(_, s)| s.info.source == source && !s.ended)
            .map(|(k, _)| k.clone())
    }

    /// Close a session. Docker reader children are killed; managed-process
    /// sessions stay alive (the service keeps running and logging).
    pub fn close(&self, id: &str) {
        let mut sessions = self.inner.lock().expect("logs lock");
        let Some(s) = sessions.get_mut(id) else { return };
        if let Some(pid) = s.reader_pid {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            sessions.remove(id);
        }
        // managed sessions: keep buffering in the background
    }

    /// Create a session backed by `docker logs -f`. Reuses a live session for
    /// the same container.
    pub fn open_docker(&self, app: &AppHandle, container_id: &str, title: &str) -> Result<String, String> {
        let source = format!("docker:{container_id}");
        if let Some(existing) = self.find_by_source(&source) {
            return Ok(existing);
        }
        let mut child = Command::new("docker")
            .args(["logs", "-f", "--tail", "300", container_id])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("docker logs: {e}"))?;
        let id = format!("d-{}-{}", &container_id[..container_id.len().min(12)], now_ms() % 100_000);
        self.create(&id, title, &source, Some(child.id() as i32));

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        self.spawn_reader(app.clone(), id.clone(), stdout, false);
        self.spawn_reader(app.clone(), id.clone(), stderr, true);

        // Reap the reader child and mark the session ended at EOF.
        let registry = self.clone();
        let app2 = app.clone();
        let sid = id.clone();
        std::thread::Builder::new()
            .name("docker-log-wait".into())
            .spawn(move || {
                let _ = child.wait();
                registry.mark_ended(&app2, &sid);
            })
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// Create a session for a process we spawned; the caller pumps output via
    /// the returned reader threads.
    pub fn open_managed(
        &self,
        app: &AppHandle,
        managed_id: &str,
        title: &str,
        stdout: Option<std::process::ChildStdout>,
        stderr: Option<std::process::ChildStderr>,
    ) -> String {
        let id = format!("m-{managed_id}");
        self.create(&id, title, &format!("managed:{managed_id}"), None);
        self.spawn_reader(app.clone(), id.clone(), stdout, false);
        self.spawn_reader(app.clone(), id.clone(), stderr, true);
        id
    }

    fn spawn_reader<R: std::io::Read + Send + 'static>(
        &self,
        app: AppHandle,
        id: String,
        pipe: Option<R>,
        is_stderr: bool,
    ) {
        let Some(pipe) = pipe else { return };
        let registry = self.clone();
        std::thread::Builder::new()
            .name(format!("log-reader-{id}"))
            .spawn(move || {
                let reader = BufReader::new(pipe);
                for line in reader.split(b'\n') {
                    match line {
                        Ok(bytes) => {
                            let text = String::from_utf8_lossy(&bytes);
                            registry.push(&app, &id, &text, is_stderr);
                        }
                        Err(_) => break,
                    }
                }
            })
            .expect("spawn log reader");
    }
}

/// Strip ANSI escape sequences and control chars; cap the length.
pub fn sanitize_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_LINE_CHARS));
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI / OSC sequences
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for t in chars.by_ref() {
                        if t.is_ascii_alphabetic() || t == '~' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(t) = chars.next() {
                        if t == '\u{7}' {
                            break;
                        }
                        if t == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        if c == '\r' || c == '\u{7}' {
            continue;
        }
        if out.chars().count() >= MAX_LINE_CHARS {
            break;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi() {
        assert_eq!(sanitize_line("\u{1b}[32mready\u{1b}[0m in 300ms"), "ready in 300ms");
        assert_eq!(sanitize_line("plain"), "plain");
        assert_eq!(sanitize_line("a\rb"), "ab");
        assert_eq!(
            sanitize_line("\u{1b}]0;title\u{7}text"),
            "text"
        );
    }
}
