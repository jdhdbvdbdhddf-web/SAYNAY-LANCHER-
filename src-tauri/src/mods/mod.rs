//! Mod manager. Uses the public Modrinth API and only installs a mod when
//! (1) Modrinth lists the exact Minecraft version + Fabric loader, (2) the file hash matches, and
//! (3) the jar's own fabric.mod.json does not demand a newer Java or Fabric Loader than the instance has.

pub mod packs;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha512};
use std::{
    cmp::Ordering,
    collections::{HashSet, VecDeque},
    io::Read,
    path::{Path, PathBuf},
};

use crate::{error::*, minecraft::Instance};

pub const PERF_MODS: &[(&str, &str)] = &[
    ("sodium", "Sodium"),
    ("lithium", "Lithium"),
    ("ferrite-core", "FerriteCore"),
    ("immediatelyfast", "ImmediatelyFast"),
    ("entityculling", "Entity Culling"),
    ("modernfix", "ModernFix"),
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub version: String,
    pub file: String,
    pub sha512: String,
    pub explicit: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub file: String,
    pub title: String,
    pub version: String,
    pub slug: Option<String>,
    pub managed: bool,
    pub size_kb: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModReport {
    pub slug: String,
    /// installed | already | incompatible | error
    pub status: String,
    pub detail: String,
}

fn mods_dir(i: &Instance) -> PathBuf {
    i.game_path().join("mods")
}
fn manifest_path(i: &Instance) -> PathBuf {
    i.game_path().join("saynay-mods.json")
}

pub fn load(i: &Instance) -> Vec<InstalledMod> {
    std::fs::read(manifest_path(i)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save(i: &Instance, m: &[InstalledMod]) -> Result<()> {
    std::fs::create_dir_all(i.game_path())?;
    std::fs::write(manifest_path(i), serde_json::to_vec_pretty(m)?)?;
    Ok(())
}

pub fn safe_name(n: &str) -> bool {
    !n.is_empty() && !n.contains(['/', '\\']) && n != "." && n != ".." && !n.contains("..")
}

pub fn list(i: &Instance) -> Vec<ModEntry> {
    let managed = load(i);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(mods_dir(i)) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".jar") {
                continue;
            }
            let size_kb = e.metadata().map(|m| m.len() / 1024).unwrap_or(0);
            match managed.iter().find(|m| m.file == name) {
                Some(m) => out.push(ModEntry { file: name, title: m.title.clone(), version: m.version.clone(), slug: Some(m.slug.clone()), managed: true, size_kb }),
                None => out.push(ModEntry { title: name.trim_end_matches(".jar").to_string(), file: name, version: String::new(), slug: None, managed: false, size_kb }),
            }
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

pub fn remove(i: &Instance, file: &str) -> Result<()> {
    if !safe_name(file) || !file.ends_with(".jar") {
        return fail("Invalid mod file name");
    }
    let p = mods_dir(i).join(file);
    if p.is_file() {
        std::fs::remove_file(p)?;
    }
    let mut m = load(i);
    m.retain(|x| x.file != file);
    save(i, &m)
}

// ---------- jar requirement checks (fabric.mod.json) ----------

fn cmp_versions(a: &str, b: &str) -> Ordering {
    let seg = |s: &str| -> Vec<u64> { s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect() };
    let (x, y) = (seg(a), seg(b));
    for k in 0..x.len().max(y.len()) {
        let (p, q) = (x.get(k).copied().unwrap_or(0), y.get(k).copied().unwrap_or(0));
        if p != q {
            return p.cmp(&q);
        }
    }
    Ordering::Equal
}

fn clause_ok(c: &str, have: &str) -> bool {
    if c == "*" || c.is_empty() {
        return true;
    }
    for (op, len) in [(">=", 2), ("<=", 2), (">", 1), ("<", 1), ("=", 1), ("^", 1), ("~", 1)] {
        if let Some(rest) = c.strip_prefix(op) {
            let _ = len;
            let o = cmp_versions(have, rest);
            return match op {
                ">=" => o != Ordering::Less,
                "<=" => o != Ordering::Greater,
                ">" => o == Ordering::Greater,
                "<" => o == Ordering::Less,
                "=" => o == Ordering::Equal,
                "^" => o != Ordering::Less && cmp_versions(have, &format!("{}", rest.split('.').next().unwrap_or("0").parse::<u64>().unwrap_or(0) + 1)) == Ordering::Less,
                _ => {
                    // "~": same major.minor
                    let mut it = rest.split('.');
                    let (maj, min) = (it.next().unwrap_or("0"), it.next().unwrap_or("0"));
                    o != Ordering::Less && have.starts_with(&format!("{maj}.{min}"))
                }
            };
        }
    }
    if c.contains('x') || c.contains('*') {
        let prefix = c.split(['x', '*']).next().unwrap_or("");
        return have.starts_with(prefix);
    }
    cmp_versions(have, c) == Ordering::Equal
}

fn pred_ok(v: &Value, have: &str) -> bool {
    match v {
        Value::String(s) => s.split_whitespace().all(|c| clause_ok(c, have)),
        Value::Array(a) => a.iter().any(|x| pred_ok(x, have)),
        _ => true,
    }
}

pub fn check_requirements(jar: &Path, loader: Option<&str>, java_major: u32) -> std::result::Result<(), String> {
    let f = std::fs::File::open(jar).map_err(|e| e.to_string())?;
    let mut ar = zip::ZipArchive::new(f).map_err(|_| "not a valid jar file".to_string())?;
    let mut text = String::new();
    match ar.by_name("fabric.mod.json") {
        Ok(mut e) => {
            e.read_to_string(&mut text).map_err(|e| e.to_string())?;
        }
        Err(_) => return Err("this jar is not a Fabric mod (no fabric.mod.json)".into()),
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| "unreadable fabric.mod.json".to_string())?;
    let depends = &v["depends"];
    if let Some(j) = depends.get("java") {
        if !pred_ok(j, &java_major.to_string()) {
            return Err(format!("it requires Java {} but this instance runs Java {java_major}", j));
        }
    }
    if let (Some(l), Some(lv)) = (depends.get("fabricloader"), loader) {
        if !pred_ok(l, lv) {
            return Err(format!("it requires Fabric Loader {} but this instance has {lv}", l));
        }
    }
    Ok(())
}

// ---------- Modrinth ----------

struct Plan {
    project_id: String,
    slug: String,
    title: String,
    version: String,
    url: String,
    filename: String,
    sha512: String,
    explicit: bool,
}

async fn get_json(http: &reqwest::Client, url: &str, q: &[(&str, String)]) -> Result<Value> {
    Ok(http.get(url).query(q).send().await?.error_for_status()?.json().await?)
}

async fn resolve(http: &reqwest::Client, inst: &Instance, project: &str, explicit: bool, manifest: &[InstalledMod]) -> std::result::Result<(Option<Plan>, Vec<String>, String), String> {
    let p = get_json(http, &format!("https://api.modrinth.com/v2/project/{project}"), &[]).await.map_err(|_| format!("project \"{project}\" was not found on Modrinth"))?;
    let pid = p["id"].as_str().unwrap_or(project).to_string();
    let slug = p["slug"].as_str().unwrap_or(project).to_string();
    let title = p["title"].as_str().unwrap_or(&slug).to_string();
    if manifest.iter().any(|m| m.project_id == pid) {
        return Ok((None, vec![], slug));
    }
    let versions = get_json(
        http,
        &format!("https://api.modrinth.com/v2/project/{pid}/version"),
        &[("game_versions", format!("[\"{}\"]", inst.mc_version)), ("loaders", "[\"fabric\"]".into())],
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut list: Vec<&Value> = versions.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    list.sort_by(|a, b| b["date_published"].as_str().cmp(&a["date_published"].as_str()));
    let pick = list
        .iter()
        .find(|v| v["version_type"] == "release")
        .or_else(|| list.iter().find(|v| v["version_type"] == "beta"))
        .ok_or_else(|| format!("{title} has no release for Minecraft {} on Fabric", inst.mc_version))?;
    let files = pick["files"].as_array().ok_or("version has no files")?;
    let file = files.iter().find(|f| f["primary"] == true).or(files.first()).ok_or("version has no files")?;
    let filename = file["filename"].as_str().unwrap_or("").to_string();
    if !safe_name(&filename) || !filename.ends_with(".jar") {
        return Err("Modrinth returned an unsafe file name".into());
    }
    let deps: Vec<String> = pick["dependencies"]
        .as_array()
        .map(|a| a.iter().filter(|d| d["dependency_type"] == "required").filter_map(|d| d["project_id"].as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok((
        Some(Plan {
            project_id: pid,
            slug: slug.clone(),
            title,
            version: pick["version_number"].as_str().unwrap_or("").to_string(),
            url: file["url"].as_str().ok_or("file has no url")?.to_string(),
            filename,
            sha512: file["hashes"]["sha512"].as_str().ok_or("file has no sha512")?.to_string(),
            explicit,
        }),
        deps,
        slug,
    ))
}

/// Installs a project and its required dependencies, or installs nothing and explains why.
pub async fn install(http: &reqwest::Client, inst: &Instance, project: &str, java_major: u32) -> Result<Vec<ModReport>> {
    if inst.loader != "fabric" {
        return fail("Mods need a Fabric instance. Create one on the Versions page.");
    }
    let manifest = load(inst);
    let mut plans: Vec<Plan> = Vec::new();
    let mut reports: Vec<ModReport> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, bool)> = VecDeque::from([(project.to_string(), true)]);

    while let Some((proj, explicit)) = queue.pop_front() {
        if !seen.insert(proj.clone()) {
            continue;
        }
        match resolve(http, inst, &proj, explicit, &manifest).await {
            Ok((Some(plan), deps, _)) => {
                for d in deps {
                    queue.push_back((d, false));
                }
                plans.push(plan);
            }
            Ok((None, _, slug)) => reports.push(ModReport { slug, status: "already".into(), detail: "Already installed".into() }),
            Err(e) => {
                let who = if explicit { project.to_string() } else { format!("{project} (needs {proj})") };
                return Ok(vec![ModReport { slug: who, status: "incompatible".into(), detail: format!("Nothing was installed: {e}") }]);
            }
        }
    }

    // Download everything to temp files and verify before touching the mods folder.
    let dir = mods_dir(inst);
    std::fs::create_dir_all(&dir)?;
    let mut temps: Vec<(PathBuf, PathBuf)> = Vec::new();
    let cleanup = |t: &Vec<(PathBuf, PathBuf)>| {
        for (tmp, _) in t {
            let _ = std::fs::remove_file(tmp);
        }
    };
    for plan in &plans {
        let bytes = http.get(&plan.url).send().await?.error_for_status()?.bytes().await?;
        if !hex::encode(Sha512::digest(&bytes)).eq_ignore_ascii_case(&plan.sha512) {
            cleanup(&temps);
            return Ok(vec![ModReport { slug: plan.slug.clone(), status: "error".into(), detail: "Download failed its SHA-512 check and was discarded.".into() }]);
        }
        let tmp = dir.join(format!("{}.saynay-tmp", plan.filename));
        std::fs::write(&tmp, &bytes)?;
        temps.push((tmp.clone(), dir.join(&plan.filename)));
        if let Err(why) = check_requirements(&tmp, inst.loader_version.as_deref(), java_major) {
            cleanup(&temps);
            return Ok(vec![ModReport { slug: plan.slug.clone(), status: "incompatible".into(), detail: format!("Nothing was installed: {why}.") }]);
        }
    }

    let mut manifest = manifest;
    for (plan, (tmp, dest)) in plans.iter().zip(temps.iter()) {
        std::fs::rename(tmp, dest)?;
        manifest.push(InstalledMod {
            project_id: plan.project_id.clone(),
            slug: plan.slug.clone(),
            title: plan.title.clone(),
            version: plan.version.clone(),
            file: plan.filename.clone(),
            sha512: plan.sha512.clone(),
            explicit: plan.explicit,
        });
        reports.push(ModReport { slug: plan.slug.clone(), status: "installed".into(), detail: format!("{} {}", plan.title, plan.version) });
    }
    save(inst, &manifest)?;
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn predicates() {
        assert!(pred_ok(&json!(">=17"), "21"));
        assert!(!pred_ok(&json!(">=21"), "17"));
        assert!(pred_ok(&json!([">=21", ">=17 <19"]), "17"));
        assert!(pred_ok(&json!(">=0.15.11"), "0.16.5"));
        assert!(!pred_ok(&json!(">=0.16.5"), "0.15.11"));
        assert!(pred_ok(&json!("*"), "1"));
    }

    #[test]
    fn names() {
        assert!(safe_name("sodium-0.6.jar"));
        assert!(!safe_name("../evil.jar"));
        assert!(!safe_name("a/b.jar"));
    }
}
