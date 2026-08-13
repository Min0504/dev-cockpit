//! Project discovery & static analysis.
//!
//! Walks the configured root directories (bounded depth / count), treats a
//! directory as a project when it has a `.git` or a recognizable manifest,
//! then extracts: frameworks, runnable services (package scripts, workspaces,
//! compose, Django), and a map of sub-package dirs used to name running
//! processes by their cwd.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use super::detect;

const MAX_DEPTH: usize = 4;
const MAX_DIRS: usize = 6000;
const MAX_WORKSPACE_DIRS: usize = 40;

const SKIP_DIRS: &[&str] = &[
    "node_modules", "vendor", "dist", "build", "out", "target", ".next", ".nuxt", ".output",
    ".venv", "venv", "__pycache__", ".cache", "coverage", "Pods", "DerivedData", ".terraform",
    ".turbo", ".expo", ".yarn", ".pnpm-store", "Library", "Applications", ".Trash",
];

#[derive(Debug, Clone, PartialEq)]
pub struct SubPackage {
    pub dir: String,
    pub name: String,
    pub framework_key: Option<String>,
    pub framework_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartableSpec {
    /// Display name — sub-package name or the project name for the root.
    pub name: String,
    /// Directory the command runs in.
    pub dir: String,
    pub command: String,
    pub framework_key: Option<String>,
    pub framework_label: Option<String>,
    pub is_http: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectStatic {
    pub path: String,
    pub name: String,
    pub package_name: Option<String>,
    /// (key, label) pairs, deduped, priority-ordered.
    pub frameworks: Vec<(String, String)>,
    pub startables: Vec<StartableSpec>,
    pub compose_file: Option<String>,
    pub compose_services: Vec<String>,
    pub subpackages: Vec<SubPackage>,
    pub has_git: bool,
}

#[derive(Debug, Default)]
pub struct DiscoveryResult {
    pub projects: Vec<ProjectStatic>,
    pub errors: Vec<String>,
}

pub fn discover(roots: &[String]) -> DiscoveryResult {
    let mut result = DiscoveryResult::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut visited = 0usize;

    for root in roots {
        let root_path = PathBuf::from(shellexpand_home(root));
        let Ok(canon_root) = root_path.canonicalize() else {
            result.errors.push(format!("root not found: {root}"));
            continue;
        };
        if !canon_root.is_dir() {
            result.errors.push(format!("root is not a directory: {root}"));
            continue;
        }

        let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
        queue.push_back((canon_root, 0));

        while let Some((dir, depth)) = queue.pop_front() {
            visited += 1;
            if visited > MAX_DIRS {
                result.errors.push("project scan truncated (too many directories)".into());
                break;
            }
            if is_project_dir(&dir) {
                let key = dir.to_string_lossy().to_string();
                if seen.insert(key) {
                    result.projects.push(analyze(&dir));
                }
                continue; // don't descend into projects
            }
            if depth >= MAX_DEPTH {
                continue;
            }
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let Ok(ft) = entry.file_type() else { continue };
                if !ft.is_dir() || ft.is_symlink() {
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') || SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                    continue;
                }
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }
    result.projects.sort_by_key(|p| p.name.to_lowercase());
    result
}

pub fn shellexpand_home(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        return format!("{}/{}", crate::util::home_dir(), rest);
    }
    if p == "~" {
        return crate::util::home_dir();
    }
    p.to_string()
}

fn is_project_dir(dir: &Path) -> bool {
    const MARKERS: &[&str] = &[
        ".git", "package.json", "pyproject.toml", "docker-compose.yml", "docker-compose.yaml",
        "compose.yml", "compose.yaml", "go.mod", "Cargo.toml", "Gemfile", "mix.exs",
    ];
    MARKERS.iter().any(|m| dir.join(m).exists())
}

// ---------------------------------------------------------------------------
// Static analysis of a single project directory
// ---------------------------------------------------------------------------

pub fn analyze(dir: &Path) -> ProjectStatic {
    let path = dir.to_string_lossy().to_string();
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    let mut p = ProjectStatic {
        path: path.clone(),
        name,
        has_git: dir.join(".git").exists(),
        ..Default::default()
    };
    let mut fw: BTreeMap<String, (String, u8)> = BTreeMap::new(); // key -> (label, prio)
    let add_fw = |key: &str, label: &str, prio: u8, fw: &mut BTreeMap<String, (String, u8)>| {
        fw.entry(key.to_string())
            .and_modify(|e| {
                if prio < e.1 {
                    *e = (label.to_string(), prio);
                }
            })
            .or_insert((label.to_string(), prio));
    };

    let pm = detect_package_manager(dir);

    // --- root package.json ---
    let root_pkg = read_json(&dir.join("package.json"));
    if let Some(pkg) = &root_pkg {
        p.package_name = pkg.get("name").and_then(|v| v.as_str()).map(String::from);
        for dep in node_deps(pkg) {
            if let Some(det) = detect::from_node_dep(&dep) {
                add_fw(det.key, det.label, detect::node_dep_priority(det.key), &mut fw);
            }
        }
        if let Some((script, cmd)) = pick_script(pkg, &pm) {
            let det = script_framework(pkg);
            p.startables.push(StartableSpec {
                name: p
                    .package_name
                    .clone()
                    .map(short_pkg_name)
                    .unwrap_or_else(|| p.name.clone()),
                dir: path.clone(),
                command: cmd,
                framework_key: det.as_ref().map(|d| d.key.to_string()),
                framework_label: det.as_ref().map(|d| d.label.to_string()),
                is_http: det
                    .as_ref()
                    .map(|d| detect::is_http_category(d.category))
                    .unwrap_or(true),
            });
            let _ = script;
        }
    }

    // --- workspaces / sub-packages ---
    let ws_dirs = workspace_dirs(dir, root_pkg.as_ref());
    for sub in ws_dirs {
        let Some(pkg) = read_json(&sub.join("package.json")) else { continue };
        let sub_path = sub.to_string_lossy().to_string();
        if sub_path == path {
            continue;
        }
        let pkg_name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .map(short_pkg_name)
            .unwrap_or_else(|| {
                sub.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            });
        let det = script_framework(&pkg);
        for dep in node_deps(&pkg) {
            if let Some(d) = detect::from_node_dep(&dep) {
                add_fw(d.key, d.label, detect::node_dep_priority(d.key), &mut fw);
            }
        }
        p.subpackages.push(SubPackage {
            dir: sub_path.clone(),
            name: pkg_name.clone(),
            framework_key: det.as_ref().map(|d| d.key.to_string()),
            framework_label: det.as_ref().map(|d| d.label.to_string()),
        });
        if let Some((_, cmd)) = pick_script(&pkg, &pm) {
            p.startables.push(StartableSpec {
                name: pkg_name,
                dir: sub_path,
                command: cmd,
                framework_key: det.as_ref().map(|d| d.key.to_string()),
                framework_label: det.as_ref().map(|d| d.label.to_string()),
                is_http: det
                    .as_ref()
                    .map(|d| detect::is_http_category(d.category))
                    .unwrap_or(true),
            });
        }
    }

    // --- python ---
    let py_deps = python_deps(dir);
    for dep in &py_deps {
        if let Some(det) = detect::from_python_dep(dep) {
            add_fw(det.key, det.label, 1, &mut fw);
        }
    }
    if dir.join("manage.py").exists() {
        add_fw("django", "Django", 0, &mut fw);
        p.startables.push(StartableSpec {
            name: format!("{} (Django)", p.name),
            dir: path.clone(),
            command: "python manage.py runserver".into(),
            framework_key: Some("django".into()),
            framework_label: Some("Django".into()),
            is_http: true,
        });
    }

    // --- compose ---
    for cf in ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
        let f = dir.join(cf);
        if f.exists() {
            p.compose_file = Some(cf.to_string());
            let (services, images) = compose_services(&f);
            p.compose_services = services;
            for img in images {
                if let Some(det) = detect::from_docker_image(&img) {
                    add_fw(det.key, det.label, 6, &mut fw);
                }
            }
            add_fw("compose", "Docker Compose", 7, &mut fw);
            break;
        }
    }

    // --- other ecosystems ---
    if dir.join("go.mod").exists() {
        add_fw("go", "Go", 2, &mut fw);
    }
    if dir.join("Cargo.toml").exists() && !dir.join("package.json").exists() {
        add_fw("rust", "Rust", 2, &mut fw);
    }
    if dir.join("Gemfile").exists() {
        add_fw("ruby", "Ruby", 2, &mut fw);
    }
    if dir.join("mix.exs").exists() {
        add_fw("elixir", "Elixir", 2, &mut fw);
    }
    if !py_deps.is_empty() && fw.is_empty() {
        add_fw("python", "Python", 5, &mut fw);
    }

    let mut pairs: Vec<(String, String, u8)> =
        fw.into_iter().map(|(k, (l, pr))| (k, l, pr)).collect();
    pairs.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
    p.frameworks = pairs.into_iter().map(|(k, l, _)| (k, l)).take(6).collect();

    p.startables.truncate(MAX_WORKSPACE_DIRS);
    p
}

fn short_pkg_name(name: impl AsRef<str>) -> String {
    let n = name.as_ref();
    n.rsplit('/').next().unwrap_or(n).to_string()
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn node_deps(pkg: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for section in ["dependencies", "devDependencies"] {
        if let Some(map) = pkg.get(section).and_then(|v| v.as_object()) {
            out.extend(map.keys().cloned());
        }
    }
    out
}

/// Representative framework for a package (best-priority dep match).
fn script_framework(pkg: &serde_json::Value) -> Option<detect::Detected> {
    let mut best: Option<detect::Detected> = None;
    for dep in node_deps(pkg) {
        if let Some(det) = detect::from_node_dep(&dep) {
            let better = match &best {
                None => true,
                Some(b) => detect::node_dep_priority(det.key) < detect::node_dep_priority(b.key),
            };
            if better {
                best = Some(det);
            }
        }
    }
    best
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageManager {
    Pnpm,
    Yarn,
    Npm,
    Bun,
}

pub fn detect_package_manager(dir: &Path) -> PackageManager {
    if dir.join("pnpm-lock.yaml").exists() || dir.join("pnpm-workspace.yaml").exists() {
        PackageManager::Pnpm
    } else if dir.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        PackageManager::Bun
    } else {
        PackageManager::Npm
    }
}

/// Pick the best dev script and produce a runnable command.
fn pick_script(pkg: &serde_json::Value, pm: &PackageManager) -> Option<(String, String)> {
    let scripts = pkg.get("scripts")?.as_object()?;
    const PREFS: &[&str] = &["dev", "start:dev", "serve", "start"];
    let script = PREFS.iter().find(|s| scripts.contains_key(**s))?;
    let cmd = match pm {
        PackageManager::Pnpm => format!("pnpm run {script}"),
        PackageManager::Yarn => format!("yarn {script}"),
        PackageManager::Bun => format!("bun run {script}"),
        PackageManager::Npm => format!("npm run {script}"),
    };
    Some((script.to_string(), cmd))
}

/// Expand workspace globs from package.json `workspaces` and
/// pnpm-workspace.yaml `packages`. Only single-level `dir/*` patterns and
/// literal paths are supported (that covers real-world monorepos); `**` is
/// ignored. Falls back to conventional subdirs when no workspace config.
fn workspace_dirs(root: &Path, pkg: Option<&serde_json::Value>) -> Vec<PathBuf> {
    let mut patterns: Vec<String> = Vec::new();

    if let Some(pkg) = pkg {
        match pkg.get("workspaces") {
            Some(serde_json::Value::Array(a)) => {
                patterns.extend(a.iter().filter_map(|v| v.as_str().map(String::from)));
            }
            Some(serde_json::Value::Object(o)) => {
                if let Some(serde_json::Value::Array(a)) = o.get("packages") {
                    patterns.extend(a.iter().filter_map(|v| v.as_str().map(String::from)));
                }
            }
            _ => {}
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        if let Ok(serde_yaml::Value::Mapping(m)) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            if let Some(serde_yaml::Value::Sequence(seq)) =
                m.get(serde_yaml::Value::String("packages".into()))
            {
                patterns.extend(seq.iter().filter_map(|v| v.as_str().map(String::from)));
            }
        }
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    if patterns.is_empty() {
        // No workspace config — probe conventional service dirs.
        const CONVENTIONAL: &[&str] = &[
            "frontend", "backend", "client", "server", "web", "api", "app", "admin", "site", "docs",
        ];
        for d in CONVENTIONAL {
            let sub = root.join(d);
            if sub.join("package.json").exists() {
                dirs.push(sub);
            }
        }
    } else {
        for pat in patterns {
            let pat = pat.trim();
            if pat.starts_with('!') || pat.contains("**") {
                continue;
            }
            if let Some(parent) = pat.strip_suffix("/*") {
                let base = root.join(parent);
                if let Ok(entries) = fs::read_dir(&base) {
                    for e in entries.flatten() {
                        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            dirs.push(e.path());
                        }
                    }
                }
            } else {
                let d = root.join(pat);
                if d.is_dir() {
                    dirs.push(d);
                }
            }
            if dirs.len() >= MAX_WORKSPACE_DIRS {
                break;
            }
        }
    }
    dirs.truncate(MAX_WORKSPACE_DIRS);
    dirs
}

/// Compose service names + images (used to link containers and label DBs).
fn compose_services(file: &Path) -> (Vec<String>, Vec<String>) {
    let Ok(text) = fs::read_to_string(file) else { return (Vec::new(), Vec::new()) };
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return (Vec::new(), Vec::new());
    };
    let mut names = Vec::new();
    let mut images = Vec::new();
    if let Some(serde_yaml::Value::Mapping(services)) =
        doc.get(serde_yaml::Value::String("services".into()))
    {
        for (k, v) in services {
            if let Some(name) = k.as_str() {
                names.push(name.to_string());
            }
            if let Some(img) = v.get("image").and_then(|i| i.as_str()) {
                images.push(img.to_string());
            }
        }
    }
    (names, images)
}

/// Python dependency names from pyproject.toml / requirements.txt.
fn python_deps(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(text) = fs::read_to_string(dir.join("pyproject.toml")) {
        if let Ok(doc) = text.parse::<toml::Table>() {
            // PEP 621: [project] dependencies = ["fastapi>=0.100", ...]
            if let Some(deps) = doc
                .get("project")
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_array())
            {
                out.extend(deps.iter().filter_map(|d| d.as_str()).map(dep_name));
            }
            // poetry: [tool.poetry.dependencies] fastapi = "^0.100"
            if let Some(deps) = doc
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_table())
            {
                out.extend(deps.keys().cloned());
            }
        }
    }
    if let Ok(text) = fs::read_to_string(dir.join("requirements.txt")) {
        out.extend(
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
                .map(dep_name),
        );
    }
    out
}

