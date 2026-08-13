import { describe, expect, it } from "vitest";
import { fmtBytes, fmtCpu, fmtUptime, projectMatches, relTime, tildify } from "./utils";
import type { ProjectView } from "./types";

describe("formatting", () => {
  it("formats bytes", () => {
    expect(fmtBytes(null)).toBe("–");
    expect(fmtBytes(52428800)).toBe("50 MB");
    expect(fmtBytes(2 * 1024 * 1024 * 1024)).toBe("2.0 GB");
  });

  it("formats cpu", () => {
    expect(fmtCpu(null)).toBe("–");
    expect(fmtCpu(1.25)).toBe("1.3%");
  });

  it("formats uptime", () => {
    const now = 1_000_000_000_000;
    expect(fmtUptime(now - 30_000, now)).toBe("30s");
    expect(fmtUptime(now - 5 * 60_000, now)).toBe("5m");
    expect(fmtUptime(now - 2 * 3600_000 - 5 * 60_000, now)).toBe("2h 5m");
    expect(fmtUptime(now - 3 * 86400_000, now)).toBe("3d");
    expect(fmtUptime(null, now)).toBe("–");
  });

  it("relative time", () => {
    const now = 1_000_000_000_000;
    expect(relTime(now / 1000 - 10, now)).toBe("just now");
    expect(relTime(now / 1000 - 300, now)).toBe("5m ago");
    expect(relTime(now / 1000 - 7200, now)).toBe("2h ago");
  });

  it("tildify", () => {
    expect(tildify("/Users/me/Dev/x")).toBe("~/Dev/x");
    expect(tildify("/opt/homebrew")).toBe("/opt/homebrew");
  });
});

describe("search", () => {
  const project: ProjectView = {
    path: "/u/dev/todayskin",
    name: "todayskin",
    active: true,
    frameworks: ["Vite", "NestJS"],
    git: {
      branch: "feature/checkout",
      dirtyCount: 2,
      ahead: 0,
      behind: 0,
      lastCommit: null,
    },
    services: [
      {
        id: "s1",
        kind: "process",
        name: "web",
        framework: "Vite",
        frameworkKey: "vite",
        runtime: "node",
        pid: 123,
        ports: [5173],
        cmd: "node vite",
        cmdFull: "node /x/vite",
        cwd: "/u/dev/todayskin/apps/web",
        cpu: 1,
        memBytes: 1,
        startedAtMs: 1,
        health: { tcp: true, httpStatus: 200, level: "ok", detail: null },
        managed: false,
        managedId: null,
        containerId: null,
        projectPath: "/u/dev/todayskin",
        startCommand: "pnpm run dev",
        isHttp: true,
      },
    ],
    containers: [],
    ports: [5173],
    hasCompose: false,
    composeRunning: false,
  };

  it("matches name, port, branch, framework, service", () => {
    expect(projectMatches(project, "today")).toBe(true);
    expect(projectMatches(project, "5173")).toBe(true);
    expect(projectMatches(project, "checkout")).toBe(true);
    expect(projectMatches(project, "nest")).toBe(true);
    expect(projectMatches(project, "web")).toBe(true);
    expect(projectMatches(project, "zzz")).toBe(false);
  });
});
