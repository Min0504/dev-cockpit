//! Snapshot assembly: joins listeners, process info, Docker containers,
//! project analysis, git info and managed children into the single
//! `Snapshot` consumed by the UI.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::*;
use crate::scan::detect::{self, Category};
use crate::scan::docker::DockerState;
use crate::scan::ports::Listener;
use crate::scan::procs::ProcInfo;
use crate::scan::projects::ProjectStatic;
use crate::util::{ellipsize, now_ms};

/// Lightweight view of a child process Dev Cockpit spawned.
#[derive(Debug, Clone)]
pub struct ManagedView {
    pub id: String,
    pub pid: i32,
    pub pgid: i32,
    pub name: String,
    pub project_path: Option<String>,
    pub dir: String,
    pub command: String,
    pub started_ms: u64,
}

pub struct LinkInput<'a> {
    pub listeners: &'a [Listener],
    pub procs: &'a HashMap<i32, ProcInfo>,
    pub docker: &'a DockerState,
    pub projects: &'a [ProjectStatic],
    pub git: &'a HashMap<String, GitInfo>,
    pub config: &'a AppConfig,
    pub managed: &'a [ManagedView],
    pub self_pid: i32,
    pub errors: Vec<String>,
}

fn quantize_start(ms: u64) -> u64 {
    (ms / 5000) * 5000
}

