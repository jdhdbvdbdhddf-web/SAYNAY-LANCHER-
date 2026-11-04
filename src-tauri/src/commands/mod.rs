//! Thin Tauri command layer. The UI only calls these; all real work lives in the engine modules.

use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    authentication::{self, McSession},
    benchmark::{self, BenchResult},
    error::*,
    hardware::{self, HardwareInfo},
    java::{self, JavaInstall},
    minecraft::{
        install::{self, ManifestVersion},
        launch, version, GameSettings, Instance,
    },
    mods::{self, packs::{self, PackItem}, ModEntry, ModReport},
    paths, performance,
    state::AppState,
    system::{self, process, update::{self, UpdateInfo}, Usage},
};

fn join_err(e: tokio::task::JoinError) -> AppError {
    AppError::Msg(e.to_string())
}

// ---------- hardware ----------

#[tauri::command]
pub async fn get_hardware(state: State<'_, AppState>) -> Result<HardwareInfo> {
    let hw = tokio::task::spawn_blocking(hardware::detect).await.map_err(join_err)?;
    *state.hw.lock().unwrap() = Some(hw.clone());
    Ok(hw)
}

#[tauri::command]
pub fn get_usage(state: State<'_, AppState>) -> Usage {
    system::usage(&mut state.sys.lock().unwrap())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPaths {
    pub root: String,
    pub instances: String,
    pub runtimes: String,
    pub backups: String,
    pub bench_tools: String,
}

#[tauri::command]
pub fn get_paths() -> DataPaths {
    let s = |p: PathBuf| p.to_string_lossy().into_owned();
    DataPaths { root: s(paths::root()), instances: s(paths::instances()), runtimes: s(paths::runtimes()), backups: s(paths::backups()), bench_tools: s(paths::bench_tools()) }
}

// ---------- versions and instances ----------

#[tauri::command]
pub async fn list_versions(state: State<'_, AppState>) -> Result<Vec<ManifestVersion>> {
    Ok(install::manifest(&state.http).await?.versions)
}

#[tauri::command]
pub fn list_instances(state: State<'_, AppState>) -> Result<Vec<Instance>> {
    state.db.list_instances()
}

#[tauri::command]
pub fn create_instance(state: State<'_, AppState>, name: String, mc_version: String, loader: String, game_dir: Option<String>) -> Result<Instance> {
    if !matches!(loader.as_str(), "vanilla" | "fabric") {
        return fail("Unsupported loader");
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        return fail("Give the instance a name.");
    }
    let id = Uuid::new_v4().simple().to_string();
    let dir = match game_dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => {
            let p = PathBuf::from(d);
            if !p.is_absolute() {
                return fail("The game folder must be an absolute path.");
            }
            p
        }
        None => paths::instances().join(&id),
    };
    std::fs::create_dir_all(&dir)?;
    let hw = state.hardware();
    let mut settings = GameSettings::default();
    settings.ram_mb = performance::recommend_ram(performance::find("balanced").unwrap(), &hw);
    let inst = Instance {
        id,
        name,
        version_id: mc_version.clone(),
        mc_version,
        loader,
        loader_version: None,
        installed: false,
        game_dir: dir.to_string_lossy().into_owned(),
        java_path: None,
        profile: "balanced".into(),
        settings,
        created_at: chrono::Utc::now().timestamp(),
    };
    state.db.save_instance(&inst)?;
    Ok(inst)
}

/// Removes the instance. Files are only deleted for folders SAYNAY created itself, and worlds are
/// moved to the backups folder first. Custom game folders are never deleted.
#[tauri::command]
pub fn delete_instance(state: State<'_, AppState>, instance_id: String, delete_files: bool) -> Result<()> {
    if state.games.lock().unwrap().contains_key(&instance_id) {
        return fail("Stop the game before deleting this instance.");
    }
    let inst = state.db.get_instance(&instance_id)?;
    if delete_files {
        let dir = inst.game_path();
        if dir.starts_with(paths::instances()) && dir.exists() {
            let saves = dir.join("saves");
            if saves.is_dir() {
                let dest = paths::backups().join(format!("worlds-{}-{}", inst.id, chrono::Utc::now().timestamp()));
                std::fs::create_dir_all(paths::backups())?;
                std::fs::rename(&saves, &dest)?;
            }
            std::fs::remove_dir_all(&dir)?;
        }
    }
    state.db.delete_instance(&instance_id)
}

