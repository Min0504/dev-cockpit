import { useEffect, useState } from "react";
import { ipc } from "../ipc";
import type { AppConfig } from "../types";
import { basename, tildify } from "../utils";
import { IconX } from "../icons";

interface Props {
  config: AppConfig;
  save: (next: AppConfig) => Promise<void>;
  onClose: () => void;
  push: (msg: string) => void;
}

export function SettingsView({ config, save, onClose, push }: Props) {
  const [autostart, setAutostart] = useState(false);
  const [newRoot, setNewRoot] = useState("");

  useEffect(() => {
    ipc.getAutostart().then(setAutostart).catch(() => {});
  }, []);

  const update = (patch: Partial<AppConfig>) => {
    save({ ...config, ...patch }).catch((e) => push(String(e)));
  };
  const updateNotify = (patch: Partial<AppConfig["notifications"]>) => {
    update({ notifications: { ...config.notifications, ...patch } });
  };

  const toggleAutostart = async () => {
    try {
      const next = await ipc.setAutostart(!autostart);
      setAutostart(next);
    } catch (e) {
      push(`Launch at login failed: ${e}`);
    }
  };

  const addRoot = () => {
    const v = newRoot.trim();
    if (!v) return;
    if (!v.startsWith("/") && !v.startsWith("~")) {
      push("Use an absolute path (or ~/…)");
      return;
    }
    if (config.roots.includes(v)) return;
    update({ roots: [...config.roots, v] });
    setNewRoot("");
    ipc.rescanProjects().catch(() => {});
  };

  return (
    <div className="sheet">
      <div className="sheet-head">
        <span className="sheet-title">Settings</span>
        <span style={{ flex: 1 }} />
        <button className="icon-btn" title="Close" onClick={onClose}>
          <IconX />
        </button>
      </div>
      <div className="sheet-body">
        <div className="set-group">
          <h3>General</h3>
          <div className="set-row">
            <label>Theme</label>
            <select value={config.theme} onChange={(e) => update({ theme: e.target.value })}>
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>
          <div className="set-row">
            <label>Launch at login</label>
            <input type="checkbox" checked={autostart} onChange={toggleAutostart} />
          </div>
          <div className="set-row">
            <label>
              Keep panel open <span className="hint">(don't hide on focus loss)</span>
            </label>
            <input
              type="checkbox"
              checked={config.pinned}
              onChange={(e) => update({ pinned: e.target.checked })}
            />
          </div>
          <div className="set-row">
            <label>Editor app</label>
            <input
              type="text"
              defaultValue={config.editorApp}
              onBlur={(e) => update({ editorApp: e.target.value.trim() || "Cursor" })}
            />
          </div>
          <div className="set-row">
            <label>Terminal app</label>
            <input
              type="text"
              defaultValue={config.terminalApp}
              onBlur={(e) => update({ terminalApp: e.target.value.trim() || "Terminal" })}
            />
          </div>
        </div>

        <div className="set-group">
          <h3>Monitoring</h3>
          <div className="set-row">
            <label>Scan interval</label>
            <select
              value={String(config.pollIntervalMs)}
              onChange={(e) => update({ pollIntervalMs: Number(e.target.value) })}
            >
              <option value="2000">2s</option>
              <option value="3000">3s</option>
              <option value="5000">5s</option>
              <option value="10000">10s</option>
            </select>
          </div>
          <div className="set-row">
            <label>Docker refresh</label>
            <select
              value={String(config.dockerIntervalMs)}
              onChange={(e) => update({ dockerIntervalMs: Number(e.target.value) })}
            >
              <option value="5000">5s</option>
              <option value="10000">10s</option>
              <option value="30000">30s</option>
            </select>
          </div>
          <div className="set-row">
            <label>Git refresh (active projects)</label>
            <select
              value={String(config.gitIntervalMs)}
              onChange={(e) => update({ gitIntervalMs: Number(e.target.value) })}
            >
              <option value="10000">10s</option>
              <option value="20000">20s</option>
              <option value="60000">1m</option>
            </select>
          </div>
          <div className="set-row">
            <label>
              HTTP health checks <span className="hint">(GET / on dev servers)</span>
            </label>
            <input
              type="checkbox"
              checked={config.httpHealth}
              onChange={(e) => update({ httpHealth: e.target.checked })}
            />
          </div>
          <div className="set-row">
            <label>Show non-dev listeners</label>
            <input
              type="checkbox"
              checked={config.showOtherListeners}
              onChange={(e) => update({ showOtherListeners: e.target.checked })}
            />
          </div>
          <div className="set-row">
            <label>Show idle projects</label>
            <input
              type="checkbox"
              checked={config.showIdleProjects}
              onChange={(e) => update({ showIdleProjects: e.target.checked })}
            />
          </div>
        </div>

        <div className="set-group">
          <h3>Notifications</h3>
          <div className="set-row">
            <label>Enabled</label>
            <input
              type="checkbox"
              checked={config.notifications.enabled}
              onChange={(e) => updateNotify({ enabled: e.target.checked })}
            />
          </div>
          {config.notifications.enabled && (
            <>
              <div className="set-row">
                <label>Service stopped</label>
                <input
                  type="checkbox"
                  checked={config.notifications.serviceStopped}
                  onChange={(e) => updateNotify({ serviceStopped: e.target.checked })}
                />
              </div>
              <div className="set-row">
                <label>Container stopped</label>
                <input
                  type="checkbox"
                  checked={config.notifications.containerStopped}
                  onChange={(e) => updateNotify({ containerStopped: e.target.checked })}
                />
              </div>
              <div className="set-row">
                <label>Health check failed</label>
                <input
                  type="checkbox"
                  checked={config.notifications.healthFailed}
                  onChange={(e) => updateNotify({ healthFailed: e.target.checked })}
                />
              </div>
              <div className="set-row">
                <label>Port conflicts</label>
                <input
                  type="checkbox"
                  checked={config.notifications.portConflict}
                  onChange={(e) => updateNotify({ portConflict: e.target.checked })}
                />
              </div>
              <div className="set-row">
                <label>Recovery</label>
                <input
                  type="checkbox"
                  checked={config.notifications.recovered}
                  onChange={(e) => updateNotify({ recovered: e.target.checked })}
                />
              </div>
              <div className="set-row">
                <label>Cooldown</label>
                <select
                  value={String(config.notifications.cooldownSec)}
                  onChange={(e) => updateNotify({ cooldownSec: Number(e.target.value) })}
                >
                  <option value="30">30s</option>
                  <option value="60">1m</option>
                  <option value="180">3m</option>
                  <option value="600">10m</option>
                </select>
              </div>
            </>
          )}
        </div>

        <div className="set-group">
          <h3>Project roots</h3>
          {config.roots.map((r) => (
            <div key={r} className="root-item">
              <span title={r}>{tildify(r)}</span>
              <button
                className="icon-btn"
                title="Remove root"
                onClick={() => update({ roots: config.roots.filter((x) => x !== r) })}
              >
                <IconX size={10} />
              </button>
            </div>
          ))}
          <div className="set-row">
            <input
              type="text"
              placeholder="~/Dev/other"
              value={newRoot}
              style={{ flex: 1, width: "auto" }}
              onChange={(e) => setNewRoot(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addRoot()}
            />
            <button className="text-btn" onClick={addRoot}>
              Add
            </button>
            <button
              className="text-btn"
              onClick={() => {
                ipc.rescanProjects().catch(() => {});
                push("Rescanning projects…");
              }}
            >
              Rescan
            </button>
          </div>
        </div>

        {config.hiddenProjects.length > 0 && (
          <div className="set-group">
            <h3>Hidden projects</h3>
            {config.hiddenProjects.map((p) => (
              <div key={p} className="root-item">
                <span title={p}>{basename(p)}</span>
                <button
                  className="text-btn"
                  onClick={() =>
                    update({ hiddenProjects: config.hiddenProjects.filter((x) => x !== p) })
                  }
                >
                  Unhide
                </button>
              </div>
            ))}
          </div>
        )}

        <div className="set-group">
          <h3>App</h3>
          <div className="set-row">
            <label>
              Dev Cockpit 1.0.0 <span className="hint">· ⌃⌥D toggles the panel</span>
            </label>
            <button
              className="text-btn"
              style={{ color: "var(--down)" }}
              onClick={() => ipc.quitApp()}
            >
              Quit Dev Cockpit
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
