import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppConfig, LogLine, LogsPayload, Snapshot } from "./types";

export const ipc = {
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),
  forceScan: () => invoke<void>("force_scan"),
  rescanProjects: () => invoke<void>("rescan_projects"),
  getConfig: () => invoke<AppConfig>("get_config"),
  setConfig: (config: AppConfig) => invoke<AppConfig>("set_config", { config }),
  stopService: (pid: number, force: boolean) =>
    invoke<void>("stop_service", { pid, force }),
  startService: (projectPath: string, serviceName: string) =>
    invoke<string>("start_service", { projectPath, serviceName }),
  restartService: (pid: number | null, projectPath: string | null, serviceName: string) =>
    invoke<void>("restart_service", { pid, projectPath, serviceName }),
  dockerAction: (id: string, action: string) =>
    invoke<void>("docker_action", { id, action }),
  composeAction: (projectPath: string, action: string) =>
    invoke<string>("compose_action", { projectPath, action }),
  openPath: (path: string, target: string) =>
    invoke<void>("open_path", { path, target }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openLogSession: (kind: string, id: string, title: string) =>
    invoke<string>("open_log_session", { kind, id, title }),
  getLogLines: (session: string) => invoke<LogsPayload>("get_log_lines", { session }),
  closeLogSession: (session: string) => invoke<void>("close_log_session", { session }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  isPaused: () => invoke<boolean>("is_paused"),
  hidePanel: () => invoke<void>("hide_panel"),
  quitApp: () => invoke<void>("quit_app"),
};

export function onSnapshot(cb: (s: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>("snapshot", (e) => cb(e.payload));
}

export function onToast(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("toast", (e) => cb(e.payload));
}

export function onPaused(cb: (paused: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("paused", (e) => cb(e.payload));
}

export function onLogBatch(
  session: string,
  cb: (lines: LogLine[]) => void,
): Promise<UnlistenFn> {
  return listen<LogLine[]>(`logs://${session}`, (e) => cb(e.payload));
}

export function onLogEnded(session: string, cb: () => void): Promise<UnlistenFn> {
  return listen<void>(`logs-ended://${session}`, () => cb());
}

export function onManagedExited(cb: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>("managed-exited", (e) => cb(e.payload));
}
