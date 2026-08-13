# Usage Guide

Everything you need to install, run and operate Dev Cockpit day to day.

*한국어판: [USAGE.ko.md](USAGE.ko.md)*

## Installation

### Via npm (Apple Silicon)

```bash
npx @minseokchae/dev-cockpit
```

Downloads the latest release, verifies its SHA-256 checksum, installs `Dev Cockpit.app` into `/Applications` (or `~/Applications` if not writable) and launches it. Options: `--dir <path>` to choose the install directory, `--no-open` to skip launching.

### Build from source

```bash
git clone https://github.com/Min0504/dev-cockpit.git
cd dev-cockpit
npm install
npm run tauri build
```

The bundle is produced at:

```
src-tauri/target/release/bundle/macos/Dev Cockpit.app
```

Move it to `/Applications` and launch it.

### First launch

- **Gatekeeper** — the app is not code-signed. If macOS blocks it, right-click the app → Open → Open.
- **Notifications** — the release build asks for notification permission once. Allow it if you want alerts for crashed servers, health failures and port conflicts.
- **Launch at Login** — enable it from the tray menu or Settings so the cockpit is always available.

After launch, a gauge icon appears in the menu bar. When services are running, the icon shows a count (running services + containers). Hover for a tooltip with the full breakdown.

## The Panel

Click the tray icon (or press `⌃⌥D` anywhere) to open the panel. It anchors under the tray icon and hides when it loses focus — unless pinned.

From top to bottom:

1. **Search field** — filters everything live.
2. **Conflict banner** — appears only when two processes listen on overlapping addresses of the same port. Names each process, PID and project; each entry has a stop button.
3. **Project cards** — one per discovered project that has something running (or that you expand). Header: project name, Git branch, dirty count, ahead/behind arrows, last commit. Rows inside: running services, containers, then startable-but-idle services.
4. **Other dev processes** — dev-looking processes that could not be attributed to any project.
5. **Docker** — containers not linked to any project, plus daemon status.
6. **Idle projects** — collapsed group of discovered projects with nothing running.
7. **Status bar** — last scan time and duration, totals, pause and rescan buttons.

### Keyboard

| Key | Action |
| --- | --- |
| `⌃⌥D` (global) | Toggle the panel from anywhere |
| `/` or `⌘F` | Focus search |
| `Esc` | Close sheet → clear search → hide panel (in that order) |

## Services

Each service row shows: status dot, framework badge, name, port badge, CPU, memory, uptime. Hover a row for actions.

- **Status dot** — green: running/healthy · yellow: degraded (TCP open but HTTP failing, container health `starting`) · red: down/unhealthy · hollow: not running.
- **Port badge** — click to open `http://localhost:<port>` when the service speaks HTTP; the tooltip lists all ports.
- **Stop** — sends SIGTERM. `⌥`-click to SIGKILL. When the process leads its own process group (e.g. `npm run dev` trees), the whole group is stopped so no orphans linger. Stopping is only allowed on detected dev services — system processes are protected.
- **Start** — appears on idle services with a detected start command. The command is resolved from the project's package manager (`pnpm` / `yarn` / `bun` / `npm`, by lockfile) and its scripts, preferring `dev` → `start:dev` → `serve` → `start`. Compose projects start with `docker compose up -d`. Starting opens a live log view.
- **Edit start command** — hover an idle service to override what Start runs for that service. Overrides are saved per project.
- **Restart** — stop + start, available when the start command is known (managed services always know theirs).
- **Logs** — services started from Dev Cockpit stream their stdout/stderr live.

## Projects

Project cards group everything that belongs to one repository or app directory.

- **Header actions** (on hover): compose up/down, open in browser, open in terminal, open in editor, reveal in Finder, hide project.
- **Git info** — branch, uncommitted-change count, ahead/behind vs upstream, last commit summary and age. Refreshed every 20 s (configurable).
- **Discovery** — projects are found by scanning your configured root folders for Git repositories and project manifests (`package.json`, `pyproject.toml`, `docker-compose.yml` / `compose.yaml`, `go.mod`, `Cargo.toml`, `Gemfile`, `mix.exs`, …). Monorepo workspaces (`pnpm-workspace.yaml`, `package.json` `workspaces`) are analyzed so each sub-package gets its own startable service.
- **Attribution** — a running process is linked to a project by its working directory (including parent processes), so `pnpm dev` started in any subfolder still lands on the right card.

## Docker

Containers linked to a project (via compose working directory or labels) appear inside the project card; the rest are listed in the Docker section.

- Rows show container/compose-service name, image, host ports, CPU, memory and uptime.
- Actions: logs (live `docker logs -f` tail), restart, stop / start.
- Compose projects get a single **compose up / compose down** button on the project card; output streams into a log view.
- If the Docker daemon is not running, Docker features simply deactivate — a subtle note is shown and everything else keeps working.

## Logs View

Opened from a service or container's log button, or automatically when you start something.

- **Live stream** with a bounded buffer (last 2,000 lines are kept; long lines truncated at 4,000 chars).
- **Filter** box narrows visible lines; the match count is shown.
- **Pause** freezes the view while buffering incoming lines; resume flushes them.
- **Clear** empties the view.
- **Follow** — the view sticks to the bottom; scroll up to detach, click Follow to re-attach.
- Leaving the view closes the log session (container tails are terminated; nothing keeps running in the background).

## Notifications

Sent for: service stopped unexpectedly · container stopped · health check failed (two consecutive failures) · recovered · new port conflict · Docker daemon lost/back.

- Per-event cooldown (default 60 s, configurable 10 s–1 h) prevents repeats.
- The first scans after app start and after sleep/wake are grace periods — no false "stopped" alerts.
- Stopping something yourself never notifies.
- Every event type can be toggled individually in Settings.

## Settings

Open with the gear button in the panel header.

| Setting | Meaning | Default |
| --- | --- | --- |
| Theme | System / Light / Dark | System |
| Launch at Login | macOS login item | off |
| Keep window open | Disable hide-on-blur (pin) | off |
| Scan interval | Ports + processes | 3 s (2/3/5/10) |
| Docker interval | `docker ps` cadence | 5 s |
| HTTP health checks | Probe local HTTP ports | on |
| Show idle projects | List projects with nothing running | on |
| Show other listeners | Non-dev processes with open ports | off |
| Notifications | Master + per-event toggles + cooldown | on, 60 s |
| Project roots | Folders scanned for projects | `~/Dev`, `~/Developer`, `~/Projects`, `~/Code`, `~/repos`, `~/workspace` (existing ones) |
| Projects | Rename, hide/unhide, per-project overrides | — |

### Config file

All settings live in a single JSON file you can edit or back up:

```
~/Library/Application Support/com.minseokchae.devcockpit/config.json
```

The app watches its own writes only; if you edit the file by hand, restart the app.

## Tray Menu

Right-click (or click) the tray icon:

- **Open Dev Cockpit** (`⌃⌥D`)
- **Rescan Now** — immediate full scan + project re-discovery
- **Pause Monitoring** — stops all scanning (checkbox)
- **Launch at Login** (checkbox)
- **Quit**

## Uninstall

1. Quit Dev Cockpit (tray menu → Quit).
2. Delete `Dev Cockpit.app`.
3. Optionally remove the config: `~/Library/Application Support/com.minseokchae.devcockpit/`.

No other files are written anywhere on the system.
