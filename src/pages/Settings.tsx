import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, DataPaths, SourceRow, UpdateInfo } from "../api";
import { Button, Field, Notice, PageHead, Panel } from "../components/ui";
import { useAction } from "../hooks/useAction";

const SOURCE_LABELS: Record<string, string> = {
  src_mojang_manifest: "Minecraft version list (Mojang)",
  src_fabric_meta: "Fabric metadata API",
  src_forge_maven: "Forge Maven repository",
  src_forge_promos: "Forge promotions list",
};

export default function Settings() {
  const [cfg, setCfg] = useState<Record<string, string>>({});
  const [sources, setSources] = useState<SourceRow[]>([]);
  const [paths, setPaths] = useState<DataPaths | null>(null);
  const [upd, setUpd] = useState<UpdateInfo | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [log, setLog] = useState<string | null>(null);
  const act = useAction();

  useEffect(() => {
    api.settings().then(setCfg).catch(() => undefined);
    api.sources().then(setSources).catch(() => undefined);
    api.paths().then(setPaths).catch(() => undefined);
  }, []);

  const save = (key: string) =>
    act.run(async () => {
      await api.setSetting(key, cfg[key] ?? "");
      setSaved(key);
    });

  const saveSource = (row: SourceRow, value: string) =>
    act.run(async () => {
      await api.setSource(row.key, value);
      setSources(await api.sources());
      setSaved(row.key);
    });

  const setLocal = (key: string, value: string) => setSources((s) => s.map((r) => (r.key === key ? { ...r, value } : r)));

  return (
    <div className="page">
      <PageHead title="Settings" />
      {act.error && <Notice kind="error">{act.error}</Notice>}

      <Panel title="Microsoft sign-in">
        <p className="muted">
          Minecraft sign-in needs an Azure app registration (public client, personal accounts) that Mojang has approved for Minecraft services. Enter its application (client) ID here. It is not a secret, and no client secret is ever needed.
        </p>
        <div className="row">
          <input type="text" style={{ maxWidth: 420 }} value={cfg.ms_client_id ?? ""} placeholder="00000000-0000-0000-0000-000000000000" onChange={(e) => setCfg({ ...cfg, ms_client_id: e.target.value })} />
          <Button onClick={() => save("ms_client_id")} busy={act.busy}>Save</Button>
          {saved === "ms_client_id" && <span className="badge good">Saved</span>}
        </div>
      </Panel>

      <Panel title="Metadata sources">
        <p className="muted small">The official addresses are used unless you change them. Only https:// addresses are accepted. Leave a field empty to use the default.</p>
        {sources.map((r) => (
          <Field key={r.key} label={SOURCE_LABELS[r.key] ?? r.key} hint={`Default: ${r.default}`}>
            <div className="row">
              <input type="text" value={r.value} placeholder={r.default} onChange={(e) => setLocal(r.key, e.target.value)} />
              <Button small onClick={() => saveSource(r, r.value)} busy={act.busy}>Save</Button>
              <Button small onClick={() => saveSource(r, "")} busy={act.busy}>Reset</Button>
              {saved === r.key && <span className="badge good">Saved</span>}
            </div>
          </Field>
        ))}
      </Panel>

      <Panel title="Updates">
        <Field label="GitHub repository that publishes SAYNAY releases (owner/name)">
          <div className="row">
            <input type="text" style={{ maxWidth: 360 }} value={cfg.update_repo ?? ""} placeholder="your-org/saynay-launcher" onChange={(e) => setCfg({ ...cfg, update_repo: e.target.value })} />
            <Button onClick={() => save("update_repo")} busy={act.busy}>Save</Button>
            <Button variant="primary" busy={act.busy} onClick={() => act.run(async () => { await api.setSetting("update_repo", cfg.update_repo ?? ""); setUpd(await api.checkUpdate()); })}>Check for updates</Button>
          </div>
        </Field>
        {upd && (
          <Notice kind={upd.available ? "good" : "info"}>
            {upd.available ? (
              <div className="row between">
                <span>SAYNAY {upd.latest} is available. You have {upd.current}.</span>
                <Button small variant="primary" onClick={() => openUrl(upd.url).catch(() => undefined)}>Open the release page</Button>
              </div>
            ) : (
              <span>You are on the latest version ({upd.current}).</span>
            )}
          </Notice>
        )}
      </Panel>

      <Panel title="Troubleshooting log" aside={<Button small onClick={() => act.run(async () => setLog(await api.logTail()))}>Show the latest entries</Button>}>
        <p className="muted small">Install, sign-in and launch steps are written to logs/launcher.log. Access tokens and passwords are never written to it.</p>
        {log !== null && <textarea readOnly value={log || "The log is empty."} style={{ minHeight: 220 }} />}
      </Panel>

      <Panel title="Where SAYNAY keeps things">
        {paths && (
          <dl className="kv selectable">
            <dt>Data</dt><dd>{paths.root}</dd>
            <dt>Instances</dt><dd>{paths.instances}</dd>
            <dt>Java runtimes</dt><dd>{paths.runtimes}</dd>
            <dt>Backups</dt><dd>{paths.backups}</dd>
            <dt>Logs</dt><dd>{paths.root}/logs</dd>
            <dt>Benchmark tools</dt><dd>{paths.benchTools}</dd>
          </dl>
        )}
      </Panel>
    </div>
  );
}