fn dep_name(spec: impl AsRef<str>) -> String {
    let s = spec.as_ref();
    s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
        .map(|i| s[..i].to_string())
        .unwrap_or_else(|| s.to_string())
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_project(files: &[(&str, &str)]) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dc-test-{}-{}-{n}",
            std::process::id(),
            crate::util::now_ms()
        ));
        for (rel, content) in files {
            let f = dir.join(rel);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(&f, content).unwrap();
        }
        dir
    }

    #[test]
    fn analyzes_monorepo() {
        let dir = tmp_project(&[
            (
                "package.json",
                r#"{"name":"todayskin","private":true,"workspaces":["apps/*"],"scripts":{}}"#,
            ),
            ("pnpm-lock.yaml", ""),
            (
                "apps/web/package.json",
                r#"{"name":"@todayskin/web","scripts":{"dev":"vite"},"dependencies":{"react":"^19"},"devDependencies":{"vite":"^7"}}"#,
            ),
            (
                "apps/api/package.json",
                r#"{"name":"@todayskin/api","scripts":{"start:dev":"nest start --watch"},"dependencies":{"@nestjs/core":"^10"}}"#,
            ),
            (
                "docker-compose.yml",
                "services:\n  postgres:\n    image: postgres:16-alpine\n  redis:\n    image: redis:7\n",
            ),
        ]);
        let p = analyze(&dir);
        assert_eq!(p.compose_file.as_deref(), Some("docker-compose.yml"));
        assert_eq!(p.compose_services, vec!["postgres", "redis"]);
        assert!(p.frameworks.iter().any(|(k, _)| k == "vite"));
        assert!(p.frameworks.iter().any(|(k, _)| k == "nestjs"));
        assert!(p.frameworks.iter().any(|(k, _)| k == "postgres"));

        let names: Vec<_> = p.startables.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"web"), "startables: {names:?}");
        assert!(names.contains(&"api"));
        let web = p.startables.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(web.command, "pnpm run dev");
        assert_eq!(web.framework_key.as_deref(), Some("vite"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn analyzes_python_fastapi() {
        let dir = tmp_project(&[(
            "pyproject.toml",
            "[project]\nname = \"svc\"\ndependencies = [\"fastapi>=0.110\", \"uvicorn[standard]\"]\n",
        )]);
        let p = analyze(&dir);
        assert!(p.frameworks.iter().any(|(k, _)| k == "fastapi"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn conventional_dirs_without_workspaces() {
        let dir = tmp_project(&[
            ("package.json", r#"{"name":"root","scripts":{"dev":"concurrently"}}"#),
            (
                "frontend/package.json",
                r#"{"name":"frontend","scripts":{"dev":"vite"},"devDependencies":{"vite":"1"}}"#,
            ),
            (
                "backend/package.json",
                r#"{"name":"backend","scripts":{"start":"node index.js"},"dependencies":{"express":"4"}}"#,
            ),
        ]);
        let p = analyze(&dir);
        let names: Vec<_> = p.startables.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"frontend"));
        assert!(names.contains(&"backend"));
        assert!(names.contains(&"root"));
        let be = p.startables.iter().find(|s| s.name == "backend").unwrap();
        assert_eq!(be.command, "npm run start");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discovery_finds_and_stops_at_projects() {
        let base = tmp_project(&[
            ("group/proj-a/package.json", r#"{"name":"a"}"#),
            ("group/proj-a/nested/package.json", r#"{"name":"nested"}"#),
            ("group/plain/README.md", "no project"),
            ("proj-b/pyproject.toml", "[project]\nname='b'\ndependencies=[]"),
        ]);
        let res = discover(&[base.to_string_lossy().to_string()]);
        let names: Vec<_> = res.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"proj-a"));
        assert!(names.contains(&"proj-b"));
        // nested project inside proj-a must NOT be listed separately
        assert!(!names.contains(&"nested"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dep_name_parsing() {
        assert_eq!(dep_name("fastapi>=0.110"), "fastapi");
        assert_eq!(dep_name("uvicorn[standard]"), "uvicorn");
        assert_eq!(dep_name("Django==5.0"), "django");
    }
}
