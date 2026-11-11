use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tauri::AppHandle;

use crate::{error::*, minecraft::install::progress, paths, system};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub managed: bool,
}

/// "1.8.0_392" -> 8, "17.0.9" -> 17, "21" -> 21
pub fn parse_major(v: &str) -> Option<u32> {
    let mut it = v.split(|c: char| matches!(c, '.' | '_' | '-' | '+')).filter(|s| !s.is_empty());
    let first = it.next()?;
    if first == "1" {
        it.next()?.parse().ok()
    } else {
        first.parse().ok()
    }
}

fn java_exe() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

/// Run `java -version` and read what it really is. Never trust a folder name.
pub fn probe(path: &Path) -> Option<JavaInstall> {
    let out = system::run(path.to_str()?, &["-version"])?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    let start = text.find("version \"")? + 9;
    let rest = &text[start..];
    let version = rest[..rest.find('"')?].to_string();
    Some(JavaInstall {
        path: path.to_string_lossy().into_owned(),
        major: parse_major(&version)?,
        version,
        managed: path.starts_with(paths::runtimes()),
    })
}

pub fn find_java_bin(dir: &Path, depth: u8) -> Option<PathBuf> {
    let direct = dir.join("bin").join(java_exe());
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find_java_bin(&p, depth - 1) {
                return Some(f);
            }
        }
    }
    None
}

fn vendor_roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    {
        for r in [
            r"C:\Program Files\Java",
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\BellSoft",
            r"C:\Program Files\Amazon Corretto",
            r"C:\Program Files\Semeru",
        ] {
            v.push(PathBuf::from(r));
        }
    }
    #[cfg(target_os = "linux")]
    {
        v.push(PathBuf::from("/usr/lib/jvm"));
        if let Some(h) = dirs::home_dir() {
            v.push(h.join(".sdkman/candidates/java"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    }
    v
}

pub fn detect() -> Vec<JavaInstall> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Ok(h) = std::env::var("JAVA_HOME") {
        cands.push(Path::new(&h).join("bin").join(java_exe()));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(java_exe());
            if p.is_file() {
                cands.push(p);
            }
        }
    }
    if let Ok(rd) = std::fs::read_dir(paths::runtimes()) {
        for e in rd.flatten() {
            if let Some(j) = find_java_bin(&e.path(), 5) {
                cands.push(j);
            }
        }
    }
    for root in vendor_roots() {
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                let p = e.path();
                let c = if cfg!(target_os = "macos") {
                    p.join("Contents/Home/bin").join(java_exe())
                } else {
                    p.join("bin").join(java_exe())
                };
                if c.is_file() {
                    cands.push(c);
                }
            }
        }
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in cands {
        let key = std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
        if !seen.insert(key) {
            continue;
        }
        if let Some(j) = probe(&p) {
            out.push(j);
        }
    }
    out.sort_by_key(|j| (j.major, !j.managed));
    out
}

pub async fn detect_async() -> Result<Vec<JavaInstall>> {
    tokio::task::spawn_blocking(detect).await.map_err(|e| AppError::Msg(e.to_string()))
}

/// Minecraft 1.16 and older only run on Java 8. Newer versions run on their required major or later.
pub fn pick(major: u32, javas: &[JavaInstall]) -> Option<JavaInstall> {
    let mut c: Vec<&JavaInstall> = javas
        .iter()
        .filter(|j| if major <= 8 { j.major == 8 } else { j.major >= major })
        .collect();
    c.sort_by_key(|j| (j.major, !j.managed));
    c.first().map(|j| (*j).clone())
}

pub async fn ensure(app: &AppHandle, http: &reqwest::Client, major: u32) -> Result<JavaInstall> {
    let have = detect_async().await?;
    if let Some(j) = pick(major, &have) {
        return Ok(j);
    }
    install_managed(app, http, major).await
}

fn extract(bytes: &[u8], is_zip: bool, dest: &Path) -> Result<()> {
    if is_zip {
        let mut ar = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        ar.extract(dest)?;
    } else {
        let gz = flate2::read::GzDecoder::new(bytes);
        tar::Archive::new(gz).unpack(dest)?;
    }
    Ok(())
}

/// Reuses a managed runtime that is already on disk instead of downloading it again.
pub async fn install_managed(app: &AppHandle, http: &reqwest::Client, major: u32) -> Result<JavaInstall> {
    if let Some(bin) = find_java_bin(&paths::runtimes().join(format!("java-{major}")), 5) {
        if let Some(j) = probe(&bin) {
            return Ok(j);
        }
    }
    install(app, http, major).await
}

/// Download a Temurin JRE from the Adoptium API and verify its SHA-256 before extracting.
pub async fn install(app: &AppHandle, http: &reqwest::Client, major: u32) -> Result<JavaInstall> {
    let os = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "mac" } else { "linux" };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };
    progress(app, "", &format!("Downloading Java {major}"), 0, 0);

    let url = format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse");
    let assets: serde_json::Value = http.get(&url).send().await?.error_for_status()?.json().await?;
    let pkg = assets
        .as_array()
        .and_then(|a| a.first())
        .and_then(|a| a.get("binary"))
        .and_then(|b| b.get("package"))
        .ok_or_else(|| AppError::Msg(format!("No Java {major} runtime is published for {os}/{arch}. Install one manually and select it on the Performance page.")))?;
    let link = pkg["link"].as_str().ok_or("Adoptium response had no download link")?;
    let sum = pkg["checksum"].as_str().ok_or("Adoptium response had no checksum")?;
    let name = pkg["name"].as_str().unwrap_or("");

    let bytes = http.get(link).send().await?.error_for_status()?.bytes().await?;
    if hex::encode(Sha256::digest(&bytes)) != sum {
        return fail("The Java download failed checksum verification and was discarded.");
    }

    progress(app, "", &format!("Unpacking Java {major}"), 0, 0);
    let dest = paths::runtimes().join(format!("java-{major}"));
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(&dest)?;
    let is_zip = name.ends_with(".zip");
    let d2 = dest.clone();
    tokio::task::spawn_blocking(move || extract(&bytes, is_zip, &d2))
        .await
        .map_err(|e| AppError::Msg(e.to_string()))??;

    let bin = find_java_bin(&dest, 5).ok_or("Java was unpacked but no java executable was found")?;
    probe(&bin).ok_or_else(|| AppError::Msg("The installed Java runtime failed to start".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn majors() {
        assert_eq!(parse_major("1.8.0_392"), Some(8));
        assert_eq!(parse_major("17.0.9"), Some(17));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(parse_major("25-ea"), Some(25));
    }
    #[test]
    fn picks() {
        let j = |m| JavaInstall { path: format!("j{m}"), version: m.to_string(), major: m, managed: false };
        let all = vec![j(8), j(17), j(21)];
        assert_eq!(pick(8, &all).unwrap().major, 8);
        assert_eq!(pick(17, &all).unwrap().major, 17);
        assert_eq!(pick(21, &all).unwrap().major, 21);
        assert!(pick(25, &all).is_none());
    }
}
