import { invoke } from "@tauri-apps/api/core";

export type Page = "home" | "play" | "versions" | "mods" | "performance" | "benchmark" | "settings" | "about";

export interface Hardware {
  cpuName: string | null;
  physicalCores: number | null;
  threads: number;
  gpus: string[];
  ramTotalMb: number;
  ramAvailableMb: number;
  os: string;
  osVersion: string | null;
  arch: string;
  storage: { mount: string; totalMb: number; availableMb: number }[];
  dataDirFreeMb: number | null;
}
export interface Usage { cpuPct: number; ramUsedMb: number; ramTotalMb: number }

export interface GameSettings {
  ramMb: number;
  customJvmArgs: string;
  width: number;
  height: number;
  fullscreen: boolean;
  graphics: "fast" | "fancy" | "fabulous";
  renderDistance: number;
  simulationDistance: number;
  entityDistance: number;
  vsync: boolean;
  maxFps: number;
  particles: "all" | "decreased" | "minimal";
  clouds: "off" | "fast" | "fancy";
}
export interface Instance {
  id: string;
  name: string;
  mcVersion: string;
  loader: "vanilla" | "fabric";
  loaderVersion: string | null;
  versionId: string;
  installed: boolean;
  gameDir: string;
  javaPath: string | null;
  profile: string;
  settings: GameSettings;
  createdAt: number;
}
export interface ManifestVersion { id: string; kind: string; releaseTime: string }
export interface JavaInstall { path: string; version: string; major: number; managed: boolean }
export interface ProfileInfo { id: string; name: string; description: string; settings: GameSettings; ramTargetMb: number; mods: string[] }
export interface Recommendation { profileId: string; reasons: string[]; ramMb: number }
export interface Account { name: string; uuid: string }
export interface DeviceCodeInfo { userCode: string; verificationUri: string; expiresIn: number }
export interface ModEntry { file: string; title: string; version: string; slug: string | null; managed: boolean; sizeKb: number }
export interface ModReport { slug: string; status: "installed" | "already" | "incompatible" | "error"; detail: string }
export interface PackItem { name: string; sizeKb: number; isDir: boolean }
export interface InstallProgress { instanceId: string; stage: string; done: number; total: number }
export interface ExitInfo { instanceId: string; code: number | null; stopped: boolean; crashReport: string | null; logFile: string | null; error: string | null }
export interface DataPaths { root: string; instances: string; runtimes: string; backups: string; benchTools: string }
export interface UpdateInfo { current: string; latest: string; available: boolean; url: string; notes: string }
export interface BenchResult {
  id: number;
  instanceId: string;
  label: string;
  mcVersion: string;
  profile: string;
  createdAt: number;
  durationS: number;
  avgFps: number | null;
  low1Fps: number | null;
  frameMsStddev: number | null;
  frameCount: number;
  frameTimesMs: number[];
  ramAvgMb: number | null;
  ramPeakMb: number | null;
  cpuAvgPct: number | null;
  loadSecs: number | null;
  notes: string[];
}

export const api = {
  hardware: () => invoke<Hardware>("get_hardware"),
  usage: () => invoke<Usage>("get_usage"),
  paths: () => invoke<DataPaths>("get_paths"),

  versions: () => invoke<ManifestVersion[]>("list_versions"),
  instances: () => invoke<Instance[]>("list_instances"),
  createInstance: (name: string, mcVersion: string, loader: string, gameDir?: string) =>
    invoke<Instance>("create_instance", { name, mcVersion, loader, gameDir: gameDir || null }),
  deleteInstance: (instanceId: string, deleteFiles: boolean) => invoke<void>("delete_instance", { instanceId, deleteFiles }),
  installInstance: (instanceId: string) => invoke<Instance>("install_instance", { instanceId }),
  requiredJava: (instanceId: string) => invoke<number | null>("required_java", { instanceId }),
  javas: () => invoke<JavaInstall[]>("detect_javas"),
  installJava: (major: number) => invoke<JavaInstall>("install_java", { major }),

  loginStart: () => invoke<DeviceCodeInfo>("ms_login_start"),
  account: () => invoke<Account | null>("ms_account"),
  logout: () => invoke<void>("ms_logout"),
  launch: (instanceId: string) => invoke<void>("launch_game", { instanceId }),
  stop: (instanceId: string) => invoke<void>("stop_game", { instanceId }),
  running: () => invoke<string[]>("running_games"),

  profiles: () => invoke<ProfileInfo[]>("list_profiles"),
  recommend: () => invoke<Recommendation>("recommend_profile"),
  applyProfile: (instanceId: string, profileId: string) => invoke<Instance>("apply_profile", { instanceId, profileId }),
  saveSettings: (instanceId: string, settings: GameSettings, profile: string, javaPath: string | null) =>
    invoke<Instance>("save_settings", { instanceId, settings, profile, javaPath }),
  restoreDefaults: (instanceId: string) => invoke<Instance>("restore_defaults", { instanceId }),
  jvmPreview: (instanceId: string) => invoke<string[]>("jvm_preview", { instanceId }),

  perfMods: () => invoke<[string, string][]>("perf_mod_catalog"),
  mods: (instanceId: string) => invoke<ModEntry[]>("list_mods", { instanceId }),
  installMod: (instanceId: string, slug: string) => invoke<ModReport[]>("install_mod", { instanceId, slug }),
  installProfileMods: (instanceId: string) => invoke<ModReport[]>("install_profile_mods", { instanceId }),
  removeMod: (instanceId: string, file: string) => invoke<void>("remove_mod", { instanceId, file }),
  packs: (instanceId: string, kind: string) => invoke<PackItem[]>("list_packs", { instanceId, kind }),
  importPack: (instanceId: string, kind: string, path: string) => invoke<void>("import_pack", { instanceId, kind, path }),
  removePack: (instanceId: string, kind: string, name: string) => invoke<void>("remove_pack", { instanceId, kind, name }),

  runBenchmark: (instanceId: string, label: string, seconds: number) => invoke<BenchResult>("run_benchmark", { instanceId, label, seconds }),
  benchmarks: (instanceId: string | null) => invoke<BenchResult[]>("list_benchmarks", { instanceId }),
  deleteBenchmark: (id: number) => invoke<void>("delete_benchmark", { id }),

  settings: () => invoke<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
};
