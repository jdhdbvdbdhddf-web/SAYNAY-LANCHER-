# SAYNAY architecture

## Rule number one

The UI never implements launcher logic. React calls a thin command layer (`src-tauri/src/commands/mod.rs`); the engine
modules do the work. Every command returns plain-text errors that the UI shows as they are.

## Modules

```
src-tauri/src
  commands/        Tauri commands, input validation only
  minecraft/       version JSON model + rules, install pipeline, launch arguments, Instance model
  java/            detection by probing, selection, verified download
  authentication/  Microsoft -> Xbox -> XSTS -> Minecraft; keychain storage
  mods/            Modrinth installs, jar requirement checks, resource and shader packs
  performance/     profiles, recommendation, RAM sizing, JVM flags, options.txt writer with backups
  benchmark/       run orchestration, statistics, CSV parsing
  hardware/        OS hardware detection
  system/          child process manager, live usage, update check
  db.rs            SQLite: settings, instances (JSON), benchmark history (JSON)
bench-mod/         Fabric mod that records frame times
```

## Data flow: install and launch

1. `install_instance`: download the version JSON (hash from the manifest). For Fabric, fetch the profile JSON from
   meta.fabricmc.net; it `inheritsFrom` the vanilla version.
2. `version::resolve` merges child onto parent. Child libraries come first so they win duplicate keys.
3. `plan_libraries` evaluates Mojang rules for this OS and architecture, handles both native styles (classifier jars
   before 1.19, `natives-*` library entries after) and Maven-only libraries.
4. Everything downloads to `.part`, is hash-checked, then renamed. Existing verified files are skipped, so Repair is just Install again.
5. `launch::prepare` refreshes the Microsoft session, finds or installs Java, builds JVM flags, then `build` expands the version
   JSON arguments. `system::process::run` spawns Java directly (no shell), streams output, and watches for the crash report.

## Loaders

- **Fabric:** `GET {fabric_meta}/versions/loader/{mc}` lists compatible loaders; the profile JSON is fetched for the chosen one and merged onto the vanilla version.
- **Forge:** `maven-metadata.xml` lists every build; only entries starting with `{mc}-` are offered. Installation downloads
  `forge-{version}-installer.jar` and its `.sha1` from the Maven, verifies the jar, runs `java -jar installer --installClient <SAYNAY data folder>`
  (the data folder already has the `versions/` and `libraries/` layout the installer expects), then finds the version it created and
  runs the normal file installation on it. Libraries the installer generated have no download URL, so they are only checked for presence and hash.
- `create_instance` re-checks the chosen loader version against the same metadata, so an incompatible version cannot be installed by calling the backend directly.

## Metadata and caching

All metadata requests go through `net.rs` (HTTPS only, rate limit messages). Mojang's manifest is cached in `cache/version_manifest.json`
for 10 minutes; if a refresh fails, the saved copy is used and the UI says so. Source addresses live in `sources.rs` and Settings.

## Accounts

Accounts are rows in SQLite (id, kind, name, uuid) with no secrets. A Microsoft account's refresh token is a separate keychain entry named after its UUID.
The active account id is a setting. Offline sessions are created on demand with the access token `0` and the vanilla offline UUID.

## Authentication

Device-code flow, so no embedded browser and no password handling. The Microsoft refresh token is stored in the OS keychain
(`keyring`). The Minecraft access token lives only in `AppState` and is `#[serde(skip)]`, so it cannot reach the UI.
Minecraft Services requires the client ID to be approved by Mojang; the error is detected and explained.

## Performance engine

- Recommendation uses installed RAM, thread count and GPU class. If the GPU cannot be detected it is not used.
- RAM target per profile is capped at half of installed memory and at free memory minus 1 GB (never below 1 GB), rounded to 256 MB.
- JVM flags are conservative and widely used: Serial GC on very small systems, G1 with a 50 ms pause target elsewhere.
  Users can replace them with their own arguments; an empty field means "use the recommended flags".
- `options.txt` keys are updated in place and all other lines are preserved. The first original is kept forever as
  `backups/<instance>/options.txt.original`; every write also saves a timestamped copy.
- No claim of a fixed FPS gain is made anywhere. The Benchmark page exists to measure it.

## Benchmark method

The companion mod hooks the start of each client frame, waits 15 s after the world loads, records the chosen number of seconds,
writes `frames.csv` with its start and end timestamps, and closes the game. The launcher samples the game process every 500 ms and
keeps only the samples inside the recorded window, so RAM and CPU describe exactly the same frames as the FPS numbers.
Average FPS is 1000 divided by mean frame time. 1% low is 1000 divided by the mean of the slowest 1% of frames.
Frame-time spread is the standard deviation. The chart is bucket-averaged to at most 400 points for drawing only.

## Why this differs from Horizon

Per the project brief, Horizon is a rewritten game engine. SAYNAY keeps the original Minecraft Java Edition so the whole Fabric mod
ecosystem keeps working, and gets its gains from correct memory sizing, JVM flags, settings and proven optimization mods.
No Horizon code or interface is used. (Horizon's own source and license were not audited for this build.)

## Decisions

- **Tauri over Electron:** smaller binary and lower idle memory, which suits a launcher that sits next to a game.
- **SQLite with JSON blobs for instances and benchmarks:** the schema can grow without migrations; queries are by id only.
- **Polling only on Home:** live usage is requested every 2 s while Home is visible and stops when you leave.
- **No automatic in-app update install:** it needs a signing key. Generate one with `tauri signer generate`, add
  `tauri-plugin-updater` and the public key to `tauri.conf.json`, and publish `latest.json` with releases.
- **Atomic mod installs:** every file is verified in a temp location before any of them enters the mods folder.
