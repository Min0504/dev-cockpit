import type { ProjectView, Service, Container } from "./types";

export function fmtBytes(bytes: number | null): string {
  if (bytes == null || bytes <= 0) return "–";
  const mb = bytes / (1024 * 1024);
  if (mb < 1000) return `${mb.toFixed(0)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

export function fmtCpu(cpu: number | null): string {
  if (cpu == null) return "–";
  if (cpu < 0.05) return "0%";
  return cpu < 10 ? `${cpu.toFixed(1)}%` : `${Math.round(cpu)}%`;
}

export function fmtUptime(startedAtMs: number | null, nowMs: number): string {
  if (!startedAtMs || startedAtMs <= 0) return "–";
  let s = Math.max(0, Math.floor((nowMs - startedAtMs) / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return m > 0 ? `${h}h ${m}m` : `${h}h`;
  }
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  return h > 0 ? `${d}d ${h}h` : `${d}d`;
}

export function relTime(epochSec: number, nowMs: number): string {
  const diff = Math.max(0, Math.floor(nowMs / 1000) - epochSec);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
  return `${Math.floor(diff / (86400 * 30))}mo ago`;
}

function textOf(parts: Array<string | number | null | undefined>): string {
  return parts
    .filter((p): p is string | number => p != null && p !== "")
    .join(" ")
    .toLowerCase();
}

export function serviceMatches(s: Service, q: string): boolean {
  return textOf([
    s.name,
    s.framework,
    s.frameworkKey,
    s.runtime,
    s.cmd,
    ...s.ports,
    s.pid,
  ]).includes(q);
}

export function containerMatches(c: Container, q: string): boolean {
  return textOf([
    c.name,
    c.image,
    c.state,
    c.composeProject,
    c.composeService,
    ...c.ports.map((p) => p.host),
  ]).includes(q);
}

export function projectMatches(p: ProjectView, q: string): boolean {
  if (
    textOf([p.name, p.path, ...p.frameworks, p.git?.branch]).includes(q)
  ) {
    return true;
  }
  return (
    p.services.some((s) => serviceMatches(s, q)) ||
    p.containers.some((c) => containerMatches(c, q))
  );
}

const FW_COLORS: Record<string, string> = {
  react: "#61dafb",
  vite: "#a463ff",
  next: "#9ca3af",
  nest: "#ea2845",
  node: "#8cc84b",
  express: "#8cc84b",
  fastapi: "#009688",
  django: "#0c4b33",
  flask: "#9ca3af",
  python: "#3776ab",
  postgres: "#336791",
  mysql: "#00758f",
  redis: "#dc382d",
  mongo: "#47a248",
  docker: "#2496ed",
  compose: "#2496ed",
  vue: "#42b883",
  nuxt: "#00dc82",
  svelte: "#ff3e00",
  astro: "#e5427c",
  remix: "#9ca3af",
  angular: "#dd0031",
  rust: "#d0a215",
  go: "#00add8",
  ruby: "#cc342d",
  rails: "#cc342d",
  php: "#777bb4",
  laravel: "#ff2d20",
  java: "#e76f00",
  spring: "#6db33f",
  storybook: "#ff4785",
  tauri: "#ffc131",
  electron: "#47848f",
  bun: "#f9f1e1",
  deno: "#9ca3af",
};

export function frameworkColor(key: string | null): string {
  if (!key) return "#9ca3af";
  return FW_COLORS[key] ?? "#9ca3af";
}

const MONOGRAMS: Record<string, string> = {
  react: "R",
  vite: "V",
  next: "N",
  nest: "Ns",
  node: "Nd",
  express: "Ex",
  fastapi: "F",
  django: "Dj",
  flask: "Fl",
  python: "Py",
  postgres: "Pg",
  mysql: "My",
  redis: "Rd",
  mongo: "M",
  docker: "D",
  compose: "DC",
  vue: "Vu",
  nuxt: "Nx",
  svelte: "Sv",
  astro: "A",
  remix: "Rx",
  angular: "Ng",
  rust: "Rs",
  go: "Go",
  ruby: "Rb",
  rails: "RR",
  php: "P",
  laravel: "L",
  java: "J",
  spring: "Sp",
  storybook: "Sb",
  tauri: "T",
  electron: "El",
  bun: "B",
  deno: "De",
};

export function monogram(key: string | null, name: string): string {
  if (key && MONOGRAMS[key]) return MONOGRAMS[key];
  const c = name.trim().charAt(0).toUpperCase();
  return c || "?";
}

export function basename(path: string): string {
  const parts = path.replace(/\/+$/, "").split("/");
  return parts[parts.length - 1] || path;
}

export function tildify(path: string): string {
  const m = path.match(/^\/Users\/[^/]+/);
  if (m) return `~${path.slice(m[0].length)}`;
  return path;
}
