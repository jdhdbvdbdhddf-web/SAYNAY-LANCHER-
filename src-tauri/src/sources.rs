//! Metadata sources. Defaults are the official ones; each can be changed in Settings (HTTPS only).

use serde::Serialize;

use crate::{db::Db, error::*, net};

pub const DEFAULTS: [(&str, &str); 4] = [
    ("src_mojang_manifest", "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"),
    ("src_fabric_meta", "https://meta.fabricmc.net/v2"),
    ("src_forge_maven", "https://maven.minecraftforge.net"),
    ("src_forge_promos", "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"),
];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sources {
    pub mojang_manifest: String,
    pub fabric_meta: String,
    pub forge_maven: String,
    pub forge_promos: String,
}

impl Sources {
    pub fn load(db: &Db) -> Sources {
        let get = |k: &str, d: &str| db.get_setting(k).ok().flatten().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| d.to_string());
        Sources {
            mojang_manifest: get(DEFAULTS[0].0, DEFAULTS[0].1),
            fabric_meta: get(DEFAULTS[1].0, DEFAULTS[1].1).trim_end_matches('/').to_string(),
            forge_maven: get(DEFAULTS[2].0, DEFAULTS[2].1).trim_end_matches('/').to_string(),
            forge_promos: get(DEFAULTS[3].0, DEFAULTS[3].1),
        }
    }
}

pub fn set(db: &Db, key: &str, value: &str) -> Result<()> {
    if !DEFAULTS.iter().any(|(k, _)| *k == key) {
        return fail("Unknown source");
    }
    let v = value.trim();
    if v.is_empty() {
        return db.set_setting(key, "");
    }
    net::ensure_https(v)?;
    db.set_setting(key, v)
}