#[tauri::command]
pub async fn install_instance(app: AppHandle, state: State<'_, AppState>, instance_id: String) -> Result<Instance> {
    let mut inst = state.db.get_instance(&instance_id)?;
    install::install(&app, &state.http, &mut inst).await?;
    let major = version::resolve(&inst.version_id)?.v.java_version.map(|j| j.major_version).unwrap_or(8);
    java::ensure(&app, &state.http, major).await?;
    inst.installed = true;
    state.db.save_instance(&inst)?;
    Ok(inst)
}

#[tauri::command]
pub fn required_java(state: State<'_, AppState>, instance_id: String) -> Result<Option<u32>> {
    let inst = state.db.get_instance(&instance_id)?;
    if !inst.installed {
        return Ok(None);
    }
    Ok(version::resolve(&inst.version_id).ok().and_then(|r| r.v.java_version).map(|j| j.major_version))
}

// ---------- java ----------

#[tauri::command]
pub async fn detect_javas() -> Result<Vec<JavaInstall>> {
    java::detect_async().await
}

#[tauri::command]
pub async fn install_java(app: AppHandle, state: State<'_, AppState>, major: u32) -> Result<JavaInstall> {
    if !(8..=40).contains(&major) {
        return fail("Unsupported Java version");
    }
    java::install(&app, &state.http, major).await
}

async fn java_major_for(state: &AppState, inst: &Instance) -> Result<u32> {
    let required = version::resolve(&inst.version_id).ok().and_then(|r| r.v.java_version).map(|j| j.major_version).unwrap_or(17);
    if let Some(p) = inst.java_path.as_deref().filter(|p| !p.is_empty()) {
        let pb = PathBuf::from(p);
        if let Some(j) = tokio::task::spawn_blocking(move || java::probe(&pb)).await.map_err(join_err)? {
            return Ok(j.major);
        }
    }
    let _ = state;
    Ok(java::pick(required, &java::detect_async().await?).map(|j| j.major).unwrap_or(required))
}

// ---------- account ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

#[tauri::command]
pub async fn ms_login_start(app: AppHandle, state: State<'_, AppState>) -> Result<DeviceCodeInfo> {
    let cid = authentication::client_id(&state.db)?;
    let dc = authentication::request_device_code(&state.http, &cid).await?;
    let info = DeviceCodeInfo { user_code: dc.user_code.clone(), verification_uri: dc.verification_uri.clone(), expires_in: dc.expires_in };
    let app2 = app.clone();
    tokio::spawn(async move {
        match authentication::complete_device_login(&app2, &cid, &dc).await {
            Ok(s) => {
                let _ = app2.emit("auth-complete", &s);
            }
            Err(e) => {
                let _ = app2.emit("auth-error", e.to_string());
            }
        }
    });
    Ok(info)
}

/// Returns the signed-in account if there is a stored login. Never returns tokens.
#[tauri::command]
pub async fn ms_account(app: AppHandle) -> Result<Option<McSession>> {
    if authentication::load_refresh().is_none() {
        return Ok(None);
    }
    Ok(authentication::ensure_session(&app).await.ok())
}

#[tauri::command]
pub async fn ms_logout(state: State<'_, AppState>) -> Result<()> {
    authentication::clear_refresh();
    *state.session.lock().await = None;
    Ok(())
}

// ---------- launching ----------

#[tauri::command]
pub async fn launch_game(app: AppHandle, state: State<'_, AppState>, instance_id: String) -> Result<()> {
    if state.games.lock().unwrap().contains_key(&instance_id) {
        return fail("This instance is already running.");
    }
    let inst = state.db.get_instance(&instance_id)?;
    let prepared = launch::prepare(&app, &inst, vec![]).await?;
    let app2 = app.clone();
    let id = instance_id.clone();
    tokio::spawn(async move {
        if let Err(e) = process::run(&app2, prepared, false).await {
            let _ = app2.emit("game-exit", json!({ "instanceId": id, "code": null, "stopped": false, "crashReport": null, "logFile": null, "error": e.to_string() }));
        }
    });
    Ok(())
}

