import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { ipc, onLogBatch, onLogEnded } from "../ipc";
import type { LogLine } from "../types";
import { IconArrowDown, IconPause, IconPlay, IconSearch, IconX } from "../icons";

const MAX_CLIENT_LINES = 2500;

interface Props {
  session: string;
  title: string;
  onClose: () => void;
}

export function LogsView({ session, title, onClose }: Props) {
  const [lines, setLines] = useState<LogLine[]>([]);
  const [query, setQuery] = useState("");
  const [paused, setPaused] = useState(false);
  const [ended, setEnded] = useState(false);
  const [follow, setFollow] = useState(true);
  const bufRef = useRef<LogLine[]>([]);
  const pausedRef = useRef(false);
  const bodyRef = useRef<HTMLDivElement | null>(null);

  pausedRef.current = paused;

  const append = useCallback((batch: LogLine[]) => {
    if (pausedRef.current) {
      bufRef.current = [...bufRef.current, ...batch].slice(-MAX_CLIENT_LINES);
      return;
    }
    setLines((cur) => [...cur, ...batch].slice(-MAX_CLIENT_LINES));
  }, []);

  useEffect(() => {
    let alive = true;
    setLines([]);
    bufRef.current = [];
    setEnded(false);
    ipc
      .getLogLines(session)
      .then((p) => {
        if (!alive) return;
        setLines(p.lines.slice(-MAX_CLIENT_LINES));
        setEnded(p.ended);
      })
      .catch(() => {});
    const un1 = onLogBatch(session, (batch) => append(batch));
    const un2 = onLogEnded(session, () => setEnded(true));
    return () => {
      alive = false;
      un1.then((f) => f()).catch(() => {});
      un2.then((f) => f()).catch(() => {});
    };
  }, [session, append]);

  useEffect(() => {
    if (follow && bodyRef.current) {
      bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
    }
  }, [lines, follow]);

  const onScroll = () => {
    const el = bodyRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
    if (atBottom !== follow) setFollow(atBottom);
  };

  const resume = () => {
    setPaused(false);
    if (bufRef.current.length > 0) {
      const buf = bufRef.current;
      bufRef.current = [];
      setLines((cur) => [...cur, ...buf].slice(-MAX_CLIENT_LINES));
    }
  };

  const close = () => {
    ipc.closeLogSession(session).catch(() => {});
    onClose();
  };

  const q = query.trim().toLowerCase();
  const visible = q ? lines.filter((l) => l.line.toLowerCase().includes(q)) : lines;

  return (
    <div className="sheet">
      <div className="sheet-head">
        <span className="sheet-title">{title}</span>
        {ended && <span className="health-chip warn">ended</span>}
        <span style={{ flex: 1 }} />
        <button className="icon-btn" title="Close" onClick={close}>
          <IconX />
        </button>
      </div>
      <div className="log-toolbar">
        <span className="search">
          <IconSearch size={11} />
          <input
            placeholder="Filter logs"
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
          title={paused ? "Resume stream" : "Pause stream"}
          onClick={() => (paused ? resume() : setPaused(true))}
        >
          {paused ? <IconPlay size={11} /> : <IconPause size={11} />}
        </button>
        <button
          className={`icon-btn${follow ? " active" : ""}`}
          title="Follow tail"
          onClick={() => {
            setFollow(true);
            if (bodyRef.current) bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
          }}
        >
          <IconArrowDown size={11} />
        </button>
        <button className="text-btn" onClick={() => setLines([])}>
          Clear
        </button>
        <span className="log-meta">
          {visible.length}
          {q ? ` / ${lines.length}` : ""} lines
          {paused && bufRef.current.length > 0 ? ` · ${bufRef.current.length} buffered` : ""}
        </span>
      </div>
      <div className="log-body" ref={bodyRef} onScroll={onScroll}>
        {visible.map((l) => (
          <div key={l.seq} className={`log-line${l.stderr ? " stderr" : ""}`}>
            {q ? highlight(l.line, q) : l.line}
          </div>
        ))}
        {visible.length === 0 && (
          <div style={{ color: "var(--text-3)", padding: 12 }}>
            {q ? "No matching lines" : "Waiting for output…"}
          </div>
        )}
      </div>
    </div>
  );
}

function highlight(line: string, q: string): ReactNode {
  const lower = line.toLowerCase();
  const parts: ReactNode[] = [];
  let i = 0;
  let key = 0;
  while (i < line.length) {
    const at = lower.indexOf(q, i);
    if (at === -1) {
      parts.push(line.slice(i));
      break;
    }
    if (at > i) parts.push(line.slice(i, at));
    parts.push(<mark key={key++}>{line.slice(at, at + q.length)}</mark>);
    i = at + q.length;
  }
  return parts;
}
