# Feature Catalog

Exact behaviors of every Dev Cockpit feature. For day-to-day operation see [USAGE.md](USAGE.md); for how it's built see [ARCHITECTURE.md](ARCHITECTURE.md).

*한국어판: [FEATURES.ko.md](FEATURES.ko.md)*

## 1. Runtime Detection

Every scan tick (default 3 s):

1. `lsof -nP -iTCP -sTCP:LISTEN` lists listening TCP sockets (field mode, one pass).
2. `sysinfo` resolves each owning PID to name, full command line, working directory, CPU %, RSS, start time and the parent chain.
3. A heuristic engine classifies each process from its command line, cwd and (for containers) image name.

Collected per service: **port(s) · PID · process name · command · project path · project name · framework · runtime · CPU · memory · uptime · status**.

### Recognized frameworks & services

Node ecosystem: Vite, Next.js, React (CRA / react-scripts), Nest.js, Express, Fastify, Koa, Hono, Nuxt, Svelte/SvelteKit, Vue, Angular, Astro, Electron, Tauri, Node, Bun, Deno.

Python: FastAPI, uvicorn, Django, Flask, Streamlit, generic Python.

Databases & infra: PostgreSQL (`postmaster`, pgvector), Redis (`redis-server`, Valkey), MySQL/MariaDB, MongoDB, ClickHouse, OpenSearch, Meilisearch, memcached, MinIO, RabbitMQ, Kafka (`cp-kafka`), LocalStack, nginx.

Other runtimes: Go, Java, .NET, PHP, Ruby.

Anything else with a listening port is still shown — either as a generic dev process (when it lives under a project root) or under **Other listeners** (hidden by default).

## 2. Project Discovery

- Scans configured root folders (defaults: `~/Dev`, `~/Developer`, `~/Projects`, `~/Code`, `~/repos`, `~/workspace`) with a bounded-depth BFS that skips `node_modules`, `.git` internals, build caches, `Library`, etc.
- A directory becomes a project when it contains `.git` or a manifest: `package.json`, `pyproject.toml`, `requirements.txt`, `docker-compose.yml` / `docker-compose.yaml` / `compose.yml` / `compose.yaml`, `go.mod`, `Cargo.toml`, `Gemfile`, `mix.exs`.
- **Monorepos** — `pnpm-workspace.yaml` package globs and `package.json` `workspaces` are expanded; each sub-package contributes its own startable service (e.g. `web`, `api`).
- **Framework inference from manifests** — dependencies in `package.json` / `pyproject.toml` / `requirements.txt` tag the project (Vite, Next, Nest, FastAPI, …) even when nothing is running.
- Re-discovery runs every 10 minutes and on demand (tray → Rescan Now, or Settings → rescan).

## 3. Project-centric View

The link stage joins all data sources into one snapshot:

- Processes attach to projects by working-directory containment, walking up the parent-process chain (a `node` child of `pnpm dev` inherits the project).
- Containers attach via Docker Compose labels (`com.docker.compose.project.working_dir`) matched against project paths.
- Each project card shows: Git branch, dirty count, ahead/behind, last commit (summary · author · age), running services, containers, listening ports, compose state.
- Projects with nothing running are grouped under **Idle projects**; hidden projects (Settings) are excluded everywhere.

## 4. One-click Control

| Action | Behavior |
| --- | --- |
| Start | Spawns the detected start command via the user's login shell, in its own process group, with output captured to a log session. Command resolution: lockfile → package manager (`pnpm-lock.yaml`→pnpm, `yarn.lock`→yarn, `bun.lock(b)`→bun, else npm), script preference `dev` → `start:dev` → `serve` → `start`. Compose projects use `docker compose up -d`. |
| Stop | SIGTERM; `⌥`-click sends SIGKILL. If the target leads its own process group the whole group is signalled (kills `npm run dev` → `node` trees cleanly). Refuses to signal processes that are not detected dev services. |
| Restart | Stop, wait for exit, re-start with the known command; failures surface as toasts. |
| Open | Terminal (configurable app, default Terminal), editor (default Cursor), Finder, browser (`http://localhost:<port>`). URLs are restricted to http/https. |
| Override | Per-project start-command overrides and display names, editable inline or in Settings, persisted in config. |

