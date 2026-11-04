import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Account, api, DeviceCodeInfo } from "../api";
import { Button, NoInstance, Notice, PageHead, Panel } from "../components/ui";
import { profileName } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";
import { useEvent } from "../hooks/useEvent";

export default function Play() {
  const c = useApp();
  const sel = c.selected;
  const [code, setCode] = useState<DeviceCodeInfo | null>(null);
  const [authErr, setAuthErr] = useState<string | null>(null);
  const login = useAction();
  const game = useAction();
  const box = useRef<HTMLDivElement>(null);

  useEvent<Account>("auth-complete", () => {
    setCode(null);
    setAuthErr(null);
  });
  useEvent<string>("auth-error", (m) => {
    setCode(null);
    setAuthErr(m);
  });

  const lines = sel ? c.logsOf(sel.id) : [];
  useEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines.length]);

  const signIn = () =>
    login.run(async () => {
      setAuthErr(null);
      const d = await api.loginStart();
      setCode(d);
      try {
        await openUrl(d.verificationUri);
      } catch {
        /* the code and address are shown below either way */
      }
    });

  const running = sel ? c.running.has(sel.id) : false;
  const exit = sel ? c.exits[sel.id] : undefined;

  return (
    <div className="page">
      <PageHead title="Play" text="Sign in with your Microsoft account, then launch the original Minecraft Java Edition." />

      <Panel title="Microsoft account">
        {c.account ? (
          <div className="row between">
            <div>
              <b>{c.account.name}</b>
              <div className="muted small selectable">{c.account.uuid}</div>
            </div>
            <Button
              onClick={() =>
                login.run(async () => {
                  await api.logout();
                  c.setAccount(null);
                })
              }
            >
              Sign out
            </Button>
          </div>
        ) : code ? (
          <div className="stack">
            <p>
              Open <b className="selectable">{code.verificationUri}</b> and enter this code. SAYNAY never sees your password.
            </p>
            <div className="code-box selectable">{code.userCode}</div>
            <div className="row">
              <Button small onClick={() => openUrl(code.verificationUri).catch(() => undefined)}>Open the page again</Button>
              <span className="muted small">Waiting for you to finish signing in...</span>
            </div>
          </div>
        ) : (
          <div className="row between">
            <span className="muted">You are not signed in.</span>
            <Button variant="primary" onClick={signIn} busy={login.busy}>
              Sign in with Microsoft
            </Button>
          </div>
        )}
        {(login.error || authErr) && <Notice kind="error">{login.error ?? authErr}</Notice>}
      </Panel>

      {!sel ? (
        <NoInstance onCreate={() => c.go("versions")} />
      ) : (
        <>
          <Panel
            title={sel.name}
            aside={
              running ? (
                <Button variant="danger" onClick={() => game.run(() => api.stop(sel.id))}>Stop game</Button>
              ) : (
                <Button
                  variant="primary"
                  busy={game.busy}
                  disabled={!sel.installed || !c.account}
                  onClick={() => game.run(() => api.launch(sel.id))}
                >
                  Launch Minecraft
                </Button>
              )
            }
          >
            <p className="muted">
              Minecraft {sel.mcVersion}, {sel.loader === "fabric" ? "Fabric" : "Vanilla"}, profile {profileName(sel.profile)}.
              {!sel.installed && " This instance still needs to be installed on the Versions page."}
              {sel.installed && !c.account && " Sign in to launch."}
            </p>
            {game.error && <Notice kind="error">{game.error}</Notice>}
            {exit && exit.error && <Notice kind="error">{exit.error}</Notice>}
            {exit && !exit.error && exit.crashReport && (
              <Notice kind="warn">
                Minecraft crashed. Crash report: <span className="selectable">{exit.crashReport}</span>
              </Notice>
            )}
            {exit && !exit.error && !exit.crashReport && !exit.stopped && exit.code !== 0 && (
              <Notice kind="warn">
                Minecraft exited with code {String(exit.code)}. Full output: <span className="selectable">{exit.logFile ?? "not saved"}</span>
              </Notice>
            )}
            {exit && !exit.error && !exit.crashReport && (exit.stopped || exit.code === 0) && <Notice kind="good">The game closed normally.</Notice>}
          </Panel>

          <Panel title="Game log">
            <div className="console selectable" ref={box} aria-live="off">
              {lines.length === 0 ? <span className="muted">Output appears here while the game runs.</span> : lines.map((l, i) => (
                <div key={i} className={/ERROR|FATAL|Exception/.test(l) ? "err" : ""}>{l}</div>
              ))}
            </div>
          </Panel>
        </>
      )}
    </div>
  );
}
