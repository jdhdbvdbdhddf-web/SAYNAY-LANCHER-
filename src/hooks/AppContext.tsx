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
  /** The active account, or null when none has been added. */
  account: Account | null;
  accounts: Account[];
  refreshAccounts: () => Promise<void>;
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
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
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

  const refreshAccounts = useCallback(async () => {
    const r = await api.accounts();
    setAccounts(r.accounts);
    setActiveId(r.active);
  }, []);

  const select = useCallback((id: string) => {
    localStorage.setItem("saynay.selected", id);
    setSelectedId(id);
  }, []);

  useEffect(() => {
    api.hardware().then(setHw).catch(() => setHw(null));
    refresh().catch(() => undefined);
    refreshAccounts().catch(() => undefined);
    api.versions(false).catch(() => undefined); // warms the version list cache at startup
    api.running().then((r) => setRunning(new Set(r))).catch(() => undefined);
  }, [refresh, refreshAccounts]);

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
      listen<Account>("auth-complete", () => {
        api.accounts().then((r) => { setAccounts(r.accounts); setActiveId(r.active); }).catch(() => undefined);
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((off) => off()));
    };
  }, []);

  const selected = useMemo(() => instances.find((i) => i.id === selectedId) ?? null, [instances, selectedId]);
  const logsOf = useCallback((id: string) => logs.current[id] ?? [], []);

  const account = accounts.find((a) => a.id === activeId) ?? null;
  const value: Ctx = { page, go, hw, instances, selected, select, refresh, account, accounts, refreshAccounts, running, logsOf, exits, progress };
  return <C.Provider value={value}>{children}</C.Provider>;
}
