# Architecture

How Dev Cockpit is put together: modules, data flow, IPC surface and the design decisions behind them.

*한국어판: [ARCHITECTURE.ko.md](ARCHITECTURE.ko.md)*

## Overview

```
┌─────────────────────────────  macOS  ─────────────────────────────┐
│  lsof        sysinfo       docker CLI      git CLI     filesystem │
└────┬────────────┬──────────────┬──────────────┬────────────┬─────┘
     │            │              │              │            │
┌────▼────────────▼──────────────▼──────────────▼────────────▼─────┐
│                     scan/  (collectors, pure functions)          │
│   ports.rs      procs.rs      docker.rs      git.rs   projects.rs│
│        └───────────┬─ detect.rs (classification) ─┬──────┘       │
│                    └──────── link.rs ─────────────┘              │
│                       (assemble Snapshot)                        │
├──────────────────────────────────────────────────────────────────┤
│ monitor.rs   scheduler · health merge · diff · tray · notifier   │
│ control.rs   start/stop/restart · compose · open                 │
│ logs.rs      ring-buffer log sessions · event streaming          │
│ notify.rs    snapshot diff → Notification Center (cooldowns)     │
│ state.rs     shared AppState (config, snapshot, managed procs)   │
│ commands.rs  #[tauri::command] surface (thin wrappers)           │
│ lib.rs       tray · panel window · global shortcut · plugins     │
└───────────────┬──────────────────────────────────────────────────┘
                │  Tauri IPC: invoke (commands) + emit (events)
┌───────────────▼──────────────────────────────────────────────────┐
│  React UI (src/)                                                 │
│  ipc.ts → hooks.ts → App.tsx → ProjectCard / ServiceRow /        │
│  ContainerRow / LogsView / SettingsView                          │
└──────────────────────────────────────────────────────────────────┘
```

Stack: **Tauri v2** (Rust backend, WKWebView shell) + **React 19 / TypeScript strict** + vanilla CSS. No web framework beyond React, no CSS framework, no ORM — the heaviest dependency is `sysinfo`.

## Backend Modules (`src-tauri/src/`)

| Module | Responsibility |
| --- | --- |
| `models.rs` | Every shared type (`Snapshot`, `Service`, `Container`, `ProjectView`, `AppConfig`, …), serialized to the UI with serde `camelCase`. Single source of truth for the data contract. |
| `config.rs` | Load/save `AppConfig` JSON under `~/Library/Application Support/<id>/`, default project roots, value clamping. |
| `scan/ports.rs` | One `lsof` pass in field mode → listeners with bind addresses; address-overlap logic for conflict detection (wildcard vs loopback vs specific, IPv4/IPv6). |
| `scan/procs.rs` | `sysinfo` metrics for listener PIDs plus their parent chains (CPU, RSS, cwd, cmdline, start time); ancestry helpers for project attribution. |
| `scan/detect.rs` | Classification heuristics: command line + cwd + image name → framework key, runtime, display name, HTTP-ness. Noise filters for system daemons. |
| `scan/projects.rs` | Root-folder BFS discovery (bounded depth, skip lists), manifest parsing, monorepo workspace expansion, package-manager + start-script resolution. Cached between discovery ticks. |
| `scan/docker.rs` | Daemon probe, `docker ps` (custom separator format), `docker stats`, compose label extraction, container actions. |
| `scan/git.rs` | `git status --porcelain=v2 --branch` + `git log -1` parsing → branch, dirty, ahead/behind, last commit. |
| `scan/health.rs` | Dependency-free TCP connect and minimal HTTP/1.1 `GET /` probes with strict timeouts, run on blocking threads. |
| `scan/link.rs` | The join: listeners × processes × projects × containers × git → `Snapshot`. Project attribution via cwd/ancestry, compose linking, conflict grouping, totals. |
| `monitor.rs` | The heartbeat. Tick loop with per-source cadences, sleep/wake resync, parallel health probes, tray title/tooltip updates, notifier feed, change-only snapshot emit. |
| `control.rs` | Start (login-shell spawn, own process group, log session), stop (SIGTERM/SIGKILL, group-aware, dev-services-only guard), restart, compose actions, open terminal/editor/Finder/URL (http(s)-only). |
| `logs.rs` | Log sessions: pump child stdout/stderr or `docker logs -f --tail 300` into a bounded ring buffer (2,000 lines × 4,000 chars), batch-emit `logs://<id>` events. |
| `notify.rs` | Snapshot differ → notification decisions: stopped services/containers, health fail (2-strike) / recovery, new conflicts, docker daemon transitions. Per-key cooldowns, startup + resync grace, user-stop suppression. Native notifications in release, `osascript` in dev. |
| `state.rs` | `AppState`: config, latest snapshot, managed children registry, log registry, suppression sets, scheduler wake handle. |
| `commands.rs` | ~20 `#[tauri::command]` wrappers — no logic of their own. |
| `lib.rs` | App wiring: tray icon + menu, panel positioning under the tray (monitor-clamped), hide-on-blur vs pin, `⌃⌥D` global shortcut, autostart, single-instance, plugin registration. |

