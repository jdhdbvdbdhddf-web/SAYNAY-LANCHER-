# SAYNAY Launcher

A lightweight Minecraft Java Edition launcher built around real, measurable performance.
It installs and launches the **original** game from Mojang's official servers, keeps full Fabric mod compatibility,
picks safe settings for your hardware, and proves the result with a repeatable benchmark.

Rust backend, Tauri 2 shell, React and TypeScript interface, SQLite storage.

## Status: read this first

The full source for every system below is written, but **it has not been compiled or run yet**. The first job on a
machine with the toolchain is to build it, run `cargo test`, and fix whatever the compiler finds (dependency versions
are pinned to major versions and may need a small adjustment). Nothing in `tests/manual-test-matrix.md` is marked as passing.
Installers are produced by the CI workflow in `.github/workflows/build.yml`, not by hand.

Two things need action from you before the game can start:

1. **Microsoft sign-in needs an approved Azure app.** Create an app registration (personal accounts, public client flows on),
   then submit its client ID to Mojang for approval at https://aka.ms/mce-reviewappid. Until it is approved, Minecraft
   Services answers "Invalid app registration" and SAYNAY shows that message. Put the client ID in Settings, or set the
   `SAYNAY_MS_CLIENT_ID` build variable.
2. **FPS measurement needs the SAYNAY Bench mod.** Build `bench-mod/` (see its README) and place the jar in `assets/bench/`.
   Without it the benchmark still records RAM, CPU and load time, and shows FPS as n/a. It never invents a number.

## What it does

| Area | Implementation |
| --- | --- |
| Install | Official version manifest, libraries, natives, assets and client jar, all hash-verified, resumable, parallel (`minecraft/install.rs`) |
| Launch | Arguments built from the official version JSON with Mojang rule evaluation; no shell involved (`minecraft/launch.rs`) |
| Loaders | Vanilla and Fabric (official Fabric meta API) |
| Java | Detects installed runtimes by running them, picks the right major, downloads a verified Temurin JRE when needed (`java/`) |
| Sign-in | Microsoft device-code flow, Xbox Live, XSTS, Minecraft Services. Password never seen, refresh token in the OS keychain, access token never sent to the UI (`authentication/`) |
| Hardware | CPU, cores, GPU, RAM, OS, storage read from the OS; anything unreadable is shown as Unavailable (`hardware/`) |
| Profiles | Potato, Low-End, Balanced, High-End, Maximum Performance. Each writes real `options.txt` values and chooses JVM flags and a safe RAM size; the best one is recommended from your hardware (`performance/`) |
| Mods | Modrinth: Sodium, Lithium, FerriteCore, ImmediatelyFast, Entity Culling, ModernFix. Installed only if Modrinth lists your exact Minecraft version on Fabric, the SHA-512 matches, and the jar's own `fabric.mod.json` accepts your Java and Loader versions. Required dependencies are resolved first and nothing is installed if any step fails (`mods/`) |
| Packs | Resource pack and shader pack import and removal |
| Benchmark | Frame times from the companion mod, RAM and CPU sampling of the game process, load time from the game log; average FPS, 1% low, frame-time spread, history, before/after comparison (`benchmark/`) |
| Safety | options.txt backed up before the first change and on every change; worlds moved to a backup folder before an instance's files are deleted; custom game folders are never deleted; RAM limited to 90% of memory with a warning above 50% |
| Process | Game output streamed to the UI and saved to `saynay-logs/`; crash reports detected; stop button |
| Updates | Checks the latest GitHub release and links to it. In-app installation needs a signing key (see `docs/ARCHITECTURE.md`) |

## Build from source

Requirements: Node 20+, Rust stable, and the Tauri 2 system packages for your OS
(https://tauri.app/start/prerequisites/). On Linux also `libdbus-1-dev` for the keychain.

```bash
npm install
npm run icons          # generates all app icons from assets/icons/logo.svg
npm run tauri dev      # run in development
npm run tauri build    # build installers for the current OS
cargo test --manifest-path src-tauri/Cargo.toml
```

Installers: Windows EXE (NSIS) and MSI, Linux AppImage and DEB, macOS DMG (Apple Silicon and Intel).
CI builds all of them: push a tag like `v0.1.0`, then download the artifacts from the workflow run.

## Using it

1. Settings: enter your approved Azure client ID.
2. Play: sign in with Microsoft.
3. Versions: create an instance (choose Fabric to use mods) and install it. Java is downloaded automatically if needed.
4. Benchmark: record a baseline, then Performance: apply the recommended profile, Mods: install the optimization mods,
   then Benchmark again and compare. Use the same world, spot and settings for both runs.
5. Play: launch.

## Known limits

- Fabric and Vanilla only. Forge, NeoForge and Quilt are not implemented.
- Versions older than 1.8 (legacy asset layout) are refused with a clear message.
- On Apple Silicon, versions older than 1.19 ship no arm64 native libraries; they need an x64 Java running under Rosetta.
- Shader packs need a shader mod (for example Iris); SAYNAY manages the pack files but does not install the mod.
- Linux GPU detection needs `lspci`; macOS and Windows use system tools. If a tool is missing the GPU shows as Unavailable.
- Fonts: the interface uses Sora or Inter when installed and falls back to system fonts. Drop font files into `assets/fonts/` if you want to bundle them.

Minecraft is a trademark of Mojang Studios and Microsoft. SAYNAY is not affiliated with them and does not distribute any game files.
