import Logo from "../components/Logo";
import { Page } from "../api";
import { useApp } from "../hooks/AppContext";

const ITEMS: { id: Page; label: string }[] = [
  { id: "home", label: "Home" },
  { id: "play", label: "Play" },
  { id: "versions", label: "Versions" },
  { id: "mods", label: "Mods" },
  { id: "performance", label: "Performance" },
  { id: "benchmark", label: "Benchmark" },
  { id: "settings", label: "Settings" },
  { id: "about", label: "About SAYNAY" },
];

export default function Sidebar() {
  const { page, go, account, running } = useApp();
  return (
    <aside className="sidebar">
      <div className="brand">
        <Logo />
        <div>
          <b>SAYNAY</b>
          <span>Launcher</span>
        </div>
      </div>
      <nav className="nav" aria-label="Main">
        {ITEMS.map((i) => (
          <button key={i.id} className={page === i.id ? "active" : ""} onClick={() => go(i.id)} aria-current={page === i.id ? "page" : undefined}>
            <span>{i.label}</span>
          </button>
        ))}
      </nav>
      <div className="spacer" />
      <div className="account-chip">
        <div className="name">
          <span className={`dot ${running.size ? "on" : ""}`} />
          {account ? account.name : "Not signed in"}
        </div>
        <div className="muted small">{running.size ? "Minecraft is running" : account ? "Ready to play" : "Sign in on the Play page"}</div>
      </div>
    </aside>
  );
}
