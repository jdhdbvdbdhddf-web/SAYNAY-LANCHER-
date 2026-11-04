use serde::Serialize;
use std::path::PathBuf;

use crate::{error::*, minecraft::Instance, mods::safe_name};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PackItem {
    pub name: String,
    pub size_kb: u64,
    pub is_dir: bool,
}

fn dir(inst: &Instance, kind: &str) -> Result<PathBuf> {
    match kind {
        "resourcepacks" | "shaderpacks" => Ok(inst.game_path().join(kind)),
        _ => fail("Unknown pack type"),
    }
}

pub fn list(inst: &Instance, kind: &str) -> Result<Vec<PackItem>> {
    let d = dir(inst, kind)?;
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let m = e.metadata()?;
            v.push(PackItem { name: e.file_name().to_string_lossy().into_owned(), size_kb: m.len() / 1024, is_dir: m.is_dir() });
        }
    }
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(v)
}

pub fn import(inst: &Instance, kind: &str, src: &str) -> Result<()> {
    let d = dir(inst, kind)?;
    let src = PathBuf::from(src);
    let name = src.file_name().and_then(|n| n.to_str()).ok_or("Invalid file")?.to_string();
    if !name.to_lowercase().ends_with(".zip") || !src.is_file() {
        return fail("Choose a .zip pack file.");
    }
    std::fs::create_dir_all(&d)?;
    std::fs::copy(&src, d.join(&name))?;
    Ok(())
}

pub fn remove(inst: &Instance, kind: &str, name: &str) -> Result<()> {
    if !safe_name(name) {
        return fail("Invalid pack name");
    }
    let p = dir(inst, kind)?.join(name);
    if p.is_dir() {
        std::fs::remove_dir_all(p)?;
    } else if p.is_file() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}
