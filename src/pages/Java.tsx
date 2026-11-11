import { useEffect, useState } from "react";
import { api, JavaInstall } from "../api";
import { Button, Notice, PageHead, Panel } from "../components/ui";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

const compatible = (j: JavaInstall, need: number | null) => need == null || (need <= 8 ? j.major === 8 : j.major >= need);

export default function Java() {
  const c = useApp();
  const sel = c.selected;
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  const [need, setNeed] = useState<number | null>(null);
  const [major, setMajor] = useState(21);
  const scan = useAction();
  const act = useAction();

  const rescan = () => scan.run(async () => setJavas(await api.javas()));
  useEffect(() => {
    rescan();
  }, [scan.run]);
  useEffect(() => {
    if (sel) api.requiredJava(sel.id).then(setNeed).catch(() => setNeed(null));
    else setNeed(null);
  }, [sel?.id, sel?.installed]);

  const pick = (path: string | null) =>
    act.run(async () => {
      if (!sel) return;
      await api.saveSettings(sel.id, sel.settings, sel.profile, path);
      await c.refresh();
    });

  const haveNeeded = need != null && javas.some((j) => compatible(j, need));

  return (
    <div className="page">
      <PageHead title="Java" text="SAYNAY finds the Java runtimes on this PC, picks the one a Minecraft version needs, and can download a verified runtime when none fits." />
      {(scan.error || act.error) && <Notice kind="error">{scan.error ?? act.error}</Notice>}

      {sel && (
        <Panel title={`Selected instance: ${sel.name}`}>
          <p>
            Minecraft {sel.mcVersion} needs <b>{need == null ? "a Java version that is known after install" : need <= 8 ? "Java 8 exactly" : `Java ${need} or newer`}</b>. Using:{" "}
            <b className="selectable">{sel.javaPath ?? "Automatic (the best match is chosen at launch)"}</b>
          </p>
          <div className="row">
            {sel.javaPath && <Button small onClick={() => pick(null)}>Switch back to automatic</Button>}
            {need != null && !haveNeeded && (
              <Button variant="primary" small busy={act.busy} onClick={() => act.run(async () => { await api.installJava(need); await rescan(); })}>
                Download Java {need}
              </Button>
            )}
            {need != null && haveNeeded && <span className="badge good">A compatible Java is available</span>}
          </div>
        </Panel>
      )}

      <Panel
        title={`Installed runtimes (${javas.length})`}
        aside={<Button small onClick={rescan} busy={scan.busy}>Scan again</Button>}
      >
        {javas.length === 0 && !scan.busy && <p className="muted">No Java was found. Download one below.</p>}
        <table>
          <tbody>
            {javas.map((j) => (
              <tr key={j.path}>
                <td>
                  <b>Java {j.major}</b> <span className="muted small">{j.version}</span>{" "}
                  {j.managed && <span className="badge">Managed by SAYNAY</span>}
                  <div className="muted small selectable">{j.path}</div>
                </td>
                <td>{sel && need != null && (compatible(j, need) ? <span className="badge good">Fits this instance</span> : <span className="badge warn">Does not fit</span>)}</td>
                <td style={{ textAlign: "right" }}>
                  {sel && <Button small disabled={sel.javaPath === j.path || (need != null && !compatible(j, need))} busy={act.busy} onClick={() => pick(j.path)}>{sel.javaPath === j.path ? "In use" : "Use for this instance"}</Button>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Panel>

      <Panel title="Download a runtime">
        <p className="muted small">Runtimes come from the Eclipse Temurin project through the Adoptium API and are checked against their published SHA-256 before use. If that Java is already downloaded, it is reused instead of downloaded again.</p>
        <div className="row">
          <select style={{ width: 180 }} value={major} onChange={(e) => setMajor(Number(e.target.value))}>
            {[8, 11, 17, 21, 25].map((m) => <option key={m} value={m}>Java {m}</option>)}
          </select>
          <Button busy={act.busy} onClick={() => act.run(async () => { await api.installJava(major); await rescan(); })}>Download</Button>
        </div>
      </Panel>
    </div>
  );
}
