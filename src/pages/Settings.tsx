import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, DataPaths, UpdateInfo } from "../api";
import { Button, Field, Notice, PageHead, Panel } from "../components/ui";
import { useAction } from "../hooks/useAction";

export default function Settings() {
  const [cfg, setCfg] = useState<Record<string, string>>({});
  const [paths, setPaths] = useState<DataPaths | null>(null);
  const [upd, setUpd] = useState<UpdateInfo | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const act = useAction();

  useEffect(() => {
    api.settings().then(setCfg).catch(() => undefined);
    api.paths().then(setPaths).catch(() => undefined);
  }, []);

  const save = (key: string) =>
    act.run(async () => {
      await api.setSetting(key, cfg[key] ?? "");
      setSaved(key);
    });

  return (
    <div className="page">
      <PageHead title="Settings" />
      {act.error && <Notice kind="error">{act.error}</Notice>}

      <Panel title="Microsoft sign-in">
        <p className="muted">
          Minecraft sign-in needs an Azure app registration (public client, personal accounts) that Mojang has approved for Minecraft services. Enter its application (client) ID here. It is not a secret.
        </p>
        <div className="row">
          <input type="text" style={{ maxWidth: 420 }} value={cfg.ms_client_id ?? ""} placeholder="00000000-0000-0000-0000-000000000000" onChange={(e) => setCfg({ ...cfg, ms_client_id: e.target.value })} />
          <Button onClick={() => save("ms_client_id")} busy={act.busy}>Save</Button>
          {saved === "ms_client_id" && <span className="badge good">Saved</span>}
        </div>
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

      <Panel title="Where SAYNAY keeps things">
        {paths && (
          <dl className="kv selectable">
            <dt>Data</dt><dd>{paths.root}</dd>
            <dt>Instances</dt><dd>{paths.instances}</dd>
            <dt>Java runtimes</dt><dd>{paths.runtimes}</dd>
            <dt>Backups</dt><dd>{paths.backups}</dd>
            <dt>Benchmark tools</dt><dd>{paths.benchTools}</dd>
          </dl>
        )}
      </Panel>
    </div>
  );
}
