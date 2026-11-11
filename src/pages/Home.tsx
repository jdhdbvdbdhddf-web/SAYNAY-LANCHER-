import { useEffect, useState } from "react";
import { api, BenchResult, Usage } from "../api";
import { Button, Meter, Notice, Panel } from "../components/ui";
import { fmtMb, fmtNum, profileName, when } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

export default function Home() {
  const c = useApp();
  const sel = c.selected;
  const hw = c.hw;
  const [usage, setUsage] = useState<Usage | null>(null);
  const [last, setLast] = useState<BenchResult | null>(null);
  const { busy, error, run } = useAction();

  // Live usage is only polled while Home is on screen, to keep the launcher itself light.
  useEffect(() => {
    let alive = true;
    const tick = () =>
      api
        .usage()
        .then((u) => {
          if (alive) setUsage(u);
        })
        .catch(() => undefined);
    tick();
    const t = window.setInterval(tick, 2000);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, []);

  useEffect(() => {
    api
      .benchmarks(sel?.id ?? null)
      .then((b) => setLast(b.find((x) => x.avgFps != null) ?? b[0] ?? null))
      .catch(() => undefined);
  }, [sel?.id]);

  const running = sel ? c.running.has(sel.id) : false;
  const label = !sel ? "Create instance" : !sel.installed ? "Install" : running ? "Stop" : "Play";
  const act = () =>
    run(async () => {
      if (!sel || !sel.installed) return c.go("versions");
      if (running) return api.stop(sel.id);
      if (!c.account) return c.go("accounts");
      await api.launch(sel.id);
    });

  return (
    <div className="page">
      <section className="hero">
        <div>
          <h1>{sel ? sel.name : "Welcome to SAYNAY"}</h1>
          <p className="muted">
            {sel ? `Minecraft ${sel.mcVersion}${sel.loader === "fabric" ? " with Fabric" : ""}` : "Create your first instance to start playing."}
          </p>
          <div className="facts">
            <div>
              <small>Optimization profile</small>
              <b>{sel ? profileName(sel.profile) : "None"}</b>
            </div>
            <div>
              <small>Memory for Minecraft</small>
              <b>{sel ? fmtMb(sel.settings.ramMb) : "n/a"}</b>
            </div>
            <div>
              <small>Account</small>
              <b>{c.account ? `${c.account.name}${c.account.kind === "offline" ? " (offline test)" : ""}` : "No account"}</b>
            </div>
          </div>
        </div>
        <button className={`play ${running ? "stop" : ""}`} onClick={act} aria-busy={busy}>
          {label}
        </button>
      </section>
      {error && <Notice kind="error">{error}</Notice>}

      <div className="grid two">
        <Panel title="Instances" aside={<Button small onClick={() => c.go("versions")}>Manage</Button>}>
          {c.instances.length === 0 && <p className="muted">No instances yet. Open Versions to install Minecraft.</p>}
          <div className="stack">
            {c.instances.map((i) => (
              <div key={i.id} className={`inst-row ${sel?.id === i.id ? "sel" : ""}`} onClick={() => c.select(i.id)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && c.select(i.id)}>
                <div>
                  <b>{i.name}</b>
                  <div className="muted small">
                    Minecraft {i.mcVersion}, {i.loader === "fabric" ? "Fabric" : "Vanilla"}
                  </div>
                </div>
                <span className={`badge ${i.installed ? "good" : "warn"}`}>{c.running.has(i.id) ? "Running" : i.installed ? "Installed" : "Not installed"}</span>
              </div>
            ))}
          </div>
        </Panel>

        <Panel title="Right now">
          <div className="stack">
            <div>
              <div className="row between small">
                <span>CPU usage</span>
                <span>{usage ? `${usage.cpuPct.toFixed(0)}%` : "..."}</span>
              </div>
              <Meter value={usage?.cpuPct ?? 0} max={100} label="CPU usage" />
            </div>
            <div>
              <div className="row between small">
                <span>Memory in use</span>
                <span>{usage ? `${fmtMb(usage.ramUsedMb)} of ${fmtMb(usage.ramTotalMb)}` : "..."}</span>
              </div>
              <Meter value={usage?.ramUsedMb ?? 0} max={usage?.ramTotalMb ?? 1} label="Memory in use" />
            </div>
          </div>
          <h3>Last benchmark</h3>
          {last ? (
            <div className="row" style={{ gap: 28 }}>
              <div className="stat"><b>{fmtNum(last.avgFps, 0)}</b><span>average FPS</span></div>
              <div className="stat"><b>{fmtNum(last.low1Fps, 0)}</b><span>1% low FPS</span></div>
              <div className="stat"><b>{last.ramAvgMb != null ? fmtMb(last.ramAvgMb) : "n/a"}</b><span>average RAM</span></div>
              <div className="muted small" style={{ alignSelf: "end" }}>{last.label}, {when(last.createdAt)}</div>
            </div>
          ) : (
            <p className="muted">No benchmark has been run for this instance. Open Benchmark to record your baseline.</p>
          )}
        </Panel>
      </div>

      <Panel title="Your hardware">
        {!hw ? (
          <p className="muted">Reading hardware...</p>
        ) : (
          <dl className="kv selectable">
            <dt>CPU</dt>
            <dd>{hw.cpuName ?? "Unavailable"}{hw.physicalCores ? `, ${hw.physicalCores} cores and ${hw.threads} threads` : `, ${hw.threads} threads`}</dd>
            <dt>GPU</dt>
            <dd>{hw.gpus.length ? hw.gpus.join(", ") : "Unavailable on this system"}</dd>
            <dt>Memory</dt>
            <dd>{fmtMb(hw.ramTotalMb)} installed, {fmtMb(hw.ramAvailableMb)} available</dd>
            <dt>System</dt>
            <dd>{hw.os} {hw.osVersion ?? ""} ({hw.arch})</dd>
            <dt>Storage</dt>
            <dd>{hw.dataDirFreeMb != null ? `${fmtMb(hw.dataDirFreeMb)} free where SAYNAY stores games` : "Unavailable"}</dd>
          </dl>
        )}
      </Panel>
    </div>
  );
}
