import { useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, GameSettings, JavaInstall, ProfileInfo, Recommendation } from "../api";
import { Button, Field, NoInstance, Notice, PageHead, Panel } from "../components/ui";
import { fmtMb, profileName } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

export default function Performance() {
  const c = useApp();
  const sel = c.selected;
  const hw = c.hw;
  const [profiles, setProfiles] = useState<ProfileInfo[]>([]);
  const [rec, setRec] = useState<Recommendation | null>(null);
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  const [need, setNeed] = useState<number | null>(null);
  const [draft, setDraft] = useState<GameSettings | null>(null);
  const [profile, setProfile] = useState("balanced");
  const [javaPath, setJavaPath] = useState("");
  const [preview, setPreview] = useState<string[]>([]);
  const [saved, setSaved] = useState(false);
  const act = useAction();

  useEffect(() => {
    api.profiles().then(setProfiles).catch(() => undefined);
    api.recommend().then(setRec).catch(() => undefined);
    api.javas().then(setJavas).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!sel) return;
    setDraft(sel.settings);
    setProfile(sel.profile);
    setJavaPath(sel.javaPath ?? "");
    setSaved(false);
    api.requiredJava(sel.id).then(setNeed).catch(() => setNeed(null));
    api.jvmPreview(sel.id).then(setPreview).catch(() => undefined);
  }, [sel]);

  if (!sel || !draft) {
    return (
      <div className="page">
        <PageHead title="Performance" />
        <NoInstance onCreate={() => c.go("versions")} />
      </div>
    );
  }

  const set = <K extends keyof GameSettings>(k: K, v: GameSettings[K]) => {
    setDraft((d) => (d ? { ...d, [k]: v } : d));
    setSaved(false);
  };
  const maxRam = hw ? Math.max(2048, Math.floor((hw.ramTotalMb * 0.75) / 256) * 256) : 16384;
  const aboveHalf = hw ? draft.ramMb > hw.ramTotalMb / 2 : false;
  const haveJava = need == null || javas.some((j) => (need <= 8 ? j.major === 8 : j.major >= need));

  const apply = (id: string) =>
    act.run(async () => {
      await api.applyProfile(sel.id, id);
      await c.refresh();
    });
  const save = () =>
    act.run(async () => {
      await api.saveSettings(sel.id, draft, profile, javaPath || null);
      await c.refresh();
      setSaved(true);
    });
  const restore = () =>
    act.run(async () => {
      const yes = await ask("Restore the original Minecraft options.txt from before SAYNAY changed it and reset SAYNAY's settings for this instance?", { title: "Restore defaults", kind: "warning" });
      if (!yes) return;
      await api.restoreDefaults(sel.id);
      await c.refresh();
    });
  const installJava = () =>
    act.run(async () => {
      if (need == null) return;
      await api.installJava(need);
      setJavas(await api.javas());
    });

  return (
    <div className="page">
      <PageHead
        title="Performance"
        text={`Tuning ${sel.name}. Profiles change real game settings and JVM flags. Your original options.txt is backed up before the first change. Results depend on your hardware, drivers, Minecraft version and the settings you choose, so measure with the Benchmark page.`}
      />
      {act.error && <Notice kind="error">{act.error}</Notice>}

      {rec && (
        <Panel title={`Recommended for this PC: ${profileName(rec.profileId)}`} aside={<Button variant="primary" small disabled={sel.profile === rec.profileId} busy={act.busy} onClick={() => apply(rec.profileId)}>{sel.profile === rec.profileId ? "In use" : "Apply recommendation"}</Button>}>
          <ul className="muted" style={{ margin: 0, paddingLeft: 18 }}>
            {rec.reasons.map((r, i) => <li key={i}>{r}</li>)}
            <li>Suggested memory: {fmtMb(rec.ramMb)}, never more than half of your installed RAM.</li>
          </ul>
        </Panel>
      )}

      <div className="grid three">
        {profiles.map((p) => (
          <section key={p.id} className={`panel profile ${sel.profile === p.id ? "active" : ""} ${rec?.profileId === p.id ? "rec" : ""}`}>
            <h2>{p.name}</h2>
            <p className="muted small">{p.description}</p>
            <dl className="kv small">
              <dt>Render distance</dt><dd>{p.settings.renderDistance} chunks</dd>
              <dt>Max FPS</dt><dd>{p.settings.maxFps >= 260 ? "Unlimited" : p.settings.maxFps}</dd>
              <dt>Memory</dt><dd>{fmtMb(p.settings.ramMb)}</dd>
              <dt>Mods</dt><dd>{p.mods.length}</dd>
            </dl>
            <Button small variant={sel.profile === p.id ? "ghost" : "primary"} disabled={sel.profile === p.id} busy={act.busy} onClick={() => apply(p.id)}>
              {sel.profile === p.id ? "Active" : "Apply"}
            </Button>
          </section>
        ))}
      </div>

      <Panel title="Memory and Java">
        <Field label={`Memory for Minecraft: ${fmtMb(draft.ramMb)}`} hint="How much memory the game may use. More is not always faster: too much slows garbage collection and can make the PC swap.">
          <input type="range" min={1024} max={maxRam} step={256} value={Math.min(draft.ramMb, maxRam)} onChange={(e) => set("ramMb", Number(e.target.value))} />
        </Field>
        {aboveHalf && <Notice kind="warn">More than half of your memory can make Windows or Linux swap and lower FPS. SAYNAY recommends staying at or below {fmtMb((hw?.ramTotalMb ?? 0) / 2)}.</Notice>}
        <div className="grid two">
          <Field label="Java runtime" hint="Automatic picks the Java that fits this Minecraft version. Manage runtimes on the Java page.">
            <select value={javaPath} onChange={(e) => { setJavaPath(e.target.value); setSaved(false); }}>
              <option value="">Automatic (recommended)</option>
              {javas.map((j) => (
                <option key={j.path} value={j.path}>
                  Java {j.major} ({j.version}){j.managed ? ", managed by SAYNAY" : ""}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Java required by this instance">
            <div className="row">
              <span>{need == null ? "Known after install" : `Java ${need}${need <= 8 ? " (8 exactly)" : " or newer"}`}</span>
              {need != null && !haveJava && <Button small variant="primary" busy={act.busy} onClick={installJava}>Install Java {need}</Button>}
              {need != null && haveJava && <span className="badge good">Available</span>}
            </div>
          </Field>
        </div>
        <Field label="Custom JVM arguments (leave empty to use SAYNAY's recommended flags)" hint="Advanced. Wrong flags can stop the game from starting. Recommended flags pick a garbage collector that suits your core count.">
          <textarea value={draft.customJvmArgs} placeholder={preview.slice(1).join(" ")} onChange={(e) => set("customJvmArgs", e.target.value)} />
        </Field>
        <p className="muted small selectable">Active flags: {preview.join(" ")}</p>
      </Panel>

      <Panel title="Game settings">
        <div className="grid two">
          <Field label={`Render distance: ${draft.renderDistance} chunks`} hint="How many chunks around you are drawn. The biggest single effect on FPS.">
            <input type="range" min={2} max={32} value={draft.renderDistance} onChange={(e) => set("renderDistance", Number(e.target.value))} />
          </Field>
          <Field label={`Simulation distance: ${draft.simulationDistance} chunks`} hint="How far from you the game runs mobs, crops and redstone. Lower saves CPU.">
            <input type="range" min={5} max={32} value={draft.simulationDistance} onChange={(e) => set("simulationDistance", Number(e.target.value))} />
          </Field>
          <Field label={`Entity distance: ${draft.entityDistance.toFixed(2)}x`} hint="How far away mobs and items are still drawn. Lower helps in busy areas.">
            <input type="range" min={0.5} max={5} step={0.25} value={draft.entityDistance} onChange={(e) => set("entityDistance", Number(e.target.value))} />
          </Field>
          <Field label={`Maximum FPS: ${draft.maxFps >= 260 ? "Unlimited" : draft.maxFps}`} hint="Caps the frame rate. A lower cap saves power and heat and can reduce stutter on weaker PCs.">
            <input type="range" min={30} max={260} step={10} value={draft.maxFps} onChange={(e) => set("maxFps", Number(e.target.value))} />
          </Field>
          <Field label="Graphics quality" hint="Fast skips effects such as leaf transparency. Fabulous costs the most GPU.">
            <select value={draft.graphics} onChange={(e) => set("graphics", e.target.value as GameSettings["graphics"])}>
              <option value="fast">Fast</option>
              <option value="fancy">Fancy</option>
              <option value="fabulous">Fabulous</option>
            </select>
          </Field>
          <Field label="Particles" hint="Fewer particles means less work during fights, explosions and weather.">
            <select value={draft.particles} onChange={(e) => set("particles", e.target.value as GameSettings["particles"])}>
              <option value="all">All</option>
              <option value="decreased">Decreased</option>
              <option value="minimal">Minimal</option>
            </select>
          </Field>
          <Field label="Clouds" hint="Off is the cheapest. Fancy draws volumetric clouds.">
            <select value={draft.clouds} onChange={(e) => set("clouds", e.target.value as GameSettings["clouds"])}>
              <option value="off">Off</option>
              <option value="fast">Fast</option>
              <option value="fancy">Fancy</option>
            </select>
          </Field>
          <Field label="Window size" hint="Smaller windows render fewer pixels, which raises FPS on weak GPUs.">
            <div className="row">
              <input type="number" style={{ width: 110 }} value={draft.width} min={320} onChange={(e) => set("width", Number(e.target.value))} />
              <span>by</span>
              <input type="number" style={{ width: 110 }} value={draft.height} min={240} onChange={(e) => set("height", Number(e.target.value))} />
            </div>
          </Field>
        </div>
        <p className="hint">VSync matches frames to your monitor: no tearing, but it can lower FPS and add input delay. Fullscreen can reduce desktop compositing overhead.</p>
        <div className="row">
          <label className="check"><input type="checkbox" checked={draft.vsync} onChange={(e) => set("vsync", e.target.checked)} /> VSync</label>
          <label className="check"><input type="checkbox" checked={draft.fullscreen} onChange={(e) => set("fullscreen", e.target.checked)} /> Fullscreen</label>
        </div>
        <div className="row">
          <Button variant="primary" busy={act.busy} onClick={save}>Save and apply to the game</Button>
          <Button variant="danger" busy={act.busy} onClick={restore}>Restore defaults</Button>
          {saved && <span className="badge good">Saved</span>}
        </div>
      </Panel>
    </div>
  );
}
