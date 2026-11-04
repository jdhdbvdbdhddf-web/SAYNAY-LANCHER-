import { useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, Instance, ManifestVersion } from "../api";
import { Button, Field, Meter, Notice, PageHead, Panel } from "../components/ui";
import { fmtMb } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

export default function Versions() {
  const c = useApp();
  const [versions, setVersions] = useState<ManifestVersion[]>([]);
  const [snap, setSnap] = useState(false);
  const [name, setName] = useState("");
  const [ver, setVer] = useState("");
  const [loader, setLoader] = useState<"vanilla" | "fabric">("fabric");
  const [dir, setDir] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const load = useAction();
  const act = useAction();

  useEffect(() => {
    load.run(async () => setVersions(await api.versions()));
  }, [load.run]);

  const shown = useMemo(() => versions.filter((v) => v.kind === "release" || (snap && v.kind === "snapshot")), [versions, snap]);
  useEffect(() => {
    if (shown.length && !shown.some((v) => v.id === ver)) setVer(shown[0].id);
  }, [shown, ver]);

  const install = (id: string) =>
    act.run(async () => {
      setBusyId(id);
      try {
        await api.installInstance(id);
      } finally {
        setBusyId(null);
        await c.refresh();
      }
    });

  const create = () =>
    act.run(async () => {
      const inst = await api.createInstance(name.trim() || `Minecraft ${ver}`, ver, loader, dir.trim());
      c.select(inst.id);
      await c.refresh();
      setName("");
      setBusyId(inst.id);
      try {
        await api.installInstance(inst.id);
      } finally {
        setBusyId(null);
        await c.refresh();
      }
    });

  const remove = (i: Instance) =>
    act.run(async () => {
      if (!(await ask(`Remove "${i.name}" from SAYNAY?`, { title: "Remove instance", kind: "warning" }))) return;
      const files = await ask("Also delete its files? Worlds are moved to the backups folder first. Folders you chose yourself are never deleted.", { title: "Delete files", kind: "warning" });
      await api.deleteInstance(i.id, files);
      await c.refresh();
    });

  const p = c.progress;
  return (
    <div className="page">
      <PageHead title="Versions" text="Install the original Minecraft Java Edition straight from Mojang's official servers. Every file is checked against its published hash." />

      <Panel title="New instance">
        <div className="grid two">
          <Field label="Name">
            <input type="text" value={name} placeholder={ver ? `Minecraft ${ver}` : "My instance"} onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label="Minecraft version">
            <select value={ver} onChange={(e) => setVer(e.target.value)} disabled={!shown.length}>
              {shown.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id}
                  {v.kind === "snapshot" ? " (snapshot)" : ""}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Loader">
            <select value={loader} onChange={(e) => setLoader(e.target.value as "vanilla" | "fabric")}>
              <option value="fabric">Fabric (supports performance mods)</option>
              <option value="vanilla">Vanilla</option>
            </select>
          </Field>
          <Field label="Game folder (optional, absolute path)">
            <input type="text" value={dir} placeholder="Leave empty to let SAYNAY manage it" onChange={(e) => setDir(e.target.value)} />
          </Field>
        </div>
        <div className="row between">
          <label className="check">
            <input type="checkbox" checked={snap} onChange={(e) => setSnap(e.target.checked)} /> Show snapshots
          </label>
          <Button variant="primary" onClick={create} busy={act.busy || load.busy} disabled={!ver}>
            Create and install
          </Button>
        </div>
        {load.error && <Notice kind="error">{load.error}</Notice>}
        {act.error && <Notice kind="error">{act.error}</Notice>}
        {busyId && p && (
          <div className="stack">
            <div className="row between small">
              <span>{p.stage}</span>
              <span>{p.total > 1 ? `${p.done} of ${p.total}` : ""}</span>
            </div>
            <Meter value={p.total > 1 ? p.done : 0} max={Math.max(1, p.total)} label={p.stage} />
          </div>
        )}
      </Panel>

      <Panel title="Your instances">
        {c.instances.length === 0 && <p className="muted">Nothing installed yet.</p>}
        <div className="stack">
          {c.instances.map((i) => (
            <div key={i.id} className={`inst-row ${c.selected?.id === i.id ? "sel" : ""}`} onClick={() => c.select(i.id)}>
              <div>
                <b>{i.name}</b>
                <div className="muted small selectable">
                  Minecraft {i.mcVersion}, {i.loader === "fabric" ? `Fabric ${i.loaderVersion ?? ""}` : "Vanilla"}, {fmtMb(i.settings.ramMb)} RAM
                </div>
                <div className="muted small selectable">{i.gameDir}</div>
              </div>
              <div className="row" onClick={(e) => e.stopPropagation()}>
                <span className={`badge ${i.installed ? "good" : "warn"}`}>{i.installed ? "Installed" : "Not installed"}</span>
                <Button small busy={busyId === i.id} disabled={c.running.has(i.id)} onClick={() => install(i.id)}>
                  {i.installed ? "Repair" : "Install"}
                </Button>
                <Button small variant="danger" disabled={c.running.has(i.id)} onClick={() => remove(i)}>
                  Remove
                </Button>
              </div>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}
