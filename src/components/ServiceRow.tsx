import { useState } from "react";
import type { Service } from "../types";
import { fmtBytes, fmtCpu, fmtUptime, frameworkColor, monogram, tildify } from "../utils";
import {
  IconGlobe,
  IconLogs,
  IconPlay,
  IconRestart,
  IconStop,
  IconTerminal,
} from "../icons";

export interface ServiceCallbacks {
  onStop: (pid: number, force?: boolean) => void;
  onStart: (projectPath: string, serviceName: string) => void;
  onRestart: (svc: Service) => void;
  onOpenLogs: (kind: "docker" | "managed", id: string, title: string) => void;
  onOpenUrl: (url: string) => void;
  onOpenPath: (path: string, target: "terminal" | "editor" | "finder") => void;
  onEditCommand: (svc: Service) => void;
}

interface Props {
  svc: Service;
  now: number;
  cb: ServiceCallbacks;
  compact?: boolean;
}

function healthDot(svc: Service): { cls: string; pulse: boolean } {
  if (svc.health.detail === "starting") return { cls: "warn", pulse: true };
  const level = svc.health.level ?? "unknown";
  return { cls: level, pulse: false };
}

export function ServiceRow({ svc, now, cb, compact }: Props) {
  const [open, setOpen] = useState(false);

  if (svc.kind === "startable") {
    return (
      <div className="svc-row">
        <button
          className="icon-btn"
          title={`Start: ${svc.startCommand ?? ""}`}
          onClick={() => svc.projectPath && cb.onStart(svc.projectPath, svc.name)}
        >
          <IconPlay size={11} />
        </button>
        <span className="monogram" style={{ background: frameworkColor(svc.frameworkKey) }}>
          {monogram(svc.frameworkKey, svc.name)}
        </span>
        <span className="svc-name">{svc.name}</span>
        {svc.framework && <span className="svc-fw">{svc.framework}</span>}
        <span
          className="startcmd"
          title="Edit start command"
          style={{ cursor: "pointer" }}
          onClick={() => cb.onEditCommand(svc)}
        >
          {svc.startCommand}
        </span>
      </div>
    );
  }

  const dot = healthDot(svc);
  const port = svc.ports[0] ?? null;
  const httpUrl = port !== null ? `http://localhost:${port}` : null;
  const canRestart = Boolean(svc.startCommand);

  return (
    <>
      <div className="svc-row clickable" onClick={() => setOpen(!open)}>
        <span className={`dot ${dot.cls}${dot.pulse ? " pulse" : ""}`} />
        <span className="monogram" style={{ background: frameworkColor(svc.frameworkKey) }}>
          {monogram(svc.frameworkKey, svc.name)}
        </span>
        <span className="svc-name">{svc.name}</span>
        {!compact && svc.framework && svc.framework !== svc.name && (
          <span className="svc-fw">{svc.framework}</span>
        )}
        {port !== null && (
          <span
            className="port"
            title={svc.isHttp && httpUrl ? `Open ${httpUrl}` : svc.ports.join(", ")}
            style={svc.isHttp ? { cursor: "pointer" } : undefined}
            onClick={(e) => {
              if (svc.isHttp && httpUrl) {
                e.stopPropagation();
                cb.onOpenUrl(httpUrl);
              }
            }}
          >
            :{port}
            {svc.ports.length > 1 ? ` +${svc.ports.length - 1}` : ""}
          </span>
        )}
        {svc.health.level === "down" && <span className="health-chip down">DOWN</span>}
        {svc.health.level === "warn" && svc.health.httpStatus != null && (
          <span className="health-chip warn">HTTP {svc.health.httpStatus}</span>
        )}
        {svc.health.detail === "starting" && (
          <span className="health-chip warn">starting</span>
        )}
        <span className="metrics">
          {svc.cpu !== null && <span>{fmtCpu(svc.cpu)}</span>}
          {svc.memBytes !== null && <span>{fmtBytes(svc.memBytes)}</span>}
          <span>{fmtUptime(svc.startedAtMs, now)}</span>
        </span>
        <span className="row-actions" onClick={(e) => e.stopPropagation()}>
          {svc.managed && svc.managedId && (
            <button
              className="icon-btn"
              title="View logs"
              onClick={() => cb.onOpenLogs("managed", svc.managedId!, svc.name)}
            >
              <IconLogs />
            </button>
          )}
          {svc.isHttp && httpUrl && (
            <button className="icon-btn" title={`Open ${httpUrl}`} onClick={() => cb.onOpenUrl(httpUrl)}>
              <IconGlobe />
            </button>
          )}
          {svc.cwd && (
            <button
              className="icon-btn"
              title="Open in terminal"
              onClick={() => cb.onOpenPath(svc.cwd!, "terminal")}
            >
              <IconTerminal />
            </button>
          )}
          {canRestart && (
            <button className="icon-btn" title="Restart" onClick={() => cb.onRestart(svc)}>
              <IconRestart />
            </button>
          )}
          {svc.pid !== null && (
            <button
              className="icon-btn danger"
              title="Stop (SIGTERM, escalates to SIGKILL)"
              onClick={() => cb.onStop(svc.pid!)}
            >
              <IconStop size={11} />
            </button>
          )}
        </span>
      </div>
      {open && (
        <div className="svc-detail">
          {svc.cmdFull && (
            <div className="kv">
              <span className="k">cmd</span>
              <span className="v">{svc.cmdFull}</span>
            </div>
          )}
          {svc.cwd && (
            <div className="kv">
              <span className="k">cwd</span>
              <span className="v">{tildify(svc.cwd)}</span>
            </div>
          )}
          <div className="kv">
            <span className="k">pid</span>
            <span className="v">
              {svc.pid ?? "–"}
              {svc.runtime ? ` · ${svc.runtime}` : ""}
              {svc.managed ? " · managed by Dev Cockpit" : ""}
            </span>
          </div>
          {svc.ports.length > 0 && (
            <div className="kv">
              <span className="k">ports</span>
              <span className="v">{svc.ports.join(", ")}</span>
            </div>
          )}
          {(svc.health.detail || svc.health.httpStatus != null) && (
            <div className="kv">
              <span className="k">health</span>
              <span className="v">
                {svc.health.detail ?? (svc.health.tcp === true ? "tcp ok" : "")}
                {svc.health.httpStatus != null ? ` · HTTP ${svc.health.httpStatus}` : ""}
              </span>
            </div>
          )}
          <div className="detail-actions">
            {svc.startCommand && (
              <button className="text-btn" onClick={() => cb.onEditCommand(svc)}>
                Edit command
              </button>
            )}
            {svc.cwd && (
              <button className="text-btn" onClick={() => cb.onOpenPath(svc.cwd!, "editor")}>
                Open in editor
              </button>
            )}
            {svc.pid !== null && (
              <button
                className="text-btn"
                style={{ color: "var(--down)" }}
                onClick={() => cb.onStop(svc.pid!, true)}
              >
                Force kill
              </button>
            )}
          </div>
        </div>
      )}
    </>
  );
}
