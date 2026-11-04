import { useCallback, useEffect, useState } from "react";
import { api, BenchResult } from "../api";
import { CompareRows, FrameLine } from "../components/Charts";
import { Button, Field, NoInstance, Notice, PageHead, Panel } from "../components/ui";
import { fmtMb, fmtNum, profileName, when } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

export default function Benchmark() {
  const c = useApp();
  const sel = c.selected;
  const [results, setResults] = useState<BenchResult[]>([]);
  const [label, setLabel] = useState("Before optimization");
  const [secs, setSecs] = useState(60);
  const [beforeId, setBeforeId] = useState<number | null>(null);
  const [afterId, setAfterId] = useState<number | null>(null);
  const act = useAction();
  const id = sel?.id ?? null;

  const load = useCallback(async () => {
    const r = await api.benchmarks(id);
    setResults(r);
    return r;
  }, [id]);

  useEffect(() => {
    load()
      .then((r) => {
        setAfterId(r[0]?.id ?? null);
        setBeforeId(r[1]?.id ?? null);
      })
      .catch(() => undefined);
  }, [load]);

  if (!sel) {
    return (
      <div className="page">
        <PageHead title="Benchmark" />
        <NoInstance onCreate={() => c.go("versions")} />
      </div>
    );
  }

  const start = () =>
    act.run(async () => {
      const prev = results[0]?.id ?? null;
      const r = await api.runBenchmark(sel.id, label, secs);
      await load();
      setBeforeId(prev);
      setAfterId(r.id);
    });

  const before = results.find((r) => r.id === beforeId);
  const after = results.find((r) => r.id === afterId);
  const mismatch = before && after && (before.mcVersion !== after.mcVersion || before.durationS !== after.durationS);
  const running = c.running.has(sel.id);

  return (
    <div className="page">
      <PageHead title="Benchmark" text="Measure real performance before and after a change. SAYNAY never estimates or invents FPS: if a number could not be measured, it shows n/a." />

      <Panel title="Run a benchmark">
        <ol className="muted" style={{ margin: 0, paddingLeft: 18 }}>
          <li>Start the benchmark. Minecraft opens with the SAYNAY Bench mod enabled.</li>
          <li>Load the same world every time and stand at the same spot. Do not move during the recording.</li>
          <li>Recording starts 15 seconds after you enter the world, runs for the chosen time, then the game closes itself.</li>
        </ol>
        <div className="grid two">
          <Field label="Label">
            <input type="text" value={label} onChange={(e) => setLabel(e.target.value)} />
          </Field>
          <Field label="Recording length">
            <select value={secs} onChange={(e) => setSecs(Number(e.target.value))}>
              <option value={30}>30 seconds</option>
              <option value={60}>60 seconds</option>
              <option value={120}>120 seconds</option>
            </select>
          </Field>
        </div>
        <div className="row">
          <Button variant="primary" busy={act.busy} disabled={!sel.installed || !c.account || running} onClick={start}>Start benchmark</Button>
          {act.busy && <span className="muted small">Benchmark in progress. Switch to Minecraft and load your world.</span>}
          {!c.account && <span className="muted small">Sign in on the Play page first.</span>}
        </div>
        {act.error && <Notice kind="error">{act.error}</Notice>}
        <p className="muted small">Fair comparisons need identical conditions: same world and spot, same resolution, same render distance, same background apps. Differences in hardware, drivers and Minecraft version change the results.</p>
      </Panel>

      {results.length >= 2 && (
        <Panel title="Compare two runs">
          <div className="grid two">
            <Field label="Before">
              <select value={beforeId ?? ""} onChange={(e) => setBeforeId(Number(e.target.value))}>
                {results.map((r) => <option key={r.id} value={r.id}>{r.label}, {when(r.createdAt)}</option>)}
              </select>
            </Field>
            <Field label="After">
              <select value={afterId ?? ""} onChange={(e) => setAfterId(Number(e.target.value))}>
                {results.map((r) => <option key={r.id} value={r.id}>{r.label}, {when(r.createdAt)}</option>)}
              </select>
            </Field>
          </div>
          {before && after && (
            <>
              {mismatch && <Notice kind="warn">These runs used a different Minecraft version or recording length, so the comparison is not like for like.</Notice>}
              <div className="legend">
                <span><i style={{ background: "#4a5a9c" }} />{before.label}</span>
                <span><i style={{ background: "linear-gradient(90deg,#8b5cf6,#38bdf8)" }} />{after.label}</span>
              </div>
              <CompareRows
                rows={[
                  { label: "Average FPS", before: before.avgFps, after: after.avgFps, unit: "FPS", higherBetter: true },
                  { label: "1% low FPS", before: before.low1Fps, after: after.low1Fps, unit: "FPS", higherBetter: true },
                  { label: "Frame-time spread", before: before.frameMsStddev, after: after.frameMsStddev, unit: "ms", higherBetter: false, digits: 2 },
                  { label: "Average RAM", before: before.ramAvgMb, after: after.ramAvgMb, unit: "MB", higherBetter: false, digits: 0 },
                  { label: "Peak RAM", before: before.ramPeakMb, after: after.ramPeakMb, unit: "MB", higherBetter: false, digits: 0 },
                  { label: "CPU use", before: before.cpuAvgPct, after: after.cpuAvgPct, unit: "%", higherBetter: false },
                  { label: "Load time", before: before.loadSecs, after: after.loadSecs, unit: "s", higherBetter: false },
                ]}
              />
              <h3>Frame times</h3>
              <FrameLine
                series={[
                  { label: before.label, color: "#6b7bd6", values: before.frameTimesMs },
                  { label: after.label, color: "#38bdf8", values: after.frameTimesMs },
                ]}
              />
              <p className="muted small">Lower and flatter lines mean smoother gameplay. Spikes are stutters. Frame-time spread is the standard deviation of every recorded frame.</p>
              {[...before.notes, ...after.notes].map((n, i) => <Notice key={i} kind="warn">{n}</Notice>)}
            </>
          )}
        </Panel>
      )}

      <Panel title="History">
        {results.length === 0 && <p className="muted">No benchmarks yet. Run one to record your baseline before changing anything.</p>}
        {results.length > 0 && (
          <table>
            <thead>
              <tr><th>Run</th><th>Profile</th><th>Avg FPS</th><th>1% low</th><th>Avg RAM</th><th>Load</th><th /></tr>
            </thead>
            <tbody>
              {results.map((r) => (
                <tr key={r.id}>
                  <td><b>{r.label}</b><div className="muted small">{when(r.createdAt)}, Minecraft {r.mcVersion}</div></td>
                  <td>{profileName(r.profile)}</td>
                  <td>{fmtNum(r.avgFps, 1)}</td>
                  <td>{fmtNum(r.low1Fps, 1)}</td>
                  <td>{r.ramAvgMb != null ? fmtMb(r.ramAvgMb) : "n/a"}</td>
                  <td>{r.loadSecs != null ? `${r.loadSecs.toFixed(1)} s` : "n/a"}</td>
                  <td style={{ textAlign: "right" }}>
                    <Button small variant="danger" onClick={() => act.run(async () => { await api.deleteBenchmark(r.id); await load(); })}>Delete</Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>
    </div>
  );
}
