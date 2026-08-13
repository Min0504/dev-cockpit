import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ipc } from "./ipc";
import {
  useClock,
  useConfig,
  useManagedExitToast,
  usePausedState,
  useSnapshot,
  useToasts,
} from "./hooks";
import type { AppConfig, Service, Snapshot } from "./types";
import { containerMatches, projectMatches, serviceMatches } from "./utils";
import { ProjectCard, type ProjectCallbacks } from "./components/ProjectCard";
import { ServiceRow } from "./components/ServiceRow";
import { ContainerRow } from "./components/ContainerRow";
import { LogsView } from "./components/LogsView";
import { SettingsView } from "./components/SettingsView";
import {
  IconChevron,
  IconGear,
  IconPause,
  IconPin,
  IconPlay,
  IconRefresh,
  IconSearch,
  IconWarn,
  IconX,
} from "./icons";

type SheetState =
  | { kind: "settings" }
  | { kind: "logs"; session: string; title: string }
  | null;

export default function App() {
  const { snapshot, ready } = useSnapshot();
  const { config, save } = useConfig();
  const { toasts, push } = useToasts();
  const [paused, setPaused] = usePausedState();
  const now = useClock(15_000);
  const [query, setQuery] = useState("");
  const [sheet, setSheet] = useState<SheetState>(null);
  const [idleOpen, setIdleOpen] = useState(false);
  const [otherOpen, setOtherOpen] = useState(false);
  const searchRef = useRef<HTMLInputElement | null>(null);

  useManagedExitToast(push);
  useTheme(config);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (sheet) setSheet(null);
        else if (query) setQuery("");
        else ipc.hidePanel().catch(() => {});
      } else if ((e.metaKey && e.key === "f") || (e.key === "/" && !isTyping(e))) {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [sheet, query]);

  const openLogs = useCallback(
    (kind: "docker" | "managed" | "session", id: string, title: string) => {
      ipc
        .openLogSession(kind, id, title)
        .then((session) => setSheet({ kind: "logs", session, title }))
        .catch((e) => push(String(e)));
    },
    [push],
  );

  const cb: ProjectCallbacks = useMemo(
    () => ({
      onStop: (pid, force) => {
        ipc.stopService(pid, force ?? false).catch((e) => push(String(e)));
      },
      onStart: (projectPath, serviceName) => {
        ipc
          .startService(projectPath, serviceName)
          .then((session) => setSheet({ kind: "logs", session, title: serviceName }))
          .catch((e) => push(String(e)));
      },
      onRestart: (svc: Service) => {
        ipc
          .restartService(svc.pid, svc.projectPath, svc.name)
          .then(() => push(`Restarting ${svc.name}…`))
          .catch((e) => push(String(e)));
      },
      onOpenLogs: openLogs,
      onOpenUrl: (url) => ipc.openUrl(url).catch((e) => push(String(e))),
      onOpenPath: (path, target) => ipc.openPath(path, target).catch((e) => push(String(e))),
      onEditCommand: () => {}, // replaced inside ProjectCard with inline editor
      onAction: (id, action) => ipc.dockerAction(id, action).catch((e) => push(String(e))),
      onCompose: (projectPath, action) => {
        ipc
          .composeAction(projectPath, action)
          .then((session) =>
            setSheet({ kind: "logs", session, title: `docker compose ${action}` }),
          )
          .catch((e) => push(String(e)));
      },
      onRename: (path, name) => {
        if (!config) return;
        const overrides = { ...config.projectOverrides };
        const cur = overrides[path] ?? { name: null, commands: {}, healthUrl: null };
        overrides[path] = { ...cur, name };
        save({ ...config, projectOverrides: overrides }).catch((e) => push(String(e)));
      },
      onHide: (path) => {
        if (!config) return;
        save({ ...config, hiddenProjects: [...config.hiddenProjects, path] }).catch((e) =>
          push(String(e)),
        );
        push("Project hidden — unhide from Settings");
      },
      onSaveCommand: (path, serviceName, command) => {
        if (!config) return;
        const overrides = { ...config.projectOverrides };
        const cur = overrides[path] ?? { name: null, commands: {}, healthUrl: null };
        overrides[path] = { ...cur, commands: { ...cur.commands, [serviceName]: command } };
        save({ ...config, projectOverrides: overrides }).catch((e) => push(String(e)));
      },
    }),
    [config, save, push, openLogs],
  );

  const q = query.trim().toLowerCase();
  const view = useMemo(() => selectView(snapshot, q, config), [snapshot, q, config]);

  const togglePin = () => {
    if (!config) return;
    save({ ...config, pinned: !config.pinned }).catch(() => {});
  };

  return (
    <div className="panel">
      <header className="header drag-region" data-tauri-drag-region>
        <span className="brand" data-tauri-drag-region>
          <span className={`brand-dot${paused ? " paused" : ""}`} />
          Dev Cockpit
        </span>
        <span className="spacer" data-tauri-drag-region />
        <span className="search">
          <IconSearch size={11} />
          <input
            ref={searchRef}
            placeholder="Search projects, ports…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          {query && (
            <button
              className="icon-btn"
              style={{ width: 16, height: 16 }}
              onClick={() => setQuery("")}
            >
              <IconX size={9} />
            </button>
          )}
        </span>
        <button
          className={`icon-btn${paused ? " active" : ""}`}
          title={paused ? "Resume monitoring" : "Pause monitoring"}
          onClick={() => setPaused(!paused)}
        >
          {paused ? <IconPlay size={12} /> : <IconPause size={12} />}
        </button>
        <button
          className={`icon-btn${config?.pinned ? " active" : ""}`}
          title={config?.pinned ? "Unpin (hide on focus loss)" : "Pin panel open"}
          onClick={togglePin}
        >
          <IconPin />
        </button>
        <button
          className="icon-btn"
          title="Settings"
          onClick={() => setSheet({ kind: "settings" })}
        >
          <IconGear />
        </button>
      </header>

      <main className="main">
        {snapshot.conflicts.length > 0 && (
          <div className="banner">
            {snapshot.conflicts.map((c) => (
              <div key={c.port}>
                <div className="banner-title">
                  <IconWarn size={13} />
                  Port :{c.port} claimed by {c.entries.length} processes
                </div>
                {c.entries.map((e, i) => (
                  <div key={i} className="entry">
                    <span className="mono">{e.process}</span>
                    {e.pid > 0 ? ` (pid ${e.pid})` : ""}
                    {e.project ? ` — ${e.project}` : ""} · {e.addr}
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}

        {!ready && <div className="empty">Scanning your dev environment…</div>}

        {ready &&
          view.activeProjects.length === 0 &&
          view.idleProjects.length === 0 &&
          view.orphans.length === 0 &&
          view.unlinked.length === 0 && (
            <div className="empty">
              <span className="big">◎</span>
              {q ? (
                <>No matches for "{query}"</>
              ) : (
                <>
                  Nothing running right now.
                  <br />
                  Projects with a start command will appear here — press ▶ to launch them.
                </>
              )}
            </div>
          )}

        {view.activeProjects.length > 0 && (
          <>
            <div className="section-label">Active</div>
            {view.activeProjects.map((p) => (
              <ProjectCard key={p.path} project={p} now={now} cb={cb} />
            ))}
          </>
        )}

        {view.orphans.length > 0 && (
          <>
            <div className="section-label">
              Services without a project
              <span className="count">{view.orphans.length}</span>
            </div>
            <div className="card">
              <div className="rows" style={{ borderTop: "none" }}>
                {view.orphans.map((s) => (
                  <ServiceRow key={s.id} svc={s} now={now} cb={cb} />
                ))}
              </div>
            </div>
          </>
        )}

        {view.unlinked.length > 0 && (
          <>
            <div className="section-label">
              Containers
              <span className="count">{view.unlinked.length}</span>
            </div>
            <div className="card">
              <div className="rows" style={{ borderTop: "none" }}>
                {view.unlinked.map((c) => (
                  <ContainerRow key={c.id} c={c} cb={cb} />
                ))}
              </div>
            </div>
          </>
        )}

        {view.idleProjects.length > 0 && (
          <>
            <div className="section-label">
              <button onClick={() => setIdleOpen(!idleOpen)}>
                <IconChevron size={10} open={idleOpen || q.length > 0} />
                Idle projects
                <span className="count">{view.idleProjects.length}</span>
              </button>
            </div>
            {(idleOpen || q.length > 0) &&
              view.idleProjects.map((p) => (
                <ProjectCard key={p.path} project={p} now={now} cb={cb} />
              ))}
          </>
        )}

        {view.others.length > 0 && (
          <>
            <div className="section-label">
              <button onClick={() => setOtherOpen(!otherOpen)}>
                <IconChevron size={10} open={otherOpen || q.length > 0} />
                Other listeners
                <span className="count">{view.others.length}</span>
              </button>
            </div>
            {(otherOpen || q.length > 0) && (
              <div className="card idle">
                <div className="rows" style={{ borderTop: "none" }}>
                  {view.others.map((s) => (
                    <OtherRow key={s.id} svc={s} />
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </main>

      <footer className="footer">
        <span className="foot-item">
          <span className="dot ok" style={{ width: 5, height: 5 }} />
          {snapshot.totals.runningServices} services
        </span>
        <span className="foot-item">
          {snapshot.docker.available
            ? `${snapshot.totals.runningContainers} containers`
            : "docker off"}
        </span>
        <span className="foot-item">{snapshot.totals.listeningPorts} ports</span>
        {paused && (
          <span className="foot-item" style={{ color: "var(--warn)" }}>
            paused
          </span>
        )}
        {snapshot.errors.length > 0 && (
          <span className="err" title={snapshot.errors.join("\n")}>
            ⚠ {snapshot.errors[0]}
          </span>
        )}
        <span style={{ flex: 1 }} />
        <button
          className="icon-btn"
          style={{ width: 20, height: 20 }}
          title="Rescan now"
          onClick={() => ipc.forceScan().catch(() => {})}
        >
          <IconRefresh size={11} />
        </button>
      </footer>

      {sheet?.kind === "settings" && config && (
        <SettingsView config={config} save={save} onClose={() => setSheet(null)} push={push} />
      )}
      {sheet?.kind === "logs" && (
        <LogsView session={sheet.session} title={sheet.title} onClose={() => setSheet(null)} />
      )}

      {toasts.length > 0 && (
        <div className="toasts">
          {toasts.map((t) => (
            <div key={t.id} className="toast">
              {t.text}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function isTyping(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
}

function selectView(snapshot: Snapshot, q: string, config: AppConfig | null) {
  let projects = snapshot.projects;
  let orphans = snapshot.orphanServices;
  let unlinked = snapshot.unlinkedContainers;
  let others = config?.showOtherListeners === false ? [] : snapshot.otherListeners;

  if (q) {
    projects = projects.filter((p) => projectMatches(p, q));
    orphans = orphans.filter((s) => serviceMatches(s, q));
    unlinked = unlinked.filter((c) => containerMatches(c, q));
    others = others.filter((s) => serviceMatches(s, q));
  }

  const showIdle = config?.showIdleProjects !== false;
  const activeProjects = projects.filter((p) => p.active);
  const idleProjects = showIdle ? projects.filter((p) => !p.active) : [];
  return { activeProjects, idleProjects, orphans, unlinked, others };
}

function OtherRow({ svc }: { svc: Service }) {
  return (
    <div className="svc-row">
      <span className="dot unknown" />
      <span className="svc-name" style={{ color: "var(--text-2)" }}>
        {svc.name}
      </span>
      <span className="svc-fw">pid {svc.pid ?? "–"}</span>
      <span className="port muted">{svc.ports.map((p) => `:${p}`).join(" ")}</span>
    </div>
  );
}

function useTheme(config: AppConfig | null) {
  useEffect(() => {
    const apply = () => {
      const pref = config?.theme ?? "system";
      const dark =
        pref === "dark" ||
        (pref === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [config?.theme]);
}
