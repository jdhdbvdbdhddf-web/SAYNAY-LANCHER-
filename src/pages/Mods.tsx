import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, ModEntry, ModReport, PackItem } from "../api";
import { Button, NoInstance, Notice, PageHead, Panel } from "../components/ui";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

type Tab = "mods" | "resourcepacks" | "shaderpacks";
const TABS: { id: Tab; label: string }[] = [
  { id: "mods", label: "Mods" },
  { id: "resourcepacks", label: "Resource packs" },
  { id: "shaderpacks", label: "Shader packs" },
];

export default function Mods() {
  const c = useApp();
  const sel = c.selected;
  const id = sel?.id;
  const [tab, setTab] = useState<Tab>("mods");
  const [mods, setMods] = useState<ModEntry[]>([]);
  const [catalog, setCatalog] = useState<[string, string][]>([]);
  const [packs, setPacks] = useState<PackItem[]>([]);
  const [reports, setReports] = useState<ModReport[]>([]);
  const [slug, setSlug] = useState("");
  const [working, setWorking] = useState<string | null>(null);
  const act = useAction();

  const reload = useCallback(async () => {
    if (!id) return;
    if (tab === "mods") setMods(await api.mods(id));
    else setPacks(await api.packs(id, tab));
  }, [id, tab]);

  useEffect(() => {
    api.perfMods().then(setCatalog).catch(() => undefined);
  }, []);
  useEffect(() => {
    act.run(reload);
  }, [reload, act.run]);

  if (!sel || !id) {
    return (
      <div className="page">
        <PageHead title="Mods" />
        <NoInstance onCreate={() => c.go("versions")} />
      </div>
    );
  }

  const install = (s: string) =>
    act.run(async () => {
      setWorking(s);
      try {
        setReports(await api.installMod(id, s));
        await reload();
      } finally {
        setWorking(null);
      }
    });

  const installProfile = () =>
    act.run(async () => {
      setWorking("profile");
      try {
        setReports(await api.installProfileMods(id));
        await reload();
      } finally {
        setWorking(null);
      }
    });

  const importPack = () =>
    act.run(async () => {
      const path = await open({ multiple: false, directory: false, filters: [{ name: "Pack (.zip)", extensions: ["zip"] }] });
      if (typeof path === "string") {
        await api.importPack(id, tab, path);
        await reload();
      }
    });

  const isFabric = sel.loader === "fabric";
  const have = new Set(mods.map((m) => m.slug).filter(Boolean));

  return (
    <div className="page">
      <PageHead title="Mods" text={`Managing ${sel.name}. Mods are only installed when they are published for Minecraft ${sel.mcVersion} on Fabric and match your Java and Fabric Loader versions.`} />
      <div className="tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} role="tab" aria-selected={tab === t.id} className={tab === t.id ? "active" : ""} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      {act.error && <Notice kind="error">{act.error}</Notice>}

      {tab === "mods" && (
        <>
          {!isFabric && <Notice kind="warn">This is a Vanilla instance. Create a Fabric instance on the Versions page to use mods.</Notice>}
          <Panel title="Optimization mods" aside={<Button variant="primary" small disabled={!isFabric || !sel.installed} busy={working === "profile"} onClick={installProfile}>Install all for this profile</Button>}>
            <table>
              <tbody>
                {catalog.map(([s, name]) => (
                  <tr key={s}>
                    <td><b>{name}</b></td>
                    <td className="muted small">modrinth.com/mod/{s}</td>
                    <td style={{ textAlign: "right" }}>
                      {have.has(s) ? <span className="badge good">Installed</span> : <Button small disabled={!isFabric || !sel.installed} busy={working === s} onClick={() => install(s)}>Check and install</Button>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {reports.length > 0 && (
              <div className="stack">
                {reports.map((r, i) => (
                  <Notice key={i} kind={r.status === "installed" || r.status === "already" ? "good" : r.status === "incompatible" ? "warn" : "error"}>
                    <b>{r.slug}</b>: {r.detail}
                  </Notice>
                ))}
              </div>
            )}
          </Panel>

          <Panel title="Install from Modrinth">
            <div className="row">
              <input type="text" style={{ maxWidth: 320 }} value={slug} placeholder="Project slug, for example fabric-api" onChange={(e) => setSlug(e.target.value)} />
              <Button disabled={!isFabric || !sel.installed || !slug.trim()} busy={working === slug.trim().toLowerCase()} onClick={() => install(slug)}>Check and install</Button>
            </div>
          </Panel>

          <Panel title={`Installed mods (${mods.length})`}>
            {mods.length === 0 && <p className="muted">No mods in this instance.</p>}
            <table>
              <tbody>
                {mods.map((m) => (
                  <tr key={m.file}>
                    <td><b>{m.title}</b> <span className="muted small">{m.version}</span></td>
                    <td className="muted small">{m.managed ? "Installed by SAYNAY" : "Added manually"}</td>
                    <td style={{ textAlign: "right" }}>
                      <Button small variant="danger" onClick={() => act.run(async () => { await api.removeMod(id, m.file); await reload(); })}>Remove</Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Panel>
        </>
      )}

      {tab !== "mods" && (
        <Panel title={tab === "resourcepacks" ? "Resource packs" : "Shader packs"} aside={<Button variant="primary" small onClick={importPack}>Import .zip</Button>}>
          {tab === "shaderpacks" && <p className="muted small">Shader packs need a shader mod such as Iris. SAYNAY does not install one automatically because it conflicts with some setups.</p>}
          {packs.length === 0 && <p className="muted">Nothing here yet.</p>}
          <table>
            <tbody>
              {packs.map((p) => (
                <tr key={p.name}>
                  <td><b>{p.name}</b></td>
                  <td className="muted small">{p.isDir ? "Folder" : `${p.sizeKb} KB`}</td>
                  <td style={{ textAlign: "right" }}>
                    <Button small variant="danger" onClick={() => act.run(async () => { await api.removePack(id, tab, p.name); await reload(); })}>Remove</Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Panel>
      )}
    </div>
  );
}
