import { useEffect, useRef, useState } from "react";
import type { ProjectView, Service } from "../types";
import { relTime, tildify } from "../utils";
import { ContainerRow, type ContainerCallbacks } from "./ContainerRow";
import { ServiceRow, type ServiceCallbacks } from "./ServiceRow";
import { IconBranch, IconCode, IconFolder, IconMore, IconTerminal } from "../icons";

export interface ProjectCallbacks extends ServiceCallbacks, ContainerCallbacks {
  onCompose: (projectPath: string, action: "up" | "down") => void;
  onRename: (path: string, name: string | null) => void;
  onHide: (path: string) => void;
  onSaveCommand: (path: string, serviceName: string, command: string) => void;
}

interface Props {
  project: ProjectView;
  now: number;
  cb: ProjectCallbacks;
}

export function ProjectCard({ project: p, now, cb }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [editingCmd, setEditingCmd] = useState<Service | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const close = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [menuOpen]);

  const git = p.git;
  const cbWithEdit: ServiceCallbacks = { ...cb, onEditCommand: (svc) => setEditingCmd(svc) };

  return (
    <div className={`card${p.active ? "" : " idle"}`} style={{ position: "relative" }}>
      <div className="card-head">
        <span className="proj-name" title={tildify(p.path)}>
          {p.name}
        </span>
        <span className="chips">
          {p.frameworks.slice(0, 3).map((f) => (
            <span key={f} className="chip">
              {f}
            </span>
          ))}
        </span>
        <span className="head-actions">
          {p.hasCompose && (
            <button
              className="text-btn"
              title={p.composeRunning ? "docker compose down" : "docker compose up -d"}
              onClick={() => cb.onCompose(p.path, p.composeRunning ? "down" : "up")}
            >
              {p.composeRunning ? "compose ↓" : "compose ↑"}
            </button>
          )}
          <button
            className="icon-btn"
            title="Open in editor"
            onClick={() => cb.onOpenPath(p.path, "editor")}
          >
            <IconCode />
          </button>
          <button
            className="icon-btn"
            title="Open in terminal"
            onClick={() => cb.onOpenPath(p.path, "terminal")}
          >
            <IconTerminal />
          </button>
          <button
            className="icon-btn"
            title="Reveal in Finder"
            onClick={() => cb.onOpenPath(p.path, "finder")}
          >
            <IconFolder />
          </button>
          <button className="icon-btn" title="More" onClick={() => setMenuOpen(!menuOpen)}>
            <IconMore />
          </button>
        </span>
      </div>

      {menuOpen && (
        <div className="menu" ref={menuRef}>
          <button
            onClick={() => {
              setMenuOpen(false);
              setRenaming(true);
            }}
          >
            Rename…
          </button>
          <button
            onClick={() => {
              setMenuOpen(false);
              cb.onRename(p.path, null);
            }}
          >
            Reset name
          </button>
          <button
            className="danger"
            onClick={() => {
              setMenuOpen(false);
              cb.onHide(p.path);
            }}
          >
            Hide project
          </button>
        </div>
      )}

      {git && (
        <div className="gitline">
          <span className="branch" title={git.branch}>
            <IconBranch size={10} />
            {git.branch}
          </span>
          {git.dirtyCount > 0 && (
            <span className="dirty" title={`${git.dirtyCount} changed files`}>
              ±{git.dirtyCount}
            </span>
          )}
          {git.ahead > 0 && <span title="commits ahead of upstream">↑{git.ahead}</span>}
          {git.behind > 0 && <span title="commits behind upstream">↓{git.behind}</span>}
          {git.lastCommit && (
            <span
              className="commit"
              title={`${git.lastCommit.summary} — ${git.lastCommit.author}`}
            >
              {git.lastCommit.summary} · {relTime(git.lastCommit.epochSec, now)}
            </span>
          )}
        </div>
      )}

      {renaming && (
        <InlineEdit
          initial={p.name}
          placeholder="Display name"
          onCancel={() => setRenaming(false)}
          onSave={(v) => {
            setRenaming(false);
            cb.onRename(p.path, v.trim() || null);
          }}
        />
      )}

      {editingCmd && (
        <InlineEdit
          initial={editingCmd.startCommand ?? ""}
          placeholder="Start command (runs in service directory)"
          onCancel={() => setEditingCmd(null)}
          onSave={(v) => {
            const svc = editingCmd;
            setEditingCmd(null);
            if (v.trim()) cb.onSaveCommand(p.path, svc.name, v.trim());
          }}
        />
      )}

      {(p.services.length > 0 || p.containers.length > 0) && (
        <div className="rows">
          {p.services.map((s) => (
            <ServiceRow key={s.id} svc={s} now={now} cb={cbWithEdit} />
          ))}
          {p.containers.map((c) => (
            <ContainerRow key={c.id} c={c} cb={cb} />
          ))}
        </div>
      )}
    </div>
  );
}

function InlineEdit({
  initial,
  placeholder,
  onSave,
  onCancel,
}: {
  initial: string;
  placeholder: string;
  onSave: (v: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initial);
  return (
    <div className="inline-edit">
      <input
        autoFocus
        value={value}
        placeholder={placeholder}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onSave(value);
          if (e.key === "Escape") {
            e.stopPropagation();
            onCancel();
          }
        }}
      />
      <button className="text-btn primary" onClick={() => onSave(value)}>
        Save
      </button>
      <button className="text-btn" onClick={onCancel}>
        Cancel
      </button>
    </div>
  );
}
