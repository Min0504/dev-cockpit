import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, onManagedExited, onPaused, onSnapshot, onToast } from "./ipc";
import type { AppConfig, Snapshot } from "./types";
import { emptySnapshot } from "./types";

export function useSnapshot(): { snapshot: Snapshot; ready: boolean } {
  const [snapshot, setSnapshot] = useState<Snapshot>(emptySnapshot);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let alive = true;
    ipc
      .getSnapshot()
      .then((s) => {
        if (alive && s.seq > 0) {
          setSnapshot(s);
          setReady(true);
        }
      })
      .catch(() => {});
    const un = onSnapshot((s) => {
      if (!alive) return;
      setSnapshot(s);
      setReady(true);
    });
    return () => {
      alive = false;
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  return { snapshot, ready };
}

export function useConfig(): {
  config: AppConfig | null;
  save: (next: AppConfig) => Promise<void>;
} {
  const [config, setConfig] = useState<AppConfig | null>(null);

  useEffect(() => {
    ipc.getConfig().then(setConfig).catch(() => {});
  }, []);

  const save = useCallback(async (next: AppConfig) => {
    setConfig(next); // optimistic
    const applied = await ipc.setConfig(next);
    setConfig(applied);
  }, []);

  return { config, save };
}

export interface Toast {
  id: number;
  text: string;
}

export function useToasts(): { toasts: Toast[]; push: (text: string) => void } {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const push = useCallback((text: string) => {
    const id = nextId.current++;
    setToasts((cur) => [...cur.slice(-2), { id, text }]);
    setTimeout(() => {
      setToasts((cur) => cur.filter((t) => t.id !== id));
    }, 4200);
  }, []);

  useEffect(() => {
    const un = onToast((msg) => push(msg));
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, [push]);

  return { toasts, push };
}

export function usePausedState(): [boolean, (p: boolean) => void] {
  const [paused, setPausedLocal] = useState(false);

  useEffect(() => {
    ipc.isPaused().then(setPausedLocal).catch(() => {});
    const un = onPaused(setPausedLocal);
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  const setPaused = useCallback((p: boolean) => {
    setPausedLocal(p);
    ipc.setPaused(p).catch(() => {});
  }, []);

  return [paused, setPaused];
}

/** Re-render periodically so relative times (uptime, "3m ago") stay fresh. */
export function useClock(ms: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

export function useManagedExitToast(push: (t: string) => void): void {
  useEffect(() => {
    const un = onManagedExited((id) => {
      const name = id.split(":").pop() ?? id;
      push(`Process exited: ${name}`);
    });
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, [push]);
}
