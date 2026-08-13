# Dev Cockpit

An always-visible dashboard for your local dev environment, living in the macOS menu bar.

> Don't dig through terminals to find out what's running.
> One glance at the screen should tell you the state of your entire dev environment.

Dev Cockpit watches listening ports, dev processes, Docker containers and Git state, groups everything by project, and lets you control it all — start, stop, restart, logs, compose up/down — from a compact popover panel.

**Port Monitor + Process Monitor + Project Dashboard + Docker Dashboard + Git Status + Service Controller + Health Monitor + macOS Notifications** — in one lightweight menu bar app (~7 MB bundle, <1% CPU at idle).

*Read this in [Korean (한국어)](README.ko.md).*

## Features

- **Real-time runtime detection** — scans listening TCP ports (`lsof`) and process metrics (`sysinfo`), and identifies 40+ frameworks and services (Vite, Next.js, NestJS, FastAPI, Postgres, Redis, …) from command lines, working directories and Docker images.
- **Project-centric view** — discovers projects in your dev folders (Git repos and manifests), then groups running services, containers and ports into project cards with Git branch, dirty/ahead/behind counts and last commit.
- **One-click control** — start (auto-detected `dev` script or `docker compose up`), stop (SIGTERM, force-kill with ⌥), restart, open in terminal / editor / Finder / browser.
- **Docker integration** — container state, ports, CPU/memory, start/stop/restart, live logs, automatic Docker Compose project linking with one-click up/down.
- **Port conflict detection** — overlapping listeners on the same port are surfaced in a banner, with the owning process and project named.
- **Health monitoring** — TCP + HTTP probes distinguish "process alive" from "actually responding".
- **macOS notifications** — unexpected service/container exits, health failures and recoveries, new port conflicts, Docker daemon state — with per-event cooldowns and sleep/wake grace so it never spams.
- **Live logs** — stream logs from managed processes and containers; filter, pause, clear; bounded ring buffer.
- **Search** — one query across project names, ports, processes, frameworks, containers and branches.

## Requirements

- macOS 12+
- [Rust](https://rustup.rs) (stable), Node 20+ (with npm)
- Docker features activate automatically when a Docker daemon (Docker Desktop, OrbStack, …) is running — the app works fine without one

## Install

Quickest (Apple Silicon, requires Node 18+):

```bash
npx @min0504/dev-cockpit
```

This downloads the latest release from GitHub, verifies its SHA-256 checksum, installs `Dev Cockpit.app` into `/Applications` and launches it.

Manual: download `Dev Cockpit.app` from [Releases](https://github.com/Min0504/dev-cockpit/releases), move it to `/Applications` and launch it. The app is unsigned, so right-click → Open on first launch.

## Build & Run

```bash
npm install

# development (hot reload)
npm run tauri dev

# release bundle
npm run tauri build
# → src-tauri/target/release/bundle/macos/Dev Cockpit.app
```

Move the `.app` to `/Applications` and launch it — a gauge icon appears in the menu bar. Enable **Launch at Login** from the tray menu or Settings to keep it always available.

Since the app is not code-signed, macOS Gatekeeper may require right-click → Open on first launch.

## Usage at a Glance

| Action | How |
| --- | --- |
| Toggle panel | Click the menu bar icon, or `⌃⌥D` |
| Search | `/` or `⌘F` inside the panel |
| Back / close panel | `Esc` |
| Force-kill a service | `⌥`-click the stop button |
| Open an HTTP service | Click its port badge |
| Keep panel open | Pin button (disables hide-on-blur) |
| Pause monitoring | Tray menu → Pause Monitoring |

The tray icon shows the number of running services and containers; the tooltip breaks down services, containers, ports and conflicts.

Full guide: **[docs/USAGE.md](docs/USAGE.md)**

## Configuration

Everything is editable from the in-app Settings view and stored at:

```
~/Library/Application Support/com.min0504.devcockpit/config.json
```

- Project roots (defaults: `~/Dev`, `~/Developer`, `~/Projects`, `~/Code`, `~/repos`, `~/workspace` — whichever exist)
- Scan intervals (ports/processes 3 s, Docker 5 s, Git 20 s, discovery 10 min)
- Notification toggles per event + cooldown
- Hide/rename projects, override start commands
- Theme (system / light / dark), launch at login

## Documentation

| Doc | Contents |
| --- | --- |
| [docs/USAGE.md](docs/USAGE.md) | Installation, panel walkthrough, every setting explained ([한국어](docs/USAGE.ko.md)) |
| [docs/FEATURES.md](docs/FEATURES.md) | Detailed feature catalog and exact behaviors ([한국어](docs/FEATURES.ko.md)) |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Module map, data flow, IPC surface, design decisions ([한국어](docs/ARCHITECTURE.ko.md)) |

## Development

```bash
npm run typecheck     # tsc --noEmit
npm test              # vitest (frontend unit tests)
cargo test            # Rust unit tests (in src-tauri/)
cargo clippy          # Rust lints (in src-tauri/)
```

Stack: Tauri v2 (Rust backend) + React 19 / TypeScript (strict) + vanilla CSS. No runtime dependencies beyond the system `lsof`, `git` and `docker` CLIs — all invoked with timeouts and failure isolation.

## Known Limitations

- Processes owned by other users are displayed but cannot be stopped (macOS permissions).
- Start supports detected package-manager scripts (`dev`, `start:dev`, `serve`, `start`) and Docker Compose; arbitrary commands are set per project as an override.
- Docker features require the `docker` CLI on `PATH`.
- Unsigned build: Gatekeeper confirmation is needed on first launch, and release-build notifications ask for permission once.

## License

[MIT](LICENSE)