fn round_cpu(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn round_mem(bytes: u64) -> u64 {
    const MB: u64 = 1024 * 1024;
    ((bytes + MB / 2) / MB) * MB
}

fn canon(p: &str) -> String {
    Path::new(p)
        .canonicalize()
        .map(|c| c.to_string_lossy().to_string())
        .unwrap_or_else(|_| p.trim_end_matches('/').to_string())
}

pub fn assemble(input: LinkInput<'_>) -> Snapshot {
    let now = now_ms();
    let cfg = input.config;

    // Project lookup sorted longest-path-first for prefix matching.
    let mut proj_paths: Vec<&ProjectStatic> = input.projects.iter().collect();
    proj_paths.sort_by_key(|p| std::cmp::Reverse(p.path.len()));
    let find_project = |dir: &Path| -> Option<&ProjectStatic> {
        let dir_str = dir.to_string_lossy();
        proj_paths
            .iter()
            .find(|p| {
                dir_str == p.path.as_str()
                    || dir_str.starts_with(&format!("{}/", p.path))
            })
            .copied()
    };

    let hidden: HashSet<&str> = cfg.hidden_projects.iter().map(String::as_str).collect();

    // pid -> listeners
    let mut by_pid: BTreeMap<i32, Vec<&Listener>> = BTreeMap::new();
    for l in input.listeners {
        if l.pid == input.self_pid {
            continue;
        }
        by_pid.entry(l.pid).or_default().push(l);
    }

    // Ports published by docker infra processes (Docker Desktop / OrbStack forwarders)
    let mut docker_infra_ports: HashSet<u16> = HashSet::new();
    let mut docker_infra_pids: HashSet<i32> = HashSet::new();
    for (pid, ls) in &by_pid {
        let name = input
            .procs
            .get(pid)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| ls[0].process.clone());
        let cmd = input.procs.get(pid).map(|p| p.cmd.clone()).unwrap_or_default();
        if let Some(det) = detect::from_process(&name, &cmd) {
            if det.key == "docker" {
                docker_infra_pids.insert(*pid);
                docker_infra_ports.extend(ls.iter().map(|l| l.port));
            }
        }
    }

    // Managed children lookup by process group.
    let managed_by_pgid: HashMap<i32, &ManagedView> =
        input.managed.iter().map(|m| (m.pgid, m)).collect();

    // ---------------- services from processes ----------------
    struct Built {
        service: Service,
        project: Option<String>, // project path
        noise: bool,
    }
    let mut built: Vec<Built> = Vec::new();

    for (pid, ls) in &by_pid {
        if docker_infra_pids.contains(pid) {
            continue;
        }
        let proc = input.procs.get(pid);
        if proc.map(|p| p.zombie).unwrap_or(false) {
            continue;
        }
        let name = proc.map(|p| p.name.clone()).unwrap_or_else(|| ls[0].process.clone());
        let cmd_full = proc.map(|p| p.cmd.clone()).unwrap_or_default();
        let cwd: Option<PathBuf> = proc.and_then(|p| p.cwd.clone());

        let mut ports: Vec<u16> = ls.iter().map(|l| l.port).collect();
        ports.sort_unstable();
        ports.dedup();

        let det = detect::from_process(&name, &cmd_full);
        let project = cwd.as_deref().and_then(find_project);
        let noise = det.is_none() && project.is_none() || detect::is_noise(&name);

        // Sub-package naming: a process whose cwd is a workspace package dir
        // gets that package's name ("web", "api"…).
        let mut display = det.as_ref().map(|d| d.label.to_string()).unwrap_or_else(|| name.clone());
        let mut fw_key = det.as_ref().map(|d| d.key.to_string());
        let mut fw_label = det.as_ref().map(|d| d.label.to_string());
        let mut start_command: Option<String> = None;
        let mut service_project: Option<String> = None;

        if let Some(p) = project {
            service_project = Some(p.path.clone());
            let cwd_str = cwd.as_ref().map(|c| c.to_string_lossy().to_string()).unwrap_or_default();
            if let Some(sub) = p.subpackages.iter().find(|s| s.dir == cwd_str) {
                display = sub.name.clone();
                if matches!(fw_key.as_deref(), None | Some("node") | Some("python") | Some("bun") | Some("deno") | Some("node-dev")) {
                    if let (Some(k), Some(l)) = (&sub.framework_key, &sub.framework_label) {
                        fw_key = Some(k.clone());
                        fw_label = Some(l.clone());
                    }
                }
            } else if cwd_str == p.path {
                // Root process of the project: prefer framework label; fall
                // back to the package name.
                if det.is_none() {
                    display = p.package_name.clone().unwrap_or_else(|| p.name.clone());
                }
            }
            // Start command for restart: matching startable spec by dir.
            start_command = p
                .startables
                .iter()
                .find(|s| s.dir == cwd_str)
                .map(|s| s.command.clone());
            // Config override wins.
            if let Some(ov) = cfg.project_overrides.get(&p.path) {
                for (svc_name, cmd) in &ov.commands {
                    let matches_name = svc_name == &display
                        || (svc_name == "." && cwd_str == p.path);
                    if matches_name && !cmd.trim().is_empty() {
                        start_command = Some(cmd.clone());
                    }
                }
            }
        }

        let is_http = det
            .as_ref()
            .map(|d| detect::is_http_category(d.category))
            .unwrap_or(service_project.is_some());

        // Managed merge (same process group as a child we spawned).
        let mut managed_flag = false;
        let mut managed_id = None;
        if !input.managed.is_empty() {
            let pgid = crate::scan::procs::pgid_of(*pid);
            if let Some(m) = managed_by_pgid.get(&pgid) {
                managed_flag = true;
                managed_id = Some(m.id.clone());
                if start_command.is_none() {
                    start_command = Some(m.command.clone());
                }
                if service_project.is_none() {
                    service_project = m.project_path.clone();
                }
            }
        }

        let id = match &service_project {
            Some(p) => format!("svc:{p}:{display}:{}", ports.first().copied().unwrap_or(0)),
            None => format!("proc:{name}:{}", ports.first().copied().unwrap_or(0)),
        };

        let service = Service {
            id,
            kind: ServiceKind::Process,
            name: display,
            framework: fw_label,
            framework_key: fw_key,
            runtime: detect::runtime_of(&name).map(String::from),
            pid: Some(*pid),
            ports,
            cmd: Some(ellipsize(&cmd_full, 96)).filter(|c| !c.is_empty()),
            cmd_full: Some(cmd_full.clone()).filter(|c| !c.is_empty()),
            cwd: cwd.as_ref().map(|c| c.to_string_lossy().to_string()),
            cpu: proc.map(|p| round_cpu(p.cpu)),
            mem_bytes: proc.map(|p| round_mem(p.mem_bytes)),
            started_at_ms: proc.map(|p| quantize_start(now.saturating_sub(p.run_time_sec * 1000))),
            health: Health::default(),
            managed: managed_flag,
            managed_id,
            container_id: None,
            project_path: service_project.clone(),
            start_command,
            is_http,
        };
        built.push(Built { service, project: service_project, noise });
    }

    // Deduplicate display names within a project (e.g. two plain `node`s).
    {
        let mut seen: HashMap<(Option<String>, String), u32> = HashMap::new();
        for b in built.iter_mut() {
            let key = (b.project.clone(), b.service.name.clone());
            let n = seen.entry(key).or_insert(0);
            *n += 1;
            if *n > 1 {
                if let Some(pid) = b.service.pid {
                    b.service.name = format!("{} #{pid}", b.service.name);
                    b.service.id = format!("{}#{pid}", b.service.id);
                }
            }
        }
    }

    // Managed children that haven't opened a port yet appear as "starting"
    // services so the user gets immediate feedback after pressing Start.
    {
        let matched: HashSet<String> = built
            .iter()
            .filter_map(|b| b.service.managed_id.clone())
            .collect();
        for m in input.managed {
            if matched.contains(&m.id) {
                continue;
            }
            if !crate::scan::procs::group_alive(m.pgid) {
                continue;
            }
            let project = Path::new(&m.dir)
                .canonicalize()
                .ok()
                .and_then(|d| find_project(&d).map(|p| p.path.clone()))
                .or_else(|| m.project_path.clone());
            built.push(Built {
                service: Service {
                    id: format!("managed:{}", m.id),
                    kind: ServiceKind::Process,
                    name: m.name.clone(),
                    framework: None,
                    framework_key: None,
                    runtime: None,
                    pid: Some(m.pid),
                    ports: Vec::new(),
                    cmd: Some(m.command.clone()),
                    cmd_full: Some(m.command.clone()),
                    cwd: Some(m.dir.clone()),
                    cpu: None,
                    mem_bytes: None,
                    started_at_ms: Some(quantize_start(m.started_ms)),
                    health: Health {
                        tcp: None,
                        http_status: None,
                        level: Some(HealthLevel::Unknown),
                        detail: Some("starting".into()),
                    },
                    managed: true,
                    managed_id: Some(m.id.clone()),
                    container_id: None,
                    project_path: project.clone(),
                    start_command: Some(m.command.clone()),
                    is_http: false,
                },
                project,
                noise: false,
            });
        }
    }

    // ---------------- containers ----------------
    let mut containers: Vec<Container> = input.docker.containers.clone();
    // canonical project path lookup by name / package name
    let by_dir_name: HashMap<String, &ProjectStatic> = input
        .projects
        .iter()
        .map(|p| (p.name.to_lowercase(), p))
        .collect();
    let by_pkg_name: HashMap<String, &ProjectStatic> = input
        .projects
        .iter()
        .filter_map(|p| p.package_name.as_ref().map(|n| (n.to_lowercase(), p)))
        .collect();

    for c in containers.iter_mut() {
        let mut linked: Option<String> = None;
        if let Some(dir) = &c.compose_dir {
            let cd = canon(dir);
            if let Some(p) = find_project(Path::new(&cd)) {
                linked = Some(p.path.clone());
            }
        }
        if linked.is_none() {
            if let Some(cp) = &c.compose_project {
                let key = cp.to_lowercase();
                if let Some(p) = by_dir_name.get(&key).or_else(|| by_pkg_name.get(&key)) {
                    linked = Some(p.path.clone());
                }
            }
        }
        c.project_path = linked;
    }

    // ---------------- project views ----------------
    let mut projects_out: Vec<ProjectView> = Vec::new();
    for ps in input.projects {
        if hidden.contains(ps.path.as_str()) {
            continue;
        }
        let ov = cfg.project_overrides.get(&ps.path);
        let display_name = ov
            .and_then(|o| o.name.clone())
            .unwrap_or_else(|| ps.name.clone());

        let mut services: Vec<Service> = built
            .iter()
            .filter(|b| b.project.as_deref() == Some(ps.path.as_str()) && !b.noise)
            .map(|b| b.service.clone())
            .collect();

        let linked_containers: Vec<Container> = containers
            .iter()
            .filter(|c| c.project_path.as_deref() == Some(ps.path.as_str()))
            .cloned()
            .collect();

        // Startables not superseded by a running process.
        let running_dirs: HashSet<String> =
            services.iter().filter_map(|s| s.cwd.clone()).collect();
        let any_node_running = services.iter().any(|s| {
            matches!(
                s.framework_key.as_deref(),
                Some("vite") | Some("nextjs") | Some("nestjs") | Some("cra") | Some("remix")
                    | Some("astro") | Some("nuxt") | Some("sveltekit") | Some("angular")
                    | Some("node") | Some("node-dev") | Some("webpack") | Some("expo")
            )
        });
        let mut startable_names: HashSet<String> = HashSet::new();
        for spec in &ps.startables {
            if running_dirs.contains(&spec.dir) {
                continue;
            }
            if spec.dir == ps.path && ps.startables.len() > 1 && any_node_running {
                // Root orchestrator script while parts already run — hide to
                // avoid double-starting everything.
                continue;
            }
            let mut command = spec.command.clone();
            if let Some(o) = ov {
                if let Some(c) = o.commands.get(&spec.name).filter(|c| !c.trim().is_empty()) {
                    command = c.clone();
                }
            }
            startable_names.insert(spec.name.clone());
            services.push(Service {
                id: format!("start:{}:{}", ps.path, spec.name),
                kind: ServiceKind::Startable,
                name: spec.name.clone(),
                framework: spec.framework_label.clone(),
                framework_key: spec.framework_key.clone(),
                runtime: None,
                pid: None,
                ports: Vec::new(),
                cmd: Some(command.clone()),
                cmd_full: None,
                cwd: Some(spec.dir.clone()),
                cpu: None,
                mem_bytes: None,
                started_at_ms: None,
                health: Health::default(),
                managed: false,
                managed_id: None,
                container_id: None,
                project_path: Some(ps.path.clone()),
                start_command: Some(command),
                is_http: spec.is_http,
            });
        }
        // Extra user-defined commands (override entries with unknown names).
        if let Some(o) = ov {
            for (svc_name, cmd) in &o.commands {
                if svc_name == "." || cmd.trim().is_empty() {
                    continue;
                }
                let exists = startable_names.contains(svc_name)
                    || services.iter().any(|s| &s.name == svc_name);
                if !exists {
                    services.push(Service {
                        id: format!("start:{}:{}", ps.path, svc_name),
                        kind: ServiceKind::Startable,
                        name: svc_name.clone(),
                        framework: None,
                        framework_key: None,
                        runtime: None,
                        pid: None,
                        ports: Vec::new(),
                        cmd: Some(cmd.clone()),
                        cmd_full: None,
                        cwd: Some(ps.path.clone()),
                        cpu: None,
                        mem_bytes: None,
                        started_at_ms: None,
                        health: Health::default(),
                        managed: false,
                        managed_id: None,
                        container_id: None,
                        project_path: Some(ps.path.clone()),
                        start_command: Some(cmd.clone()),
                        is_http: true,
                    });
                }
            }
        }

        // Sort: running first (by port), then startables by name.
        services.sort_by(|a, b| {
            let ka = a.kind != ServiceKind::Process;
            let kb = b.kind != ServiceKind::Process;
            ka.cmp(&kb)
                .then_with(|| a.first_port().unwrap_or(u16::MAX).cmp(&b.first_port().unwrap_or(u16::MAX)))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let mut ports: Vec<u16> = services.iter().flat_map(|s| s.ports.clone()).collect();
        ports.extend(linked_containers.iter().flat_map(|c| c.ports.iter().filter_map(|b| b.host)));
        ports.sort_unstable();
        ports.dedup();

        let active = services.iter().any(|s| s.kind == ServiceKind::Process)
            || linked_containers.iter().any(|c| c.state == "running");
        let compose_running = linked_containers
            .iter()
            .any(|c| c.state == "running" && c.compose_project.is_some());

        projects_out.push(ProjectView {
            path: ps.path.clone(),
            name: display_name,
            active,
            frameworks: ps.frameworks.iter().map(|(_, l)| l.clone()).collect(),
            git: input.git.get(&ps.path).cloned(),
            services,
            containers: linked_containers,
            ports,
            has_compose: ps.compose_file.is_some(),
            compose_running,
        });
    }
    projects_out.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    // ---------------- orphans & other ----------------
    let mut orphan_services: Vec<Service> = built
        .iter()
        .filter(|b| b.project.is_none() && !b.noise)
        .map(|b| b.service.clone())
        .collect();
    orphan_services.sort_by_key(|s| s.first_port().unwrap_or(u16::MAX));

    let mut other_listeners: Vec<Service> = built
        .iter()
        .filter(|b| b.noise)
        .map(|b| {
            let mut s = b.service.clone();
            s.cmd_full = None;
            s.is_http = false;
            s
        })
        .collect();
    other_listeners.sort_by_key(|s| s.first_port().unwrap_or(u16::MAX));

    let unlinked_containers: Vec<Container> = containers
        .iter()
        .filter(|c| c.project_path.is_none())
        .cloned()
        .collect();

    // ---------------- conflicts ----------------
    let project_name_of = |path: &Option<String>| -> Option<String> {
        path.as_ref().and_then(|p| {
            projects_out
                .iter()
                .find(|pv| &pv.path == p)
                .map(|pv| pv.name.clone())
        })
    };
    let mut port_owners: BTreeMap<u16, Vec<ConflictEntry>> = BTreeMap::new();
    for b in &built {
        for l in input.listeners.iter().filter(|l| Some(l.pid) == b.service.pid) {
            port_owners.entry(l.port).or_default().push(ConflictEntry {
                pid: l.pid,
                process: b.service.name.clone(),
                project: project_name_of(&b.service.project_path),
                addr: l.addr.clone(),
            });
        }
    }
    // container-published ports count as an owner too
    for c in containers.iter().filter(|c| c.state == "running") {
        for pb in &c.ports {
            if let Some(host) = pb.host {
                port_owners.entry(host).or_default().push(ConflictEntry {
                    pid: 0,
                    process: format!("container {}", c.name),
                    project: project_name_of(&c.project_path),
                    addr: format!("*:{host}"),
                });
            }
        }
    }
    let conflicts: Vec<PortConflict> = port_owners
        .into_iter()
        .filter_map(|(port, entries)| {
            let mut owners: Vec<ConflictEntry> = Vec::new();
            let mut seen_pid: HashSet<i32> = HashSet::new();
            for e in entries {
                if e.pid == 0 || seen_pid.insert(e.pid) {
                    owners.push(e);
                }
            }
            if owners.len() > 1 {
                Some(PortConflict { port, entries: owners })
            } else {
                None
            }
        })
        .collect();

    // ---------------- totals ----------------
    let running_services = built.iter().filter(|b| !b.noise).count() as u32;
    let running_containers = containers.iter().filter(|c| c.state == "running").count() as u32;
    let listening_ports = {
        let mut all: HashSet<u16> = input.listeners.iter().map(|l| l.port).collect();
        all.extend(docker_infra_ports.iter());
        all.len() as u32
    };

    Snapshot {
        seq: 0,
        generated_at_ms: now,
        scan_ms: 0,
        docker: DockerSummary {
            available: input.docker.available,
            reason: input.docker.reason.clone(),
            running: running_containers,
            total: containers.len() as u32,
        },
        totals: Totals {
            running_services,
            listening_ports,
            running_containers,
            active_projects: projects_out.iter().filter(|p| p.active).count() as u32,
            projects_total: projects_out.len() as u32,
        },
        projects: projects_out,
        orphan_services,
        other_listeners,
        unlinked_containers,
        conflicts,
        errors: input.errors,
    }
}

#[allow(unused)]
fn category_of(key: &str) -> Category {
    match key {
        "postgres" | "redis" | "mysql" | "mongodb" | "clickhouse" | "memcached" => Category::Database,
        "docker" | "nginx" | "caddy" | "ssh" => Category::Infra,
        _ => Category::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::projects::{StartableSpec, SubPackage};

    fn listener(pid: i32, process: &str, port: u16, addr: &str) -> Listener {
        Listener {
            pid,
            process: process.into(),
            addr: addr.into(),
            port,
            ipv6: addr.starts_with('['),
            loopback: addr.starts_with("127.") || addr.starts_with("[::1]"),
        }
    }

    fn proc_info(pid: i32, name: &str, cmd: &str, cwd: &str) -> ProcInfo {
        ProcInfo {
            pid,
            name: name.into(),
            cmd: cmd.into(),
            cwd: Some(PathBuf::from(cwd)),
            cpu: 1.234,
            mem_bytes: 100 * 1024 * 1024 + 12345,
            run_time_sec: 3600,
            zombie: false,
        }
    }

    fn base_project(path: &str, name: &str) -> ProjectStatic {
        ProjectStatic {
            path: path.into(),
            name: name.into(),
            package_name: Some(name.into()),
            frameworks: vec![("vite".into(), "Vite".into())],
            startables: vec![
                StartableSpec {
                    name: "web".into(),
                    dir: format!("{path}/apps/web"),
                    command: "pnpm run dev".into(),
                    framework_key: Some("vite".into()),
                    framework_label: Some("Vite".into()),
                    is_http: true,
                },
                StartableSpec {
                    name: "api".into(),
                    dir: format!("{path}/apps/api"),
                    command: "pnpm run start:dev".into(),
                    framework_key: Some("nestjs".into()),
                    framework_label: Some("NestJS".into()),
                    is_http: true,
                },
            ],
            compose_file: Some("docker-compose.yml".into()),
            compose_services: vec!["postgres".into()],
            subpackages: vec![
                SubPackage {
                    dir: format!("{path}/apps/web"),
                    name: "web".into(),
                    framework_key: Some("vite".into()),
                    framework_label: Some("Vite".into()),
                },
                SubPackage {
                    dir: format!("{path}/apps/api"),
                    name: "api".into(),
                    framework_key: Some("nestjs".into()),
                    framework_label: Some("NestJS".into()),
                },
            ],
            has_git: true,
        }
    }

    #[test]
    fn links_processes_to_projects_and_supersedes_startables() {
        let proj = base_project("/u/dev/todayskin", "todayskin");
        let listeners = vec![
            listener(100, "node", 5173, "*:5173"),
            listener(200, "rapportd", 49152, "*:49152"),
        ];
        let mut procs = HashMap::new();
        procs.insert(
            100,
            proc_info(100, "node", "node /u/dev/todayskin/apps/web/node_modules/.bin/vite", "/u/dev/todayskin/apps/web"),
        );
        procs.insert(200, proc_info(200, "rapportd", "/usr/libexec/rapportd", "/"));

        let snap = assemble(LinkInput {
            listeners: &listeners,
            procs: &procs,
            docker: &DockerState::default(),
            projects: &[proj],
            git: &HashMap::new(),
            config: &AppConfig::default(),
            managed: &[],
            self_pid: 1,
            errors: vec![],
        });

        assert_eq!(snap.projects.len(), 1);
        let p = &snap.projects[0];
        assert!(p.active);
        // running "web" (vite) + startable "api"
        let web = p.services.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(web.kind, ServiceKind::Process);
        assert_eq!(web.framework_key.as_deref(), Some("vite"));
        assert_eq!(web.ports, vec![5173]);
        assert_eq!(web.cpu, Some(1.2)); // rounded
        assert_eq!(web.mem_bytes.unwrap() % (1024 * 1024), 0); // MB-rounded
        let api = p.services.iter().find(|s| s.name == "api").unwrap();
        assert_eq!(api.kind, ServiceKind::Startable);
        assert_eq!(api.start_command.as_deref(), Some("pnpm run start:dev"));
        // no separate "web" startable — superseded by the running process
        assert_eq!(p.services.iter().filter(|s| s.name == "web").count(), 1);
        // noise listener went to other_listeners
        assert_eq!(snap.other_listeners.len(), 1);
        assert_eq!(snap.other_listeners[0].pid, Some(200));
        assert!(snap.conflicts.is_empty());
    }

    #[test]
    fn container_links_by_compose_dir_and_conflicts_detected() {
        let proj = base_project("/u/dev/acme", "acme");
        let mut docker = DockerState { available: true, reason: None, containers: vec![] };
        docker.containers.push(Container {
            id: "abc".into(),
            name: "acme-dev-postgres".into(),
            image: "postgres:16-alpine".into(),
            state: "running".into(),
            status_text: "Up 2 hours (healthy)".into(),
            health: Some("healthy".into()),
            ports: vec![PortBinding { host: Some(5432), container: 5432, proto: "tcp".into() }],
            compose_project: Some("acme".into()),
            compose_service: Some("postgres".into()),
            compose_dir: Some("/u/dev/acme".into()),
            cpu: None,
            mem_bytes: None,
            mem_limit_bytes: None,
            running_for: Some("2 hours".into()),
            project_path: None,
        });

        // local postgres also on 5432 -> conflict with container-published port
        let listeners = vec![listener(300, "postgres", 5432, "127.0.0.1:5432")];
        let mut procs = HashMap::new();
        procs.insert(300, proc_info(300, "postgres", "/opt/homebrew/bin/postgres", "/opt/homebrew"));

        let snap = assemble(LinkInput {
            listeners: &listeners,
            procs: &procs,
            docker: &docker,
            projects: &[proj],
            git: &HashMap::new(),
            config: &AppConfig::default(),
            managed: &[],
            self_pid: 1,
            errors: vec![],
        });

        let p = &snap.projects[0];
        assert_eq!(p.containers.len(), 1);
        assert_eq!(p.containers[0].name, "acme-dev-postgres");
        assert!(p.active);
        assert!(snap.unlinked_containers.is_empty());
        assert_eq!(snap.conflicts.len(), 1);
        assert_eq!(snap.conflicts[0].port, 5432);
        assert_eq!(snap.conflicts[0].entries.len(), 2);
        // orphan postgres (cwd outside projects)
        assert!(snap.orphan_services.iter().any(|s| s.framework_key.as_deref() == Some("postgres")));
    }

    #[test]
    fn hidden_projects_are_excluded() {
        let proj = base_project("/u/dev/secret", "secret");
        let cfg = AppConfig {
            hidden_projects: vec!["/u/dev/secret".into()],
            ..AppConfig::default()
        };
        let snap = assemble(LinkInput {
            listeners: &[],
            procs: &HashMap::new(),
            docker: &DockerState::default(),
            projects: &[proj],
            git: &HashMap::new(),
            config: &cfg,
            managed: &[],
            self_pid: 1,
            errors: vec![],
        });
        assert!(snap.projects.is_empty());
    }
}
