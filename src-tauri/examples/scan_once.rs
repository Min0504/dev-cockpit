//! Runs one full scan pipeline against the real machine and prints the
//! resulting Snapshot as JSON. Used for end-to-end verification without the UI:
//!
//!     cargo run --example scan_once [--json]

use std::collections::HashMap;

use dev_cockpit_lib::scan::{docker, git, link, ports, procs, projects};

fn main() {
    let json = std::env::args().any(|a| a == "--json");
    let cfg = dev_cockpit_lib::config::load();

    let t0 = std::time::Instant::now();
    let listeners = match ports::scan() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("port scan failed: {e}");
            Vec::new()
        }
    };

    let pids: Vec<i32> = listeners.iter().map(|l| l.pid).collect();
    let mut scanner = procs::ProcScanner::new();
    // Two passes so CPU% has a measurement window.
    scanner.refresh(&pids);
    std::thread::sleep(std::time::Duration::from_millis(400));
    let proc_map = scanner.refresh(&pids);

    let docker_state = docker::probe();
    let discovery = projects::discover(&cfg.roots);

    let mut git_map = HashMap::new();
    for p in &discovery.projects {
        if let Some(info) = git::collect(&p.path) {
            git_map.insert(p.path.clone(), info);
        }
    }

    let snapshot = link::assemble(link::LinkInput {
        listeners: &listeners,
        procs: &proc_map,
        docker: &docker_state,
        projects: &discovery.projects,
        git: &git_map,
        config: &cfg,
        managed: &[],
        self_pid: std::process::id() as i32,
        errors: discovery.errors.clone(),
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot).expect("serialize"));
        return;
    }

    println!("scan took {:?}", t0.elapsed());
    println!(
        "docker: available={} running={}/{}",
        snapshot.docker.available, snapshot.docker.running, snapshot.docker.total
    );
    println!(
        "totals: {} services · {} ports · {} containers · {}/{} projects active",
        snapshot.totals.running_services,
        snapshot.totals.listening_ports,
        snapshot.totals.running_containers,
        snapshot.totals.active_projects,
        snapshot.totals.projects_total
    );
    for p in &snapshot.projects {
        let git = p
            .git
            .as_ref()
            .map(|g| format!(" [{}{}]", g.branch, if g.dirty_count > 0 { "*" } else { "" }))
            .unwrap_or_default();
        println!("\n▸ {}{}  ({})", p.name, git, p.path);
        for s in &p.services {
            println!(
                "    {} {}  ports={:?} pid={:?} fw={:?}",
                if s.kind == dev_cockpit_lib::models::ServiceKind::Startable { "▷" } else { "●" },
                s.name,
                s.ports,
                s.pid,
                s.framework
            );
        }
        for c in &p.containers {
            println!("    ◆ {} [{}] {}", c.name, c.state, c.image);
        }
    }
    if !snapshot.orphan_services.is_empty() {
        println!("\norphan services:");
        for s in &snapshot.orphan_services {
            println!("  ● {} ports={:?} pid={:?}", s.name, s.ports, s.pid);
        }
    }
    if !snapshot.other_listeners.is_empty() {
        println!("\nother listeners: {}", snapshot.other_listeners.len());
    }
    if !snapshot.conflicts.is_empty() {
        println!("\nport conflicts:");
        for c in &snapshot.conflicts {
            println!("  :{} × {}", c.port, c.entries.len());
        }
    }
    if !snapshot.errors.is_empty() {
        println!("\nerrors: {:?}", snapshot.errors);
    }
}
