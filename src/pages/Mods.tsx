import { useCallback, useEffect, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
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

  const loaderName = sel.loader === "fabric" ? "Fabric" : sel.loader === "forge" ? "Forge" : "Vanilla";
  const canMod = sel.loader !== "vanilla" && sel.installed;

  /** Shows exactly what would be installed and waits for a yes. Nothing is downloaded before that. */
  const confirmThen = async (plan: ModReport[], run: () => Promise<ModReport[]>) => {
    setReports(plan.filter((r) => r.status !== "planned"));
    const planned = plan.filter((r) => r.status === "planned");
    if (planned.length === 0) return;
    const yes = await ask(`Install these into ${sel.name}?\n\n${planned.map((r) => `- ${r.detail}`).join("\n")}\n\nEach file is checked against its published hash first, and nothing is added if any check fails.`, { title: "Install mods", kind: "info" });
    if (!yes) return;
    setReports(await run());
    await reload();
  };

  const install = (s: string) =>
    act.run(async () => {
      setWorking(s);
      try {
        await confirmThen(await api.previewMod(id, s), () => api.installMod(id, s));
      } finally {
        setWorking(null);
      }
    });

  const installProfile = () =>
    act.run(async () => {
      setWorking("profile");
      try {
        await confirmThen(await api.previewProfileMods(id), () => api.installProfileMods(id));
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

  const have = new Set(mods.map((m) => m.slug).filter(Boolean));

  return (
    <div className="page">
      <PageHead title="Mods" text={`Managing ${sel.name} (${loaderName}). A mod is only offered when Modrinth lists Minecraft ${sel.mcVersion} for this loader and its hash matches. You always confirm before anything is installed.`} />
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
          {sel.loader === "vanilla" && <Notice kind="warn">This is a Vanilla instance. Install a Fabric or Forge version on the Versions page to use mods.</Notice>}
          {sel.loader === "forge" && <Notice>Most optimization mods are Fabric-only. On Forge, SAYNAY reports exactly which ones have no compatible build for this version.</Notice>}
          <Panel title="Optimization mods" aside={<Button variant="primary" small disabled={!canMod} busy={working === "profile"} onClick={installProfile}>Install all for this profile</Button>}>
            <table>
              <tbody>
                {catalog.map(([s, name]) => (
                  <tr key={s}>
                    <td><b>{name}</b></td>
                    <td className="muted small">modrinth.com/mod/{s}</td>
                    <td style={{ textAlign: "right" }}>
                      {have.has(s) ? <span className="badge good">Installed</span> : <Button small disabled={!canMod} busy={working === s} onClick={() => install(s)}>Check and install</Button>}
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
              <Button disabled={!canMod || !slug.trim()} busy={working === slug.trim().toLowerCase()} onClick={() => install(slug)}>Check and install</Button>
            </div>
          </Panel>

          <Panel title={`Installed mods (${mods.length})`}>
            {mods.length === 0 && <p className="muted">No mods in this instance.</p>}
            <table>
              <tbody>
                {mods.map((m) => (
                  <tr key={m.file} style={{ opacity: m.enabled ? 1 : 0.55 }}>
                    <td><b>{m.title}</b> <span className="muted small">{m.version}</span></td>
                    <td className="muted small">{m.managed ? "Installed by SAYNAY" : "Added manually"}{m.enabled ? "" : ", disabled"}</td>
                    <td style={{ textAlign: "right" }}>
                      <div className="row" style={{ justifyContent: "flex-end" }}>
                        <Button small onClick={() => act.run(async () => { await api.setModEnabled(id, m.file, !m.enabled); await reload(); })}>{m.enabled ? "Disable" : "Enable"}</Button>
                        <Button small variant="danger" onClick={() => act.run(async () => { if (await ask(`Remove ${m.title}?`, { title: "Remove mod", kind: "warning" })) { await api.removeMod(id, m.file); await reload(); } })}>Remove</Button>
                      </div>
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
          {tab === "shaderpacks" && <p className="muted small">Shader packs need a shader mod such as Iris. SAYNAY manages the pack files but does not install a shader mod for you.</p>}
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
