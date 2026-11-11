# SAYNAY Launcher

A lightweight Minecraft Java Edition launcher built around real, measurable performance.
It installs and launches the **original** game from Mojang's official servers, keeps full Fabric mod compatibility,
picks safe settings for your hardware, and proves the result with a repeatable benchmark.

Rust backend, Tauri 2 shell, React and TypeScript interface, SQLite storage.

## Status: read this first

The first version of this code compiled and produced a Windows installer on GitHub's build servers. **This update (Forge, accounts,
version manager, Java page) has not been compiled yet**, so the next build may report a compile error or two. Fix those first.
Nothing in `tests/manual-test-matrix.md` is marked as passing, and no platform other than a Windows build of the earlier version has been built.
Do not treat a feature as working until its row in that matrix has been run.

Two things need action from you before the game can start with a Microsoft account:

1. **Microsoft sign-in needs an approved Azure app.** Create an app registration (personal accounts, public client flows on),
   then submit its client ID to Mojang for approval at https://aka.ms/mce-reviewappid. Until it is approved, Minecraft
   Services answers "Invalid app registration" and SAYNAY shows that message. Put the client ID in Settings, or set the
   `SAYNAY_MS_CLIENT_ID` build variable. Without it you can still use an offline test account.
2. **FPS measurement needs the SAYNAY Bench mod.** Build `bench-mod/` (see its README) and place the jar in `assets/bench/`.
   Without it the benchmark still records RAM, CPU and load time, and shows FPS as n/a. It never invents a number.

## What it does

| Area | Implementation |
| --- | --- |
| Versions | Mojang's official manifest (cached, with a clear warning when only the saved copy is available). Search, filter by release, snapshot and older, release dates, installed markers, refresh at startup and on demand, remove downloaded versions |
| Editions | Vanilla, Fabric and Forge. Fabric loader versions come from Fabric's own API for the exact Minecraft version. Forge versions come from the official Maven metadata, filtered to that Minecraft version, with recommended and latest marked. An incompatible loader version is rejected by the backend, not only hidden |
| Install | Libraries, natives, assets and client jar, all hash-verified, retried, resumable and parallel. A second install of the same instance is refused while one is running. Pre-1.7 asset layouts are supported |
| Forge | Downloads the official installer from the official Maven, verifies it against the published SHA-1, runs it headless against SAYNAY's own game folder, then installs what it produced |
| Accounts | Microsoft (official device-code flow, refresh token in the OS keychain, one entry per account) and Offline test accounts, shown side by side, with switching and removal. Offline accounts are labelled everywhere, cannot join online-mode servers, and are never presented as Microsoft accounts |
| Java | Detects installs by running them, picks the right major for each Minecraft version, downloads a verified Temurin JRE when none fits and reuses it afterwards, per-instance selection, shows version and path |
| Profiles | Potato, Low-End, Balanced, High-End, Maximum Performance. Each writes real `options.txt` values, chooses JVM flags and a safe RAM size, and is recommended from detected hardware. Every setting has a one-line explanation and there is a restore-defaults button |
| Mods | Modrinth: Sodium, Lithium, FerriteCore, ImmediatelyFast, Entity Culling, ModernFix and any other project. Works for Fabric and Forge instances, installs only if Modrinth lists the exact version and loader and the SHA-512 matches, resolves required dependencies, shows what will be installed and asks first, can enable, disable and remove |
| Benchmark | Average, 1% low, lowest and highest FPS, frame-time spread, RAM, CPU, load time, and the test machine's hardware and Java, recorded with every run. Before/after comparison and history |
| Logs | `logs/launcher.log` for install, sign-in and launch steps. Tokens are scrubbed before anything is written, and the launch command is logged with the token value hidden |
| Sources | The four metadata addresses are configurable in Settings (HTTPS only) with a reset button |

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

1. Accounts: add an account. Login with Microsoft needs your approved client ID in Settings; Add Offline Account works for local testing.
2. Versions: select a version, choose Vanilla, Fabric or Forge (and a loader version), and install. Java is downloaded automatically if needed.
3. Benchmark: record a baseline, then Performance: apply the recommended profile, Mods: install the optimization mods,
   then Benchmark again and compare. Use the same world, spot and settings for both runs.
4. Play: launch.

## Known limits

- Vanilla, Fabric and Forge only. NeoForge and Quilt are not implemented.
- Forge runs its own official installer, which needs a network connection and can take a few minutes. Forge for Minecraft 1.13 and newer is the expected path; very old Forge versions use older installers and are untested.
- Most optimization mods are Fabric-only. On a Forge instance SAYNAY reports exactly which ones have no compatible build.
- Prism Launcher's metadata repository is not used; Mojang, Fabric and Forge are queried directly. GitHub Releases are only used to check for SAYNAY updates.
- Offline accounts are for local testing. They do not grant access to online-mode servers.
- On Apple Silicon, versions older than 1.19 ship no arm64 native libraries; they need an x64 Java running under Rosetta.
- Shader packs need a shader mod (for example Iris); SAYNAY manages the pack files but does not install the mod.
- Linux GPU detection needs `lspci`; macOS and Windows use system tools. If a tool is missing the GPU shows as Unavailable.
- Fonts: the interface uses Sora or Inter when installed and falls back to system fonts. Drop font files into `assets/fonts/` if you want to bundle them.

Minecraft is a trademark of Mojang Studios and Microsoft. SAYNAY is not affiliated with them and does not distribute any game files.
