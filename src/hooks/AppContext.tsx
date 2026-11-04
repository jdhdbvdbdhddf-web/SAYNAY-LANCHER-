import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Account, api, ExitInfo, Hardware, Instance, InstallProgress, Page } from "../api";

interface Ctx {
  page: Page;
  go: (p: Page) => void;
  hw: Hardware | null;
  instances: Instance[];
  selected: Instance | null;
  select: (id: string) => void;
  refresh: () => Promise<void>;
  account: Account | null;
  setAccount: (a: Account | null) => void;
  running: Set<string>;
  logsOf: (id: string) => string[];
  exits: Record<string, ExitInfo>;
  progress: InstallProgress | null;
}

const C = createContext<Ctx | null>(null);

export function useApp(): Ctx {
  const v = useContext(C);
  if (!v) throw new Error("AppProvider is missing");
  return v;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [page, go] = useState<Page>("home");
  const [hw, setHw] = useState<Hardware | null>(null);
  const [instances, setInstances] = useState<Instance[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(() => localStorage.getItem("saynay.selected"));
  const [account, setAccount] = useState<Account | null>(null);
  const [running, setRunning] = useState<Set<string>>(new Set());
  const [exits, setExits] = useState<Record<string, ExitInfo>>({});
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [, setTick] = useState(0);
  const logs = useRef<Record<string, string[]>>({});
  const pending = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const list = await api.instances();
    setInstances(list);
    setSelectedId((cur) => (cur && list.some((i) => i.id === cur) ? cur : list[0]?.id ?? null));
  }, []);

  const select = useCallback((id: string) => {
    localStorage.setItem("saynay.selected", id);
    setSelectedId(id);
  }, []);

  useEffect(() => {
    api.hardware().then(setHw).catch(() => setHw(null));
    refresh().catch(() => undefined);
    api.account().then(setAccount).catch(() => undefined);
    api.running().then((r) => setRunning(new Set(r))).catch(() => undefined);
  }, [refresh]);

  // One set of global listeners. Log lines are batched so a chatty game cannot flood React.
  useEffect(() => {
    const subs = [
      listen<{ instanceId: string }>("game-started", (e) => {
        logs.current[e.payload.instanceId] = [];
        setRunning((s) => new Set(s).add(e.payload.instanceId));
        setExits((x) => {
          const n = { ...x };
          delete n[e.payload.instanceId];
          return n;
        });
      }),
      listen<{ instanceId: string; line: string }>("game-log", (e) => {
        const arr = (logs.current[e.payload.instanceId] ??= []);
        arr.push(e.payload.line);
        if (arr.length > 600) arr.splice(0, arr.length - 600);
        if (pending.current === null) {
          pending.current = window.setTimeout(() => {
            pending.current = null;
            setTick((t) => t + 1);
          }, 250);
        }
      }),
      listen<ExitInfo>("game-exit", (e) => {
        setRunning((s) => {
          const n = new Set(s);
          n.delete(e.payload.instanceId);
          return n;
        });
        setExits((x) => ({ ...x, [e.payload.instanceId]: e.payload }));
      }),
      listen<InstallProgress>("install-progress", (e) => setProgress(e.payload)),
      listen<Account>("auth-complete", (e) => setAccount(e.payload)),
    ];
    return () => {
      subs.forEach((p) => p.then((off) => off()));
    };
  }, []);

  const selected = useMemo(() => instances.find((i) => i.id === selectedId) ?? null, [instances, selectedId]);
  const logsOf = useCallback((id: string) => logs.current[id] ?? [], []);

  const value: Ctx = { page, go, hw, instances, selected, select, refresh, account, setAccount, running, logsOf, exits, progress };
  return <C.Provider value={value}>{children}</C.Provider>;
}
