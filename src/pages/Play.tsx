import { useEffect, useRef } from "react";
import { api } from "../api";
import { Button, NoInstance, Notice, PageHead, Panel } from "../components/ui";
import { profileName } from "../format";
import { useApp } from "../hooks/AppContext";
import { useAction } from "../hooks/useAction";

export default function Play() {
  const c = useApp();
  const sel = c.selected;
  const game = useAction();
  const box = useRef<HTMLDivElement>(null);

  const lines = sel ? c.logsOf(sel.id) : [];
  useEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines.length]);

  const running = sel ? c.running.has(sel.id) : false;
  const exit = sel ? c.exits[sel.id] : undefined;

  return (
    <div className="page">
      <PageHead title="Play" text="Launch the original Minecraft Java Edition with the active account." />

      <Panel title="Account">
        {c.account ? (
          <div className="row between">
            <div>
              <b>{c.account.name}</b>{" "}
              <span className={`badge ${c.account.kind === "microsoft" ? "good" : "warn"}`}>{c.account.kind === "microsoft" ? "Microsoft" : "Offline test account"}</span>
              {c.account.kind === "offline" && <div className="muted small">Offline accounts cannot join online-mode servers.</div>}
            </div>
            <Button onClick={() => c.go("accounts")}>Manage accounts</Button>
          </div>
        ) : (
          <div className="row between">
            <span className="muted">No account yet.</span>
            <Button variant="primary" onClick={() => c.go("accounts")}>Add an account</Button>
          </div>
        )}
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
                <Button variant="primary" busy={game.busy} disabled={!sel.installed || !c.account} onClick={() => game.run(() => api.launch(sel.id))}>
                  Launch Minecraft
                </Button>
              )
            }
          >
            <p className="muted">
              Minecraft {sel.mcVersion}, {sel.loader === "vanilla" ? "Vanilla" : sel.loader === "fabric" ? "Fabric" : "Forge"}, profile {profileName(sel.profile)}.
              {!sel.installed && " This instance still needs to be installed on the Versions page."}
              {sel.installed && !c.account && " Add an account to launch."}
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
