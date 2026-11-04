import Logo from "../components/Logo";
import { PageHead, Panel } from "../components/ui";

export default function About() {
  return (
    <div className="page">
      <PageHead title="About SAYNAY" />
      <Panel>
        <div className="row" style={{ gap: 18 }}>
          <Logo size={72} />
          <div>
            <h2>SAYNAY Launcher 0.1.0</h2>
            <p className="muted">A Minecraft Java Edition launcher built around real, measurable performance.</p>
          </div>
        </div>
      </Panel>
      <Panel title="What SAYNAY is">
        <p>
          SAYNAY launches the original Minecraft Java Edition and keeps full compatibility with Fabric and its mods. It reads your hardware, picks safe memory and graphics settings, installs compatible optimization mods, and proves the result with a benchmark you can repeat.
        </p>
        <p className="muted">
          It takes inspiration from the way Horizon treats performance as a first-class goal, but it is a different thing: Horizon replaces the game engine, while SAYNAY keeps the real game and the whole modding ecosystem. No code or interface from other launchers is copied.
        </p>
      </Panel>
      <Panel title="What SAYNAY promises">
        <ul className="muted" style={{ margin: 0, paddingLeft: 18 }}>
          <li>It does not promise a fixed FPS gain. Your hardware, drivers, Minecraft version and settings decide the result.</li>
          <li>It never stores your Microsoft password. Sign-in uses Microsoft's official device-code flow, and the refresh token lives in your system keychain.</li>
          <li>It backs up options.txt before changing it and never overwrites your worlds.</li>
          <li>It downloads Minecraft only from Mojang's official servers and verifies every file hash.</li>
        </ul>
      </Panel>
      <Panel title="Credits">
        <p className="muted">Minecraft is a trademark of Mojang Studios and Microsoft. SAYNAY is not affiliated with them. Optimization mods are made by their authors and installed from Modrinth. Fabric is developed by the FabricMC team.</p>
      </Panel>
    </div>
  );
}
