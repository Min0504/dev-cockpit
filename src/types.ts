// Mirrors src-tauri/src/models.rs (serde camelCase).

export type ServiceKind = "process" | "container" | "startable";
export type HealthLevel = "ok" | "warn" | "down" | "unknown";

export interface Health {
  tcp: boolean | null;
  httpStatus: number | null;
  level: HealthLevel | null;
  detail: string | null;
}

export interface Service {
  id: string;
  kind: ServiceKind;
  name: string;
  framework: string | null;
  frameworkKey: string | null;
  runtime: string | null;
  pid: number | null;
  ports: number[];
  cmd: string | null;
  cmdFull: string | null;
  cwd: string | null;
  cpu: number | null;
  memBytes: number | null;
  startedAtMs: number | null;
  health: Health;
  managed: boolean;
  managedId: string | null;
  containerId: string | null;
  projectPath: string | null;
  startCommand: string | null;
  isHttp: boolean;
}

export interface PortBinding {
  host: number | null;
  container: number;
  proto: string;
}

export interface Container {
  id: string;
  name: string;
  image: string;
  state: string;
  statusText: string;
  health: string | null;
  ports: PortBinding[];
  composeProject: string | null;
  composeService: string | null;
  composeDir: string | null;
  cpu: number | null;
  memBytes: number | null;
  memLimitBytes: number | null;
  runningFor: string | null;
  projectPath: string | null;
}

export interface CommitInfo {
  hash: string;
  summary: string;
  epochSec: number;
  author: string;
}

export interface GitInfo {
  branch: string;
  dirtyCount: number;
  ahead: number;
  behind: number;
  lastCommit: CommitInfo | null;
}

export interface ProjectView {
  path: string;
  name: string;
  active: boolean;
  frameworks: string[];
  git: GitInfo | null;
  services: Service[];
  containers: Container[];
  ports: number[];
  hasCompose: boolean;
  composeRunning: boolean;
}

export interface ConflictEntry {
  pid: number;
  process: string;
  project: string | null;
  addr: string;
}

export interface PortConflict {
  port: number;
  entries: ConflictEntry[];
}

export interface DockerSummary {
  available: boolean;
  reason: string | null;
  running: number;
  total: number;
}

export interface Totals {
  runningServices: number;
  listeningPorts: number;
  runningContainers: number;
  activeProjects: number;
  projectsTotal: number;
}

export interface Snapshot {
  seq: number;
  generatedAtMs: number;
  scanMs: number;
  docker: DockerSummary;
  projects: ProjectView[];
  orphanServices: Service[];
  otherListeners: Service[];
  unlinkedContainers: Container[];
  conflicts: PortConflict[];
  totals: Totals;
  errors: string[];
}

export interface NotifyConfig {
  enabled: boolean;
  serviceStopped: boolean;
  containerStopped: boolean;
  healthFailed: boolean;
  portConflict: boolean;
  recovered: boolean;
  cooldownSec: number;
}

export interface ProjectOverride {
  name: string | null;
  commands: Record<string, string>;
  healthUrl: string | null;
}

export interface AppConfig {
  roots: string[];
  pollIntervalMs: number;
  dockerIntervalMs: number;
  dockerStatsIntervalMs: number;
  gitIntervalMs: number;
  discoveryIntervalMs: number;
  httpHealth: boolean;
  notifications: NotifyConfig;
  theme: string;
  editorApp: string;
  terminalApp: string;
  showOtherListeners: boolean;
  showIdleProjects: boolean;
  hiddenProjects: string[];
  projectOverrides: Record<string, ProjectOverride>;
  pinned: boolean;
}

export interface LogLine {
  seq: number;
  atMs: number;
  line: string;
  stderr: boolean;
}

export interface LogSessionInfo {
  sessionId: string;
  title: string;
  source: string;
}

export interface LogsPayload {
  info: LogSessionInfo;
  lines: LogLine[];
  ended: boolean;
}

export function emptySnapshot(): Snapshot {
  return {
    seq: 0,
    generatedAtMs: 0,
    scanMs: 0,
    docker: { available: false, reason: null, running: 0, total: 0 },
    projects: [],
    orphanServices: [],
    otherListeners: [],
    unlinkedContainers: [],
    conflicts: [],
    totals: {
      runningServices: 0,
      listeningPorts: 0,
      runningContainers: 0,
      activeProjects: 0,
      projectsTotal: 0,
    },
    errors: [],
  };
}
