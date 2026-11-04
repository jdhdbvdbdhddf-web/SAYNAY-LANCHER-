import { Page } from "./api";
import { AppProvider, useApp } from "./hooks/AppContext";
import Sidebar from "./layouts/Sidebar";
import About from "./pages/About";
import Benchmark from "./pages/Benchmark";
import Home from "./pages/Home";
import Mods from "./pages/Mods";
import Performance from "./pages/Performance";
import Play from "./pages/Play";
import Settings from "./pages/Settings";
import Versions from "./pages/Versions";

const VIEWS: Record<Page, () => JSX.Element> = {
  home: Home,
  play: Play,
  versions: Versions,
  mods: Mods,
  performance: Performance,
  benchmark: Benchmark,
  settings: Settings,
  about: About,
};

function Shell() {
  const { page } = useApp();
  const View = VIEWS[page];
  return (
    <div className="app">
      <Sidebar />
      <main className="main">
        <View />
      </main>
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <Shell />
    </AppProvider>
  );
}
