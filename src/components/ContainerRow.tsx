import { useState } from "react";
import type { Container } from "../types";
import { fmtBytes, fmtCpu } from "../utils";
import { IconLogs, IconPlay, IconRestart, IconStop } from "../icons";

export interface ContainerCallbacks {
  onAction: (id: string, action: "start" | "stop" | "restart") => void;
  onOpenLogs: (kind: "docker" | "managed", id: string, title: string) => void;
  onOpenUrl: (url: string) => void;
}

interface Props {
  c: Container;
  cb: ContainerCallbacks;
}

function containerDot(c: Container): { cls: string; pulse: boolean } {
  if (c.state !== "running") return { cls: "unknown", pulse: false };
  if (c.health === "unhealthy") return { cls: "down", pulse: false };
  if (c.health === "starting") return { cls: "warn", pulse: true };
  return { cls: "ok", pulse: false };
}

export function ContainerRow({ c, cb }: Props) {
  const [open, setOpen] = useState(false);
  const dot = containerDot(c);
  const running = c.state === "running";
  const primary = c.ports.find((p) => p.host !== null) ?? null;

  return (
    <>
      <div className="svc-row clickable" onClick={() => setOpen(!open)}>
        <span className={`dot ${dot.cls}${dot.pulse ? " pulse" : ""}`} />
        <span className="monogram" style={{ background: "#2496ed" }}>
          {(c.composeService ?? c.name).slice(0, 2).replace(/^./, (ch) => ch.toUpperCase())}
        </span>
        <span className="svc-name">{c.composeService ?? c.name}</span>
        <span className="svc-fw">{c.image.split("@")[0]}</span>
        {primary && primary.host !== null && (
          <span
            className="port"
            title={`Open http://localhost:${primary.host}`}
            style={{ cursor: "pointer" }}
            onClick={(e) => {
              e.stopPropagation();
              cb.onOpenUrl(`http://localhost:${primary.host}`);
            }}
          >
            :{primary.host}
            {primary.container !== primary.host && (
              <span className="mapping">→{primary.container}</span>
            )}
          </span>
        )}
        {c.health === "unhealthy" && <span className="health-chip down">UNHEALTHY</span>}
        {!running && <span className="health-chip warn">{c.state}</span>}
        <span className="metrics">
          {c.cpu !== null && <span>{fmtCpu(c.cpu)}</span>}
          {c.memBytes !== null && <span>{fmtBytes(c.memBytes)}</span>}
        </span>
        <span className="row-actions" onClick={(e) => e.stopPropagation()}>
          <button
            className="icon-btn"
            title="Container logs"
            onClick={() => cb.onOpenLogs("docker", c.id, c.name)}
          >
            <IconLogs />
          </button>
          {running ? (
            <>
              <button
                className="icon-btn"
                title="Restart container"
                onClick={() => cb.onAction(c.id, "restart")}
              >
                <IconRestart />
              </button>
              <button
                className="icon-btn danger"
                title="Stop container"
                onClick={() => cb.onAction(c.id, "stop")}
              >
                <IconStop size={11} />
              </button>
            </>
          ) : (
            <button
              className="icon-btn"
              title="Start container"
              onClick={() => cb.onAction(c.id, "start")}
            >
              <IconPlay size={11} />
            </button>
          )}
        </span>
      </div>
      {open && (
        <div className="svc-detail">
          <div className="kv">
            <span className="k">name</span>
            <span className="v">{c.name}</span>
          </div>
          <div className="kv">
            <span className="k">image</span>
            <span className="v">{c.image}</span>
          </div>
          <div className="kv">
            <span className="k">status</span>
            <span className="v">{c.statusText}</span>
          </div>
          {c.ports.length > 0 && (
            <div className="kv">
              <span className="k">ports</span>
              <span className="v">
                {c.ports
                  .map((p) =>
                    p.host !== null
                      ? `${p.host}→${p.container}/${p.proto}`
                      : `${p.container}/${p.proto}`,
                  )
                  .join(", ")}
              </span>
            </div>
          )}
          {c.composeProject && (
            <div className="kv">
              <span className="k">compose</span>
              <span className="v">
                {c.composeProject}
                {c.composeService ? ` / ${c.composeService}` : ""}
              </span>
            </div>
          )}
          <div className="kv">
            <span className="k">id</span>
            <span className="v">{c.id.slice(0, 12)}</span>
          </div>
        </div>
      )}
    </>
  );
}
