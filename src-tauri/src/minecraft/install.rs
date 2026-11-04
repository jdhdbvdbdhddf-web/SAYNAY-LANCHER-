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
    minecraft::{version, Instance},
    paths,
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

// ---------- official metadata ----------

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

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

#[derive(Deserialize)]
pub struct Manifest {
    pub versions: Vec<ManifestVersion>,
}

pub async fn manifest(http: &reqwest::Client) -> Result<Manifest> {
    Ok(http.get(MANIFEST_URL).send().await?.error_for_status()?.json().await?)
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
                let r = download_file(&http, &j.url, &j.dest, j.sha1.as_deref()).await;
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

// ---------- Fabric (official meta API) ----------

pub async fn fabric_latest_loader(http: &reqwest::Client, mc: &str) -> Result<String> {
    let v: serde_json::Value = http
        .get(format!("https://meta.fabricmc.net/v2/versions/loader/{mc}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    v.as_array()
        .and_then(|a| a.iter().find(|e| e["loader"]["stable"] == true).or(a.first()))
        .and_then(|e| e["loader"]["version"].as_str().map(String::from))
        .ok_or_else(|| AppError::Msg(format!("Fabric does not support Minecraft {mc}")))
}

async fn install_fabric_profile(http: &reqwest::Client, mc: &str, loader: &str) -> Result<String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/profile/json");
    let resp = http.get(&url).send().await?;
    if !resp.status().is_success() {
        return fail(format!("Fabric does not support Minecraft {mc} with loader {loader}"));
    }
    let text = resp.text().await?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let id = v["id"].as_str().ok_or("Fabric profile has no id")?.to_string();
    let p = version::json_path(&id);
    tokio::fs::create_dir_all(p.parent().unwrap()).await?;
    tokio::fs::write(&p, text).await?;
    Ok(id)
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

async fn install_assets(app: &AppHandle, http: &reqwest::Client, id: &str, r: &version::Resolved) -> Result<()> {
    let ai = r.v.asset_index.as_ref().ok_or("This version has no asset index")?;
    let idx_path = paths::assets().join("indexes").join(format!("{}.json", ai.id));
    download_file(http, &ai.url, &idx_path, Some(&ai.sha1)).await?;
    let idx: serde_json::Value = serde_json::from_slice(&std::fs::read(&idx_path)?)?;
    if idx["map_to_resources"] == true || idx["virtual"] == true {
        return fail("This version uses the legacy asset layout, which SAYNAY does not support. Use Minecraft 1.8 or newer.");
    }
    let objs = idx["objects"].as_object().ok_or("Malformed asset index")?;
    let hashes: BTreeSet<String> = objs.values().filter_map(|o| o["hash"].as_str().map(String::from)).filter(|h| h.len() >= 2).collect();
    let jobs: Vec<Job> = hashes
        .into_iter()
        .map(|h| {
            let p = h[..2].to_string();
            Job {
                url: format!("https://resources.download.minecraft.net/{p}/{h}"),
                dest: paths::assets().join("objects").join(&p).join(&h),
                sha1: Some(h),
            }
        })
        .collect();
    run_downloads(app, http, id, "Downloading assets", jobs).await
}

// ---------- the whole install ----------

pub async fn install(app: &AppHandle, http: &reqwest::Client, inst: &mut Instance) -> Result<()> {
    let id = inst.id.clone();
    progress(app, &id, "Reading version metadata", 0, 0);
    let man = manifest(http).await?;
    let entry = man
        .versions
        .iter()
        .find(|v| v.id == inst.mc_version)
        .ok_or_else(|| AppError::Msg(format!("Minecraft {} is not in Mojang's version list", inst.mc_version)))?;
    download_file(http, &entry.url, &version::json_path(&entry.id), Some(&entry.sha1)).await?;

    if inst.loader == "fabric" {
        let lv = match &inst.loader_version {
            Some(v) => v.clone(),
            None => fabric_latest_loader(http, &inst.mc_version).await?,
        };
        inst.version_id = install_fabric_profile(http, &inst.mc_version, &lv).await?;
        inst.loader_version = Some(lv);
    } else {
        inst.version_id = inst.mc_version.clone();
    }

    let r = version::resolve(&inst.version_id)?;
    let client = r.v.downloads.as_ref().and_then(|d| d.client.as_ref()).ok_or("This version has no client download")?;
    progress(app, &id, "Downloading Minecraft", 0, 1);
    download_file(http, &client.url, &version::client_jar(&r.jar_id), Some(&client.sha1)).await?;

    let libs = version::plan_libraries(&r.v);
    let jobs: Vec<Job> = libs
        .iter()
        .map(|l| Job { url: l.url.clone(), dest: paths::libraries().join(&l.path), sha1: l.sha1.clone() })
        .collect();
    run_downloads(app, http, &id, "Downloading libraries", jobs).await?;

    progress(app, &id, "Preparing native libraries", 0, 0);
    let nd = version::natives_dir(&r.jar_id);
    let _ = std::fs::remove_dir_all(&nd);
    std::fs::create_dir_all(&nd)?;
    for l in libs.iter().filter(|l| l.extract_natives) {
        extract_natives(&paths::libraries().join(&l.path), &nd, &l.exclude)?;
    }

    install_assets(app, http, &id, &r).await?;
    progress(app, &id, "Done", 1, 1);
    Ok(())
}
