import { useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Account, api, DeviceCodeInfo } from "../api";
import { Button, Notice, PageHead, Panel } from "../components/ui";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";
import { useEvent } from "../hooks/useEvent";

export default function Accounts() {
  const c = useApp();
  const [code, setCode] = useState<DeviceCodeInfo | null>(null);
  const [authErr, setAuthErr] = useState<string | null>(null);
  const [offlineName, setOfflineName] = useState("");
  const ms = useAction();
  const off = useAction();
  const list = useAction();

  useEvent<Account>("auth-complete", () => {
    setCode(null);
    setAuthErr(null);
  });
  useEvent<string>("auth-error", (m) => {
    setCode(null);
    setAuthErr(m);
  });

  const login = () =>
    ms.run(async () => {
      setAuthErr(null);
      const d = await api.loginStart();
      setCode(d);
      try {
        await openUrl(d.verificationUri);
      } catch {
        /* the address and code are shown below either way */
      }
    });

  const addOffline = () =>
    off.run(async () => {
      await api.addOfflineAccount(offlineName);
      setOfflineName("");
      await c.refreshAccounts();
    });

  const remove = (a: Account) =>
    list.run(async () => {
      const yes = await ask(`Remove ${a.name} from SAYNAY? ${a.kind === "microsoft" ? "The saved Microsoft login is deleted from your system keychain." : ""}`, { title: "Remove account", kind: "warning" });
      if (!yes) return;
      await api.removeAccount(a.id);
      await c.refreshAccounts();
    });

  const needsClientId = (ms.error ?? authErr ?? "").includes("client ID");

  return (
    <div className="page">
      <PageHead title="Accounts" text="Microsoft accounts are the real thing. Offline accounts are local test accounts and are kept clearly separate." />

      <div className="grid two">
        <Panel title="Microsoft account">
          <p className="muted">
            Official Microsoft sign-in. You enter a short code on microsoft.com. SAYNAY never sees your password, and the login is stored in your system keychain.
          </p>
          {code ? (
            <div className="stack">
              <p>
                Open <b className="selectable">{code.verificationUri}</b> and enter this code:
              </p>
              <div className="code-box selectable">{code.userCode}</div>
              <div className="row">
                <Button small onClick={() => openUrl(code.verificationUri).catch(() => undefined)}>Open the page again</Button>
                <span className="muted small">Waiting for you to finish signing in...</span>
              </div>
            </div>
          ) : (
            <div>
              <Button variant="primary" onClick={login} busy={ms.busy}>Login with Microsoft</Button>
            </div>
          )}
          {(ms.error || authErr) && (
            <Notice kind="error">
              {ms.error ?? authErr}
              {needsClientId && (
                <div className="row" style={{ marginTop: 8 }}>
                  <Button small onClick={() => c.go("settings")}>Open Settings</Button>
                </div>
              )}
            </Notice>
          )}
        </Panel>

        <Panel title="Offline account (testing only)">
          <p className="muted">
            A local test account with a name you choose. It is not a Microsoft account, it cannot join online-mode servers, and it does not unlock anything. To play online, buy Minecraft: Java Edition and sign in with Microsoft.
          </p>
          <div className="row">
            <input type="text" style={{ maxWidth: 240 }} value={offlineName} maxLength={16} placeholder="Username (3 to 16 characters)" onChange={(e) => setOfflineName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addOffline()} />
            <Button onClick={addOffline} busy={off.busy} disabled={offlineName.trim().length < 3}>Add Offline Account</Button>
          </div>
          {off.error && <Notice kind="error">{off.error}</Notice>}
        </Panel>
      </div>

      <Panel title="Saved accounts">
        {list.error && <Notice kind="error">{list.error}</Notice>}
        {c.accounts.length === 0 && <p className="muted">No accounts yet. Add one above to be able to launch Minecraft.</p>}
        <div className="stack">
          {c.accounts.map((a) => {
            const active = c.account?.id === a.id;
            return (
              <div key={a.id} className={`inst-row ${active ? "sel" : ""}`} style={{ cursor: "default" }}>
                <div>
                  <b>{a.name}</b>{" "}
                  <span className={`badge ${a.kind === "microsoft" ? "good" : "warn"}`}>{a.kind === "microsoft" ? "Microsoft" : "Offline test account"}</span>
                  <div className="muted small selectable">{a.uuid}</div>
                </div>
                <div className="row">
                  {active ? (
                    <span className="badge good">Active</span>
                  ) : (
                    <Button small onClick={() => list.run(async () => { await api.setActiveAccount(a.id); await c.refreshAccounts(); })}>Use this account</Button>
                  )}
                  <Button small variant="danger" onClick={() => remove(a)}>Remove</Button>
                </div>
              </div>
            );
          })}
        </div>
      </Panel>
    </div>
  );
}
