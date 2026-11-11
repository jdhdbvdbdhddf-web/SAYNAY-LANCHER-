use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::{
    error::*,
    java, logger,
    minecraft::{version, Instance},
    net, paths,
    sources::Sources,
};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub instance_id: String,
    pub stage: String,
    pub done: usize,
    pub total: usize,
}

pub fn progress(app: &AppHandle, instance_id: &str, stage: &str, done: usize, total: usize) {
    let _ = app.emit(
        "install-progress",
        Progress { instance_id: instance_id.into(), stage: stage.into(), done, total },
    );
}

// ---------- official metadata (Mojang) with a safe on-disk cache ----------

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
    pub release_time: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Manifest {
    pub versions: Vec<ManifestVersion>,
}

#[derive(Deserialize, Serialize)]
struct CacheFile {
    fetched_at: i64,
    manifest: Manifest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionList {
    pub versions: Vec<ManifestVersion>,
    pub from_cache: bool,
    pub fetched_at: i64,
    pub warning: Option<String>,
    pub installed: Vec<String>,
}

fn cache_file() -> PathBuf {
    paths::cache().join("version_manifest.json")
}

fn read_cache() -> Option<CacheFile> {
    serde_json::from_slice(&std::fs::read(cache_file()).ok()?).ok()
}

/// Fresh data when possible; the saved copy (with a clear warning) when the network or Mojang is unavailable.
pub async fn manifest(http: &reqwest::Client, src: &Sources, refresh: bool) -> Result<(Manifest, bool, i64, Option<String>)> {
    let now = chrono::Utc::now().timestamp();
    let cached = read_cache();
    if !refresh {
        if let Some(c) = &cached {
            if now - c.fetched_at < 600 {
                return Ok((c.manifest.clone(), true, c.fetched_at, None));
            }
        }
    }
    let fetched = async {
        let v = net::get_json(http, &src.mojang_manifest).await?;
        let m: Manifest = serde_json::from_value(v).map_err(|_| AppError::Msg("Mojang's version list had an unexpected format.".into()))?;
        if m.versions.is_empty() {
            return fail("Mojang's version list was empty.");
        }
        Ok::<Manifest, AppError>(m)
    }
    .await;
    match fetched {
        Ok(m) => {
            let _ = std::fs::create_dir_all(paths::cache());
            let _ = std::fs::write(cache_file(), serde_json::to_vec(&CacheFile { fetched_at: now, manifest: m.clone() }).unwrap_or_default());
            Ok((m, false, now, None))
        }
        Err(e) => {
            logger::error(&format!("Version list refresh failed: {e}"));
            match cached {
                Some(c) => Ok((c.manifest, true, c.fetched_at, Some(format!("Could not refresh the version list ({e}). Showing the copy saved earlier.")))),
                None => Err(e),
            }
        }
    }
}

pub fn installed_ids() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(paths::versions()) {
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if let Ok(r) = version::resolve(&id) {
                if version::client_jar(&r.jar_id).is_file() {
                    v.push(id);
                }
            }
        }
    }
    v.sort();
    v
}

fn seg(s: &str) -> Result<String> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | ' ')) {
        return fail(format!("Unexpected version text: {s}"));
    }
    Ok(s.replace(' ', "%20"))
}

// ---------- verified downloads ----------