#[tauri::command]
pub fn stop_game(state: State<'_, AppState>, instance_id: String) {
    if let Some(g) = state.games.lock().unwrap().get(&instance_id) {
        g.stop.notify_one();
    }
}

#[tauri::command]
pub fn running_games(state: State<'_, AppState>) -> Vec<String> {
    state.games.lock().unwrap().keys().cloned().collect()
}

// ---------- performance ----------

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Vec<performance::ProfileInfo> {
    performance::profile_infos(&state.hardware())
}

#[tauri::command]
pub fn recommend_profile(state: State<'_, AppState>) -> performance::Recommendation {
    performance::recommend(&state.hardware())
}

#[tauri::command]
pub fn apply_profile(state: State<'_, AppState>, instance_id: String, profile_id: String) -> Result<Instance> {
    let def = performance::find(&profile_id).ok_or("Unknown profile")?;
    let mut inst = state.db.get_instance(&instance_id)?;
    inst.profile = def.id.to_string();
    inst.settings = performance::settings_for(def, &state.hardware(), &inst.settings);
    performance::write_options(&inst)?;
    state.db.save_instance(&inst)?;
    Ok(inst)
}

fn validate(s: &GameSettings, hw: &HardwareInfo) -> Result<()> {
    let max_ram = (hw.ram_total_mb as f64 * 0.9) as u32;
    if s.ram_mb < 512 || s.ram_mb > max_ram.max(1024) {
        return fail(format!("RAM must be between 512 MB and {} MB (90% of installed memory).", max_ram.max(1024)));
    }
    let ok = (2..=64).contains(&s.render_distance)
        && (5..=32).contains(&s.simulation_distance)
        && (0.5..=5.0).contains(&s.entity_distance)
        && (10..=260).contains(&s.max_fps)
        && (320..=7680).contains(&s.width)
        && (240..=4320).contains(&s.height)
        && matches!(s.graphics.as_str(), "fast" | "fancy" | "fabulous")
        && matches!(s.particles.as_str(), "all" | "decreased" | "minimal")
        && matches!(s.clouds.as_str(), "off" | "fast" | "fancy");
    if !ok {
        return fail("One of the settings is out of its allowed range.");
    }
    Ok(())
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, instance_id: String, settings: GameSettings, profile: String, java_path: Option<String>) -> Result<Instance> {
    validate(&settings, &state.hardware())?;
    let mut inst = state.db.get_instance(&instance_id)?;
    if performance::find(&profile).is_none() {
        return fail("Unknown profile");
    }
    inst.settings = settings;
    inst.profile = profile;
    inst.java_path = java_path.filter(|p| !p.trim().is_empty());
    performance::write_options(&inst)?;
    state.db.save_instance(&inst)?;
    Ok(inst)
}

#[tauri::command]
pub fn restore_defaults(state: State<'_, AppState>, instance_id: String) -> Result<Instance> {
    let mut inst = state.db.get_instance(&instance_id)?;
    performance::restore_options(&inst)?;
    let mut s = GameSettings::default();
    s.ram_mb = performance::recommend_ram(performance::find("balanced").unwrap(), &state.hardware());
    inst.settings = s;
    inst.profile = "balanced".into();
    inst.java_path = None;
    state.db.save_instance(&inst)?;
    Ok(inst)
}

#[tauri::command]
pub fn jvm_preview(state: State<'_, AppState>, instance_id: String) -> Result<Vec<String>> {
    let inst = state.db.get_instance(&instance_id)?;
    let major = version::resolve(&inst.version_id).ok().and_then(|r| r.v.java_version).map(|j| j.major_version).unwrap_or(17);
    let mut v = vec![format!("-Xmx{}M", inst.settings.ram_mb)];
    if inst.settings.custom_jvm_args.trim().is_empty() {
        v.extend(performance::jvm_flags(&inst.profile, major, state.hardware().threads));
    } else {
        v.extend(inst.settings.custom_jvm_args.split_whitespace().map(String::from));
    }
    Ok(v)
}