## Scheduling

One tokio task drives everything (`monitor.rs`). Each source has its own cadence and only runs when due:

| Source | Default cadence |
| --- | --- |
| Ports + processes (+ health) | 3 s |
| Docker `ps` | 5 s |
| Docker `stats` | 15 s |
| Git | 20 s |
| Project discovery | 10 min |

- The loop sleeps in ~800 ms slices; a `Notify` handle lets commands (Scan Now, config change, service started) wake it immediately.
- **Sleep/wake**: if the loop detects it overslept far beyond its budget, the next cycle is a *resync* — data refreshes but disappearance alerts are suppressed, so closing the laptop never produces a wall of "service stopped" notifications.
- **Pause** (tray or status bar) parks the loop entirely.

## Data Flow

1. Collectors run (subset due per tick) and produce plain data.
2. `link.rs` assembles an immutable `Snapshot { projects, orphanServices, otherListeners, unlinkedContainers, conflicts, totals, errors, seq }`.
3. Health results are merged in; transitions tracked.
4. `notify.rs` diffs against the previous snapshot and fires notifications through cooldowns.
5. The snapshot is compared with the last emitted one — **only if different** is `snapshot` emitted to the UI and the tray title refreshed.
6. React receives it via one `useSnapshot()` hook; the whole UI is a pure function of (snapshot, config, view state).

Collector failures never propagate: each external command has a timeout, errors are collected into `snapshot.errors` (shown as a status-bar warning), and the rest of the data stays live.

## IPC Surface

Commands (invoked from `src/ipc.ts`):

| Group | Commands |
| --- | --- |
| Data | `get_snapshot`, `force_scan`, `rescan_projects` |
| Config | `get_config`, `set_config` |
| Services | `start_service`, `stop_service`, `restart_service` |
| Docker | `docker_action`, `compose_action` |
| Open | `open_path` (terminal/editor/finder), `open_url` (http/https only) |
| Logs | `open_log_session`, `get_log_lines`, `close_log_session` |
| App | `get_autostart`, `set_autostart`, `set_paused`, `is_paused`, `hide_panel`, `quit_app` |

Events (backend → UI): `snapshot`, `toast`, `paused`, `managed-exited`, `logs://<session>`, `logs-ended://<session>`.

## Frontend (`src/`)

| File | Role |
| --- | --- |
| `types.ts` | TypeScript mirror of `models.rs` (camelCase contract). |
| `ipc.ts` | Typed `invoke`/`listen` wrappers — the only file touching Tauri APIs. |
| `hooks.ts` | `useSnapshot`, `useConfig`, `useToasts`, `usePaused`, clock + theme hooks. |
| `utils.ts` | Formatting (bytes/CPU/uptime/relative time), search matching, framework colors + monograms. Unit-tested. |
| `App.tsx` | Shell: search, conflict banner, section composition, sheet routing (logs/settings), keyboard handling, toasts. |
| `components/` | `ProjectCard`, `ServiceRow`, `ContainerRow`, `LogsView`, `SettingsView`. |
| `styles.css` | Design tokens (CSS variables per theme) + all component styles. No CSS framework. |

UI principles: information density over decoration, hover-revealed actions, no animation beyond functional transitions, tabular numerals for metrics, system font stack, native-feeling popover with vibrancy.

## Safety Model

Local-only by design — but still deliberate about what it may touch:

- **Stop guard**: only PIDs that the current snapshot classifies as dev services can be signalled; system processes are refused.
- **Process groups**: managed children run in their own group; stop signals the group, so no orphaned watchers.
- **Command execution**: Start runs only detected package-manager scripts / compose, or an explicit user-set override — never arbitrary strings from scans.
- **URL guard**: `open_url` accepts `http(s)` only.
- **No network egress**: probes target `localhost` exclusively; nothing phones home. Logs stay in memory.

## Testing

- **Rust (25 tests)**: `lsof` field-mode parsing + conflict overlap, `docker ps` parsing + memory/port parsing, git porcelain parsing, discovery/workspace/package-manager resolution on temp dirs, link/attribution/conflict/hidden-project assembly, health probes against real local listeners.
- **TypeScript (vitest)**: formatting and search-matching utilities.
- Parsers are deliberately pure functions over strings so fixtures can cover edge cases without the real CLIs.

## Build

- `npm run tauri dev` — vite dev server + debug Rust build with hot reload.
- `npm run tauri build` — release bundle (`Dev Cockpit.app`, ~7 MB).
- Icons are generated by `scripts/render-icons.swift` (app squircle + menubar template image).