fn sha1_file(p: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha1::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

async fn try_download(http: &reqwest::Client, url: &str, tmp: &Path, sha1: Option<&str>) -> Result<()> {
    let mut resp = http.get(url).send().await?.error_for_status()?;
    let mut f = tokio::fs::File::create(tmp).await?;
    let mut h = Sha1::new();
    while let Some(c) = resp.chunk().await? {
        h.update(&c);
        f.write_all(&c).await?;
    }
    f.flush().await?;
    if let Some(want) = sha1 {
        let got = hex::encode(h.finalize());
        if !got.eq_ignore_ascii_case(want) {
            return fail(format!("Checksum mismatch for {url}"));
        }
    }
    Ok(())
}

/// Skips files that already exist and match their hash. Downloads to `.part`, verifies, then renames.
pub async fn download_file(http: &reqwest::Client, url: &str, dest: &Path, sha1: Option<&str>) -> Result<()> {
    if dest.is_file() {
        match sha1 {
            Some(h) => {
                if sha1_file(dest).map(|s| s.eq_ignore_ascii_case(h)).unwrap_or(false) {
                    return Ok(());
                }
            }
            None => return Ok(()),
        }
    }
    if let Some(p) = dest.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = PathBuf::from(tmp);
    let mut last: Option<AppError> = None;
    for attempt in 0..3u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(600 * attempt)).await;
        }
        match try_download(http, url, &tmp, sha1).await {
            Ok(()) => {
                tokio::fs::rename(&tmp, dest).await?;
                return Ok(());
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or_else(|| AppError::Msg("download failed".into())))
}

struct Job {
    url: String,
    dest: PathBuf,
    sha1: Option<String>,
}

async fn run_downloads(app: &AppHandle, http: &reqwest::Client, inst_id: &str, stage: &str, jobs: Vec<Job>) -> Result<()> {
    let total = jobs.len();
    let done = Arc::new(AtomicUsize::new(0));
    progress(app, inst_id, stage, 0, total);
    let results: Vec<Result<()>> = stream::iter(jobs)
        .map(|j| {
            let http = http.clone();
            let done = done.clone();
            let app = app.clone();
            let inst = inst_id.to_string();
            let stage = stage.to_string();
            async move {
                let r = if j.url.is_empty() {
                    if j.dest.is_file() { Ok(()) } else { fail(format!("A required file is missing and has no download address: {}", j.dest.display())) }
                } else {
                    download_file(&http, &j.url, &j.dest, j.sha1.as_deref()).await
                };
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 20 == 0 || n == total {
                    progress(&app, &inst, &stage, n, total);
                }
                r
            }
        })
        .buffer_unordered(16)
        .collect()
        .await;
    for r in results {
        r?;
    }
    Ok(())
}

// ---------- loaders: Fabric ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoaderChoice {
    pub version: String,
    pub stable: bool,
    pub recommended: bool,
}

pub async fn fabric_loaders(http: &reqwest::Client, src: &Sources, mc: &str) -> Result<Vec<LoaderChoice>> {
    let v = net::get_json(http, &format!("{}/versions/loader/{}", src.fabric_meta, seg(mc)?)).await?;
    let arr = v.as_array().ok_or("Fabric returned unexpected data")?;
    let mut out: Vec<LoaderChoice> = arr
        .iter()
        .filter_map(|e| Some(LoaderChoice { version: e["loader"]["version"].as_str()?.to_string(), stable: e["loader"]["stable"] == true, recommended: false }))
        .collect();
    if let Some(first) = out.iter_mut().find(|l| l.stable) {
        first.recommended = true;
    }
    Ok(out)
}

async fn install_fabric_profile(http: &reqwest::Client, src: &Sources, mc: &str, loader: &str) -> Result<String> {
    let url = format!("{}/versions/loader/{}/{}/profile/json", src.fabric_meta, seg(mc)?, seg(loader)?);
    let text = net::get_text(http, &url).await.map_err(|_| AppError::Msg(format!("Fabric does not support Minecraft {mc} with loader {loader}")))?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let id = v["id"].as_str().ok_or("Fabric profile has no id")?.to_string();
    let p = version::json_path(&id);
    tokio::fs::create_dir_all(p.parent().unwrap()).await?;
    tokio::fs::write(&p, text).await?;
    Ok(id)
}

// ---------- loaders: Forge (official Maven + official installer) ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ForgeChoice {
    /// The full Maven version, for example 1.20.1-47.2.0. Stored on the instance.
    pub version: String,
    pub label: String,
    pub recommended: bool,
    pub latest: bool,
}

pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<version>") {
        rest = &rest[i + 9..];
        let Some(j) = rest.find("</version>") else { break };
        let v = rest[..j].trim();
        if !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+')) {
            out.push(v.to_string());
        }
        rest = &rest[j + 10..];
    }
    out
}

/// Only versions whose Maven id starts with the selected Minecraft version are ever offered.
pub async fn forge_versions(http: &reqwest::Client, src: &Sources, mc: &str) -> Result<Vec<ForgeChoice>> {
    let xml = net::get_text(http, &format!("{}/net/minecraftforge/forge/maven-metadata.xml", src.forge_maven)).await?;
    let prefix = format!("{mc}-");
    let mut list: Vec<String> = parse_maven_versions(&xml).into_iter().filter(|v| v.starts_with(&prefix)).collect();
    list.reverse(); // the metadata lists oldest first
    let promos = net::get_json(http, &src.forge_promos).await.ok();
    let promo = |k: &str| promos.as_ref().and_then(|p| p["promos"][format!("{mc}-{k}")].as_str().map(String::from));
    let (rec, lat) = (promo("recommended"), promo("latest"));
    Ok(list
        .into_iter()
        .map(|v| {
            let label = v.strip_prefix(&prefix).unwrap_or(&v).to_string();
            ForgeChoice { recommended: rec.as_deref() == Some(label.as_str()), latest: lat.as_deref() == Some(label.as_str()), label, version: v }
        })
        .collect())
}

fn version_dirs() -> BTreeSet<String> {
    std::fs::read_dir(paths::versions())
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default()
}

async fn run_forge_installer(app: &AppHandle, http: &reqwest::Client, src: &Sources, inst_id: &str, mc: &str, full: &str, java_path: &Path) -> Result<String> {
    if !full.starts_with(&format!("{mc}-")) || !full.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+')) {
        return fail("That Forge version does not belong to this Minecraft version.");
    }
    let url = format!("{}/net/minecraftforge/forge/{full}/forge-{full}-installer.jar", src.forge_maven);
    progress(app, inst_id, "Downloading the Forge installer", 0, 1);
    let sha_text = net::get_text(http, &format!("{url}.sha1")).await.map_err(|_| AppError::Msg("No installer is published for this Forge version, so it cannot be installed.".into()))?;
    let sha = sha_text.split_whitespace().next().unwrap_or("").to_lowercase();
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return fail("The Forge checksum file was not in the expected format, so the installer was not downloaded.");
    }
    let jar = paths::cache().join(format!("forge-{full}-installer.jar"));
    download_file(http, &url, &jar, Some(&sha)).await?;

    let root = paths::root();
    let lp = root.join("launcher_profiles.json");
    if !lp.exists() {
        std::fs::write(&lp, r#"{"profiles":{},"version":3}"#)?;
    }
    let before = version_dirs();
    progress(app, inst_id, "Running the official Forge installer (this can take a few minutes)", 0, 0);
    logger::info(&format!("Running Forge installer {full}"));
    let mut cmd = tokio::process::Command::new(java_path);
    cmd.arg("-jar").arg(&jar).arg("--installClient").arg(&root).current_dir(&root).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000);
    }
    let out = tokio::time::timeout(Duration::from_secs(1200), cmd.output())
        .await
        .map_err(|_| AppError::Msg("The Forge installer took longer than 20 minutes and was stopped.".into()))??;
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let tail: Vec<&str> = text.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
    if !out.status.success() {
        logger::error(&format!("Forge installer failed: {}", tail.join(" | ")));
        return fail(format!("The Forge installer failed (exit code {:?}). Last lines: {}", out.status.code(), tail.join(" ")));
    }

    let label = full.strip_prefix(&format!("{mc}-")).unwrap_or(full);
    let mut candidates: Vec<String> = version_dirs().difference(&before).cloned().collect();
    candidates.extend(version_dirs().into_iter().filter(|d| d.to_lowercase().contains("forge") && d.contains(label)));
    for id in candidates {
        if let Ok(v) = version::load(&id) {
            if v.inherits_from.as_deref() == Some(mc) || v.jar.as_deref() == Some(mc) || id.contains(label) {
                return Ok(id);
            }
        }
    }
    fail("The Forge installer finished but no Forge version was created. Check logs/launcher.log.")
}

