import { useCallback, useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, ForgeChoice, Instance, LoaderChoice, ManifestVersion, VersionList } from "../api";
import { Button, Field, Meter, Notice, PageHead, Panel } from "../components/ui";
import { fmtMb, when } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

type Kind = "release" | "snapshot" | "old";
type Edition = "vanilla" | "fabric" | "forge";
const kindOf = (v: ManifestVersion): Kind => (v.kind === "release" ? "release" : v.kind === "snapshot" ? "snapshot" : "old");
const kindLabel = (k: string) => ({ release: "Release", snapshot: "Snapshot", old_beta: "Old beta", old_alpha: "Old alpha" } as Record<string, string>)[k] ?? k;
const loaderName = (l: string) => (l === "fabric" ? "Fabric" : l === "forge" ? "Forge" : "Vanilla");

export default function Versions() {
  const c = useApp();
  const [data, setData] = useState<VersionList | null>(null);
  const [q, setQ] = useState("");
  const [kinds, setKinds] = useState<Kind[]>(["release"]);
  const [pick, setPick] = useState<ManifestVersion | null>(null);
  const [fabric, setFabric] = useState<LoaderChoice[] | null>(null);
  const [forge, setForge] = useState<ForgeChoice[] | null>(null);
  const [edition, setEdition] = useState<Edition>("vanilla");
  const [fabricVer, setFabricVer] = useState("");
  const [forgeVer, setForgeVer] = useState("");
  const [name, setName] = useState("");
  const [dir, setDir] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const load = useAction();
  const act = useAction();

  const refresh = useCallback((force: boolean) => load.run(async () => setData(await api.versions(force))), [load.run]);
  useEffect(() => {
    refresh(false);
  }, [refresh]);

  // Loader lists come from Fabric's and Forge's own metadata for exactly the selected version.
  useEffect(() => {
    setFabric(null);
    setForge(null);
    setEdition("vanilla");
    if (!pick) return;
    api.fabricLoaders(pick.id).then((l) => { setFabric(l); setFabricVer(l.find((x) => x.recommended)?.version ?? l[0]?.version ?? ""); }).catch(() => setFabric([]));
    api.forgeVersions(pick.id).then((l) => { setForge(l); setForgeVer(l.find((x) => x.recommended)?.version ?? l[0]?.version ?? ""); }).catch(() => setForge([]));
  }, [pick?.id]);

  const installed = useMemo(() => new Set(data?.installed ?? []), [data]);
  const rows = useMemo(() => {
    const t = q.trim().toLowerCase();
    return (data?.versions ?? []).filter((v) => kinds.includes(kindOf(v)) && (!t || v.id.toLowerCase().includes(t)));
  }, [data, q, kinds]);
  const toggle = (k: Kind) => setKinds((s) => (s.includes(k) ? s.filter((x) => x !== k) : [...s, k]));

  const runInstall = (id: string) =>
    act.run(async () => {
      setBusyId(id);
      try {
        await api.installInstance(id);
      } finally {
        setBusyId(null);
        await c.refresh();
        await refresh(false);
      }
    });

  const create = () =>
    act.run(async () => {
      if (!pick) return;
      const lv = edition === "fabric" ? fabricVer : edition === "forge" ? forgeVer : null;
      const inst = await api.createInstance(name.trim() || `Minecraft ${pick.id} ${loaderName(edition)}`, pick.id, edition, lv, dir.trim());
      c.select(inst.id);
      await c.refresh();
      setName("");
      setBusyId(inst.id);
      try {
        await api.installInstance(inst.id);
      } finally {
        setBusyId(null);
        await c.refresh();
        await refresh(false);
      }
    });

  const removeInstance = (i: Instance) =>
    act.run(async () => {
      if (!(await ask(`Remove "${i.name}" from SAYNAY?`, { title: "Remove instance", kind: "warning" }))) return;
      const files = await ask("Also delete its files? Worlds are moved to the backups folder first. Folders you chose yourself are never deleted.", { title: "Delete files", kind: "warning" });
      await api.deleteInstance(i.id, files);
      await c.refresh();
    });

  const removeVersion = (id: string) =>
    act.run(async () => {
      if (!(await ask(`Remove the downloaded files of version ${id}? You can install it again later.`, { title: "Remove version", kind: "warning" }))) return;
      await api.removeVersion(id);
      await refresh(false);
    });

  const p = c.progress;
  const unusedInstalled = (data?.installed ?? []).filter((id) => !c.instances.some((i) => i.versionId === id || i.mcVersion === id));
  const noFabric = fabric !== null && fabric.length === 0;
  const noForge = forge !== null && forge.length === 0;

  return (
    <div className="page">
      <PageHead
        title="Versions"
        text="Every version comes from Mojang's official list. Files are checked against their published hashes, and interrupted downloads can simply be retried."
        aside={<Button onClick={() => refresh(true)} busy={load.busy}>Refresh list</Button>}
      />
      {load.error && <Notice kind="error">{load.error}</Notice>}
      {data?.warning && <Notice kind="warn">{data.warning}</Notice>}
      {data && !data.warning && data.fromCache && <Notice>Showing the list saved {when(data.fetchedAt)}. Use Refresh list to fetch it again.</Notice>}

      <Panel title={`Minecraft versions${data ? ` (${rows.length})` : ""}`}>
        <div className="row">
          <input type="text" style={{ maxWidth: 260 }} value={q} placeholder="Search, for example 1.20" onChange={(e) => setQ(e.target.value)} />
          {(["release", "snapshot", "old"] as Kind[]).map((k) => (
            <button key={k} className={`chip ${kinds.includes(k) ? "on" : ""}`} onClick={() => toggle(k)}>
              {k === "release" ? "Releases" : k === "snapshot" ? "Snapshots" : "Older versions"}
            </button>
          ))}
        </div>
        <div className="scroll">
          <table>
            <tbody>
              {rows.slice(0, 250).map((v) => (
                <tr key={v.id} onClick={() => setPick(v)} style={{ cursor: "pointer", background: pick?.id === v.id ? "rgba(139,92,246,.14)" : undefined }}>
                  <td><b>{v.id}</b></td>
                  <td><span className="badge">{kindLabel(v.kind)}</span></td>
                  <td className="muted small">{v.releaseTime.slice(0, 10)}</td>
                  <td>{installed.has(v.id) && <span className="badge good">Installed</span>}</td>
                  <td style={{ textAlign: "right" }}><Button small variant={pick?.id === v.id ? "primary" : "ghost"}>{pick?.id === v.id ? "Selected" : "Select"}</Button></td>
                </tr>
              ))}
            </tbody>
          </table>
          {data && rows.length === 0 && <p className="muted" style={{ padding: 14 }}>No versions match.</p>}
          {rows.length > 250 && <p className="muted small" style={{ padding: 10 }}>Showing the newest 250 of {rows.length}. Search to narrow it down.</p>}
        </div>
      </Panel>

      {pick && (
        <Panel title={`Install Minecraft ${pick.id}`}>
          <div className="grid three">
            <div className={`edition ${edition === "vanilla" ? "on" : ""}`} role="button" aria-pressed={edition === "vanilla"} onClick={() => setEdition("vanilla")}>
              <h3>Vanilla</h3>
              <span className="muted small">The original game, with its libraries and assets.</span>
            </div>
            <div className={`edition ${edition === "fabric" ? "on" : ""}`} role="button" aria-disabled={noFabric || fabric === null} onClick={() => !noFabric && fabric !== null && setEdition("fabric")}>
              <h3>Fabric</h3>
              <span className="muted small">{fabric === null ? "Checking compatible loaders..." : noFabric ? "Fabric does not support this version." : `${fabric.length} compatible loader versions.`}</span>
            </div>
            <div className={`edition ${edition === "forge" ? "on" : ""}`} role="button" aria-disabled={noForge || forge === null} onClick={() => !noForge && forge !== null && setEdition("forge")}>
              <h3>Forge</h3>
              <span className="muted small">{forge === null ? "Checking Forge versions..." : noForge ? "No Forge build exists for this version." : `${forge.length} compatible Forge versions.`}</span>
            </div>
          </div>

          <div className="grid two">
            {edition === "fabric" && fabric && fabric.length > 0 && (
              <Field label="Fabric Loader version" hint="Only loaders that Fabric lists for this Minecraft version are shown.">
                <select value={fabricVer} onChange={(e) => setFabricVer(e.target.value)}>
                  {fabric.map((l) => <option key={l.version} value={l.version}>{l.version}{l.recommended ? " (recommended)" : l.stable ? "" : " (unstable)"}</option>)}
                </select>
              </Field>
            )}
            {edition === "forge" && forge && forge.length > 0 && (
              <Field label="Forge version" hint="Installed with the official Forge installer, which SAYNAY downloads and verifies first.">
                <select value={forgeVer} onChange={(e) => setForgeVer(e.target.value)}>
                  {forge.map((f) => <option key={f.version} value={f.version}>{f.label}{f.recommended ? " (recommended)" : f.latest ? " (latest)" : ""}</option>)}
                </select>
              </Field>
            )}
            <Field label="Name">
              <input type="text" value={name} placeholder={`Minecraft ${pick.id} ${loaderName(edition)}`} onChange={(e) => setName(e.target.value)} />
            </Field>
            <Field label="Game folder (optional, absolute path)" hint="Leave empty to let SAYNAY manage it. Pointing at an existing folder keeps its worlds.">
              <input type="text" value={dir} onChange={(e) => setDir(e.target.value)} />
            </Field>
          </div>

          <div className="row between">
            <span className="muted small">Java is downloaded automatically if this version needs a runtime you do not have.</span>
            <Button variant="primary" onClick={create} busy={act.busy || !!busyId} disabled={(edition === "fabric" && !fabricVer) || (edition === "forge" && !forgeVer)}>Install</Button>
          </div>
          {act.error && (
            <Notice kind="error">
              {act.error}
              <div className="muted small" style={{ marginTop: 6 }}>Nothing is lost: verified files are kept. Use Retry install on the instance below to continue.</div>
            </Notice>
          )}
          {busyId && p && (
            <div className="stack">
              <div className="row between small"><span>{p.stage}</span><span>{p.total > 1 ? `${p.done} of ${p.total}` : ""}</span></div>
              <Meter value={p.total > 1 ? p.done : 0} max={Math.max(1, p.total)} label={p.stage} />
            </div>
          )}
        </Panel>
      )}

      <Panel title="Your instances">
        {!pick && act.error && <Notice kind="error">{act.error}</Notice>}
        {c.instances.length === 0 && <p className="muted">Nothing installed yet. Select a version above.</p>}
        <div className="stack">
          {c.instances.map((i) => (
            <div key={i.id} className={`inst-row ${c.selected?.id === i.id ? "sel" : ""}`} onClick={() => c.select(i.id)}>
              <div>
                <b>{i.name}</b>
                <div className="muted small selectable">
                  Minecraft {i.mcVersion}, {loaderName(i.loader)}{i.loaderVersion ? ` ${i.loaderVersion.replace(`${i.mcVersion}-`, "")}` : ""}, {fmtMb(i.settings.ramMb)} RAM
                </div>
                <div className="muted small selectable">{i.gameDir}</div>
              </div>
              <div className="row" onClick={(e) => e.stopPropagation()}>
                <span className={`badge ${i.installed ? "good" : "warn"}`}>{c.running.has(i.id) ? "Running" : i.installed ? "Installed" : "Not installed"}</span>
                {c.running.has(i.id) ? (
                  <Button small variant="danger" onClick={() => act.run(() => api.stop(i.id))}>Stop</Button>
                ) : (
                  <Button small variant="primary" disabled={!i.installed || !c.account} onClick={() => act.run(() => api.launch(i.id))}>Launch</Button>
                )}
                <Button small busy={busyId === i.id} disabled={c.running.has(i.id) || !!busyId} onClick={() => runInstall(i.id)}>{i.installed ? "Repair" : "Retry install"}</Button>
                <Button small variant="danger" disabled={c.running.has(i.id)} onClick={() => removeInstance(i)}>Remove</Button>
              </div>
            </div>
          ))}
        </div>
        {!c.account && c.instances.length > 0 && <p className="muted small">Add an account on the Accounts page to launch.</p>}
      </Panel>

      {unusedInstalled.length > 0 && (
        <Panel title="Downloaded versions no instance uses">
          <table>
            <tbody>
              {unusedInstalled.map((id) => (
                <tr key={id}>
                  <td><b>{id}</b></td>
                  <td style={{ textAlign: "right" }}><Button small variant="danger" onClick={() => removeVersion(id)}>Remove files</Button></td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="muted small">Shared libraries and assets are kept, because other versions use them.</p>
        </Panel>
      )}
    </div>
  );
}