User-initiated stops are recorded and suppressed from "service stopped" notifications.

## 5. Docker Integration

- Daemon probe first — when Docker is down, all Docker work is skipped (no error spam) and a note is shown; state changes (down↔up) are notified.
- `docker ps` every 5 s (configurable): name, image, state, health, port bindings, compose project/service/working-dir labels.
- `docker stats` on a slower cadence (15 s) fills CPU / memory / limit.
- Actions: start / stop / restart per container; `docker logs -f` live tail (last 300 lines + follow); compose up/down per project with streamed output.
- Compose containers are grouped into their project card automatically.

## 6. Port Management & Conflicts

- Every listening TCP port is tracked with its bind address (`*:5173`, `127.0.0.1:8080`, `[::1]:6379`).
- **Conflict detection** understands address semantics: two listeners collide when their binds overlap (wildcard `*` overlaps everything on that port; two different loopback addresses do not). IPv4/IPv6 dual binds of one server are not false positives.
- Conflicts render as a red banner naming each process, PID, project and exact bind, with per-entry stop buttons — and fire a notification when they first appear.

## 7. Health Monitoring

Levels beyond "process exists":

- **TCP probe** — connect to the port (IPv4 then IPv6) with a short timeout.
- **HTTP probe** — for services that speak HTTP: `GET /` and parse the status line. Any 2xx–4xx response counts as healthy (4xx still means "responding"); a 5xx response marks the service degraded.
- Container health states (`healthy` / `starting` / `unhealthy`) come from Docker.
- Resulting levels: `ok` · `warn` (TCP open but HTTP 5xx / container starting) · `down` (port not accepting connections) · `unknown`.
- A health failure notifies only after **two consecutive** failing probes of a previously-healthy service; recovery notifies once (both toggleable).

## 8. macOS Notifications

| Event | Fired when |
| --- | --- |
| Service stopped | A tracked service with a port disappears without you stopping it |
| Container stopped | A running container leaves `running` unexpectedly |
| Health check failed | Two consecutive failed probes of a previously healthy service |
| Recovered | A previously alerted-down service becomes healthy |
| Port conflict | A new overlapping-listener conflict appears |
| Docker unreachable / back | Daemon state transitions |

Safeguards: per-event-key cooldown (default 60 s, clamp 10 s–1 h), startup grace (first two ticks), sleep/wake resync grace, suppression of user-initiated stops. Release builds use the native Notification Center; dev builds fall back to `osascript`.

## 9. Logs

- Sources: processes started by Dev Cockpit (stdout+stderr) and `docker logs -f` tails.
- Ring buffer: 2,000 lines per session, 4,000 chars per line — memory-bounded by construction.
- Lines are batched to the UI over Tauri events; the view supports filter, pause (with catch-up on resume), clear, copy, stderr highlighting and auto-follow.
- Closing the view tears the session down; container tails are killed. Nothing is written to disk.

## 10. Search

One query box filters the whole snapshot — project names, paths, ports, PIDs, process names, commands, frameworks, runtimes, container names, images, compose services and Git branches. Matching is case-insensitive substring; sections collapse when empty.

## 11. Always-visible UX

- **Tray icon** with live count of running services + containers; tooltip with services/containers/ports/conflicts breakdown.
- **Popover panel** (460×640) anchored under the tray icon, clamped to the tray's monitor, above normal windows, hidden from the Dock and app switcher.
- Hide-on-blur by default; **pin** keeps it open. `⌃⌥D` toggles from anywhere (global shortcut).
- **Single instance** — relaunching focuses the existing instance instead of duplicating it.
- Dark / light theme following the system (or forced).
- UI re-renders only when a scan actually changed something.

## 12. Reliability & Performance

- Every external command (`lsof`, `docker`, `git`, spawned dev servers) runs with a timeout; one failing collector degrades gracefully instead of crashing the app (errors surface in the status bar).
- Sleep/wake detection: a stalled scheduler triggers a resync cycle that never produces false "stopped" alerts.
- Interval separation (ports 3 s / docker 5 s / stats 15 s / git 20 s / discovery 10 m) keeps steady-state CPU under ~1 %.
- Snapshot diffing means zero UI work when nothing changed; log buffers and per-session caps bound memory.