// ---------- natives and assets ----------

fn extract_natives(jar: &Path, dest: &Path, exclude: &[String]) -> Result<()> {
    let f = std::fs::File::open(jar)?;
    let mut ar = zip::ZipArchive::new(f)?;
    for i in 0..ar.len() {
        let mut e = ar.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let Some(name) = e.enclosed_name() else { continue };
        let name_s = name.to_string_lossy().replace('\\', "/");
        if exclude.iter().any(|x| name_s.starts_with(x.as_str())) {
            continue;
        }
        if ![".dll", ".so", ".dylib", ".jnilib"].iter().any(|ext| name_s.ends_with(ext)) {
            continue;
        }
        let Some(file_name) = name.file_name() else { continue };
        let mut out = std::fs::File::create(dest.join(file_name))?;
        std::io::copy(&mut e, &mut out)?;
    }
    Ok(())
}

/// Versions before 1.7 read assets from a plain folder tree instead of the hashed object store.
fn link_legacy_assets(objs: &serde_json::Map<String, serde_json::Value>, index_id: &str, to_resources: bool, game_dir: &Path) -> Result<()> {
    for (name, o) in objs {
        let Some(h) = o["hash"].as_str().filter(|h| h.len() >= 2) else { continue };
        let rel = Path::new(name);
        if !rel.components().all(|c| matches!(c, std::path::Component::Normal(_))) {
            continue;
        }
        let from = paths::assets().join("objects").join(&h[..2]).join(h);
        let to = if to_resources { game_dir.join("resources").join(rel) } else { paths::assets().join("virtual").join(index_id).join(rel) };
        if !to.exists() {
            if let Some(p) = to.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

async fn install_assets(app: &AppHandle, http: &reqwest::Client, id: &str, r: &version::Resolved, game_dir: &Path) -> Result<()> {
    let ai = r.v.asset_index.as_ref().ok_or("This version has no asset index")?;
    let idx_path = paths::assets().join("indexes").join(format!("{}.json", ai.id));
    download_file(http, &ai.url, &idx_path, Some(&ai.sha1)).await?;
    let idx: serde_json::Value = serde_json::from_slice(&std::fs::read(&idx_path)?)?;
    let objs = idx["objects"].as_object().ok_or("Malformed asset index")?.clone();
    let hashes: BTreeSet<String> = objs.values().filter_map(|o| o["hash"].as_str().map(String::from)).filter(|h| h.len() >= 2).collect();
    let jobs: Vec<Job> = hashes
        .into_iter()
        .map(|h| {
            let p = h[..2].to_string();
            Job { url: format!("https://resources.download.minecraft.net/{p}/{h}"), dest: paths::assets().join("objects").join(&p).join(&h), sha1: Some(h) }
        })
        .collect();
    run_downloads(app, http, id, "Downloading assets", jobs).await?;

    let (virt, to_res) = (idx["virtual"] == true, idx["map_to_resources"] == true);
    if virt || to_res {
        let (aid, gd) = (ai.id.clone(), game_dir.to_path_buf());
        tokio::task::spawn_blocking(move || link_legacy_assets(&objs, &aid, to_res, &gd)).await.map_err(|e| AppError::Msg(e.to_string()))??;
    }
    Ok(())
}

/// Client jar, libraries, natives and assets for one resolved version. Safe to repeat: verified files are skipped.
async fn install_files(app: &AppHandle, http: &reqwest::Client, inst_id: &str, version_id: &str, game_dir: &Path) -> Result<()> {
    let r = version::resolve(version_id)?;
    let jar = version::client_jar(&r.jar_id);
    match r.v.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        Some(client) => {
            progress(app, inst_id, "Downloading Minecraft", 0, 1);
            download_file(http, &client.url, &jar, Some(&client.sha1)).await?;
        }
        None if jar.is_file() => {}
        None => return fail("This version has no client download and the base game is not installed."),
    }

    let libs = version::plan_libraries(&r.v);
    let jobs: Vec<Job> = libs.iter().map(|l| Job { url: l.url.clone(), dest: paths::libraries().join(&l.path), sha1: l.sha1.clone() }).collect();
    run_downloads(app, http, inst_id, "Downloading libraries", jobs).await?;

    progress(app, inst_id, "Preparing native libraries", 0, 0);
    let nd = version::natives_dir(&r.jar_id);
    let _ = std::fs::remove_dir_all(&nd);
    std::fs::create_dir_all(&nd)?;
    for l in libs.iter().filter(|l| l.extract_natives) {
        extract_natives(&paths::libraries().join(&l.path), &nd, &l.exclude)?;
    }
    install_assets(app, http, inst_id, &r, game_dir).await
}

// ---------- the whole install ----------

pub async fn install(app: &AppHandle, http: &reqwest::Client, src: &Sources, inst: &mut Instance) -> Result<()> {
    let id = inst.id.clone();
    let mc = inst.mc_version.clone();
    let game_dir = inst.game_path();
    logger::info(&format!("Installing {} ({} {})", inst.name, mc, inst.loader));
    progress(app, &id, "Reading version metadata", 0, 0);
    let (man, _, _, _) = manifest(http, src, false).await?;
    let entry = man.versions.iter().find(|v| v.id == mc).ok_or_else(|| AppError::Msg(format!("Minecraft {mc} is not in Mojang's version list")))?;
    download_file(http, &entry.url, &version::json_path(&entry.id), Some(&entry.sha1)).await?;

    match inst.loader.as_str() {
        "fabric" => {
            let lv = match &inst.loader_version {
                Some(v) => v.clone(),
                None => fabric_loaders(http, src, &mc).await?.into_iter().find(|l| l.recommended).map(|l| l.version).ok_or_else(|| AppError::Msg(format!("Fabric does not support Minecraft {mc}")))?,
            };
            inst.version_id = install_fabric_profile(http, src, &mc, &lv).await?;
            inst.loader_version = Some(lv);
            install_files(app, http, &id, &inst.version_id, &game_dir).await?;
        }
        "forge" => {
            let full = inst.loader_version.clone().ok_or("Choose a Forge version first.")?;
            install_files(app, http, &id, &mc, &game_dir).await?; // the installer needs the base game on disk
            let major = version::resolve(&mc)?.v.java_version.map(|j| j.major_version).unwrap_or(8);
            let java = java::ensure(app, http, major).await?;
            inst.version_id = run_forge_installer(app, http, src, &id, &mc, &full, Path::new(&java.path)).await?;
            install_files(app, http, &id, &inst.version_id, &game_dir).await?;
        }
        _ => {
            inst.version_id = mc.clone();
            install_files(app, http, &id, &mc, &game_dir).await?;
        }
    }
    progress(app, &id, "Done", 1, 1);
    logger::info(&format!("Installed {} as {}", inst.name, inst.version_id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_metadata_parsing() {
        let xml = "<metadata><versioning><latest>1.20.1-47.3.0</latest><versions><version>1.7.10-10.13.4.1614-1.7.10</version><version>1.20.1-47.2.0</version><version>bad version!</version></versions></versioning></metadata>";
        let v = parse_maven_versions(xml);
        assert_eq!(v, vec!["1.7.10-10.13.4.1614-1.7.10", "1.20.1-47.2.0"]);
    }

    #[test]
    fn version_segments_are_validated() {
        assert_eq!(seg("1.14 Pre-Release 1").unwrap(), "1.14%20Pre-Release%201");
        assert!(seg("../etc").is_err());
    }
}