// ---------- mods and packs ----------

#[tauri::command]
pub fn perf_mod_catalog() -> Vec<(String, String)> {
    mods::PERF_MODS.iter().map(|(s, n)| (s.to_string(), n.to_string())).collect()
}

#[tauri::command]
pub fn list_mods(state: State<'_, AppState>, instance_id: String) -> Result<Vec<ModEntry>> {
    Ok(mods::list(&state.db.get_instance(&instance_id)?))
}

#[tauri::command]
pub async fn install_mod(state: State<'_, AppState>, instance_id: String, slug: String) -> Result<Vec<ModReport>> {
    let inst = state.db.get_instance(&instance_id)?;
    let slug = slug.trim().to_lowercase();
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return fail("Enter a Modrinth project slug, for example sodium.");
    }
    let major = java_major_for(&state, &inst).await?;
    mods::install(&state.http, &inst, &slug, major).await
}

#[tauri::command]
pub async fn install_profile_mods(state: State<'_, AppState>, instance_id: String) -> Result<Vec<ModReport>> {
    let inst = state.db.get_instance(&instance_id)?;
    let def = performance::find(&inst.profile).ok_or("Unknown profile")?;
    let major = java_major_for(&state, &inst).await?;
    let mut all = Vec::new();
    for slug in def.mods {
        all.extend(mods::install(&state.http, &inst, slug, major).await?);
    }
    Ok(all)
}

#[tauri::command]
pub fn remove_mod(state: State<'_, AppState>, instance_id: String, file: String) -> Result<()> {
    mods::remove(&state.db.get_instance(&instance_id)?, &file)
}

#[tauri::command]
pub fn list_packs(state: State<'_, AppState>, instance_id: String, kind: String) -> Result<Vec<PackItem>> {
    packs::list(&state.db.get_instance(&instance_id)?, &kind)
}

#[tauri::command]
pub fn import_pack(state: State<'_, AppState>, instance_id: String, kind: String, path: String) -> Result<()> {
    packs::import(&state.db.get_instance(&instance_id)?, &kind, &path)
}

#[tauri::command]
pub fn remove_pack(state: State<'_, AppState>, instance_id: String, kind: String, name: String) -> Result<()> {
    packs::remove(&state.db.get_instance(&instance_id)?, &kind, &name)
}

// ---------- benchmark ----------

#[tauri::command]
pub async fn run_benchmark(app: AppHandle, state: State<'_, AppState>, instance_id: String, label: String, seconds: u32) -> Result<BenchResult> {
    if !(20..=300).contains(&seconds) {
        return fail("Benchmark length must be between 20 and 300 seconds.");
    }
    if state.games.lock().unwrap().contains_key(&instance_id) {
        return fail("This instance is already running.");
    }
    let inst = state.db.get_instance(&instance_id)?;
    let label = if label.trim().is_empty() { "Benchmark".to_string() } else { label.trim().chars().take(60).collect() };
    benchmark::run(&app, &inst, label, seconds).await
}

#[tauri::command]
pub fn list_benchmarks(state: State<'_, AppState>, instance_id: Option<String>) -> Result<Vec<BenchResult>> {
    state.db.list_benchmarks(instance_id.as_deref())
}

#[tauri::command]
pub fn delete_benchmark(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db.delete_benchmark(id)
}

// ---------- settings and updates ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, String>> {
    state.db.all_settings()
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<()> {
    if !matches!(key.as_str(), "ms_client_id" | "update_repo" | "show_snapshots") {
        return fail("Unknown setting");
    }
    state.db.set_setting(&key, value.trim())
}

#[tauri::command]
pub async fn check_update(state: State<'_, AppState>) -> Result<UpdateInfo> {
    let repo = state.db.get_setting("update_repo")?.unwrap_or_default();
    update::check(&state.http, &repo).await
}
