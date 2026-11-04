//! SAYNAY Performance Engine: hardware-aware profile recommendation, safe RAM sizing,
//! JVM flag generation, and real options.txt changes with backups.

use serde::Serialize;
use std::collections::HashMap;

use crate::{error::*, hardware::HardwareInfo, minecraft::{GameSettings, Instance}, paths};

pub struct ProfileDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub graphics: &'static str,
    pub render: u32,
    pub sim: u32,
    pub entity: f32,
    pub max_fps: u32,
    pub particles: &'static str,
    pub clouds: &'static str,
    pub ram_target_mb: u32,
    pub mods: &'static [&'static str],
}

const CORE_MODS: &[&str] = &["sodium", "lithium", "ferrite-core", "modernfix", "entityculling"];
const ALL_MODS: &[&str] = &["sodium", "lithium", "ferrite-core", "modernfix", "entityculling", "immediatelyfast"];

pub static PROFILES: [ProfileDef; 5] = [
    ProfileDef { id: "potato", name: "Potato PC", description: "Lowest load for old or very small systems. Short view distance, minimal particles, no clouds, 60 FPS cap.", graphics: "fast", render: 6, sim: 5, entity: 0.5, max_fps: 60, particles: "minimal", clouds: "off", ram_target_mb: 2048, mods: CORE_MODS },
    ProfileDef { id: "low", name: "Low-End PC", description: "Smooth play on integrated graphics and 4-8 GB systems. Fast graphics, reduced distances.", graphics: "fast", render: 8, sim: 6, entity: 0.75, max_fps: 90, particles: "decreased", clouds: "off", ram_target_mb: 3072, mods: CORE_MODS },
    ProfileDef { id: "balanced", name: "Balanced PC", description: "A sensible middle for most laptops and desktops. Fancy graphics at a moderate view distance.", graphics: "fancy", render: 12, sim: 8, entity: 1.0, max_fps: 144, particles: "decreased", clouds: "fast", ram_target_mb: 4096, mods: ALL_MODS },
    ProfileDef { id: "high", name: "High-End PC", description: "For strong CPUs and a dedicated GPU. Longer view distance and a high FPS cap.", graphics: "fancy", render: 16, sim: 10, entity: 1.25, max_fps: 240, particles: "all", clouds: "fast", ram_target_mb: 6144, mods: ALL_MODS },
    ProfileDef { id: "max", name: "Maximum Performance", description: "Built for top-tier hardware: long distances, all effects, uncapped FPS and every compatible optimization mod.", graphics: "fancy", render: 20, sim: 12, entity: 1.5, max_fps: 260, particles: "all", clouds: "fancy", ram_target_mb: 8192, mods: ALL_MODS },
];

pub fn find(id: &str) -> Option<&'static ProfileDef> {
    PROFILES.iter().find(|p| p.id == id)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub settings: GameSettings,
    pub ram_target_mb: u32,
    pub mods: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub profile_id: String,
    pub reasons: Vec<String>,
    pub ram_mb: u32,
}

/// Safe RAM: never more than half of installed memory, never more than what is free minus 1 GB headroom
/// (but never below 1 GB), rounded to 256 MB.
pub fn recommend_ram(def: &ProfileDef, hw: &HardwareInfo) -> u32 {
    let cap = (hw.ram_total_mb / 2) as u32;
    let free_cap = hw.ram_available_mb.saturating_sub(1024) as u32;
    let r = def.ram_target_mb.min(cap).min(free_cap.max(1024));
    ((r / 256) * 256).max(1024)
}

pub fn settings_for(def: &ProfileDef, hw: &HardwareInfo, base: &GameSettings) -> GameSettings {
    GameSettings {
        ram_mb: recommend_ram(def, hw),
        graphics: def.graphics.into(),
        render_distance: def.render,
        simulation_distance: def.sim,
        entity_distance: def.entity,
        vsync: false,
        max_fps: def.max_fps,
        particles: def.particles.into(),
        clouds: def.clouds.into(),
        // user-owned values survive profile changes
        width: base.width,
        height: base.height,
        fullscreen: base.fullscreen,
        custom_jvm_args: base.custom_jvm_args.clone(),
    }
}

pub fn profile_infos(hw: &HardwareInfo) -> Vec<ProfileInfo> {
    PROFILES
        .iter()
        .map(|d| ProfileInfo {
            id: d.id.into(),
            name: d.name.into(),
            description: d.description.into(),
            settings: settings_for(d, hw, &GameSettings::default()),
            ram_target_mb: d.ram_target_mb,
            mods: d.mods.iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}

fn is_capable_gpu(name: &str) -> bool {
    let l = name.to_lowercase();
    // Apple M-series GPUs are integrated but strong enough to be treated as capable.
    ["nvidia", "geforce", "rtx", "gtx", "radeon rx", "radeon pro", "arc ", "apple m"].iter().any(|k| l.contains(k))
}

pub fn recommend(hw: &HardwareInfo) -> Recommendation {
    let gb = hw.ram_total_mb as f32 / 1024.0;
    let t = hw.threads;
    let mut reasons = vec![format!("{:.0} GB RAM and {} CPU threads detected.", gb, t)];
    let gpu = if hw.gpus.is_empty() {
        reasons.push("GPU could not be detected on this system, so it was not used for the decision.".into());
        None
    } else {
        let capable = hw.gpus.iter().any(|g| is_capable_gpu(g));
        reasons.push(format!("GPU: {} ({}).", hw.gpus.join(", "), if capable { "capable" } else { "integrated or entry-level" }));
        Some(capable)
    };

    let id = if gb <= 4.5 || t <= 2 {
        "potato"
    } else if gb <= 8.5 || t <= 4 || gpu == Some(false) {
        "low"
    } else if gb < 15.5 || t < 8 {
        "balanced"
    } else if gpu == Some(true) && t >= 12 && gb >= 31.0 {
        "max"
    } else if gpu == Some(true) {
        "high"
    } else {
        "balanced"
    };
    let def = find(id).unwrap();
    Recommendation { profile_id: id.into(), reasons, ram_mb: recommend_ram(def, hw) }
}

/// Conservative, widely used flags. Serial GC on very small systems avoids extra GC threads;
/// G1 with a short pause target everywhere else. Results still depend on your hardware.
pub fn jvm_flags(profile: &str, _java_major: u32, threads: usize) -> Vec<String> {
    let mut f: Vec<&str> = Vec::new();
    if profile == "potato" || threads <= 2 {
        f.extend(["-XX:+UseSerialGC", "-XX:+PerfDisableSharedMem"]);
    } else {
        f.extend([
            "-XX:+UseG1GC",
            "-XX:+ParallelRefProcEnabled",
            "-XX:MaxGCPauseMillis=50",
            "-XX:+UnlockExperimentalVMOptions",
            "-XX:G1NewSizePercent=20",
            "-XX:G1ReservePercent=20",
            "-XX:+PerfDisableSharedMem",
        ]);
        if matches!(profile, "balanced" | "high" | "max") {
            f.push("-XX:G1HeapRegionSize=8M");
        }
    }
    f.into_iter().map(String::from).collect()
}

// ---------- options.txt with backups ----------

pub fn options_overrides(s: &GameSettings) -> Vec<(String, String)> {
    let g = match s.graphics.as_str() { "fast" => 0, "fabulous" => 2, _ => 1 };
    let p = match s.particles.as_str() { "all" => 0, "decreased" => 1, _ => 2 };
    let clouds = match s.clouds.as_str() { "off" => "\"false\"", "fast" => "\"fast\"", _ => "\"true\"" };
    vec![
        ("renderDistance".into(), s.render_distance.to_string()),
        ("simulationDistance".into(), s.simulation_distance.to_string()),
        ("entityDistanceScaling".into(), format!("{:.1}", s.entity_distance)),
        ("graphicsMode".into(), g.to_string()),
        ("particles".into(), p.to_string()),
        ("renderClouds".into(), clouds.to_string()),
        ("maxFps".into(), s.max_fps.to_string()),
        ("enableVsync".into(), s.vsync.to_string()),
        ("fullscreen".into(), s.fullscreen.to_string()),
    ]
}

/// Writes the managed keys into options.txt, keeping every other line. The very first version of the
/// file is saved once as `options.txt.original`, and every write also saves a timestamped copy.
pub fn write_options(inst: &Instance) -> Result<()> {
    let game = inst.game_path();
    std::fs::create_dir_all(&game)?;
    let path = game.join("options.txt");
    let bdir = paths::backups().join(&inst.id);
    std::fs::create_dir_all(&bdir)?;
    let original = bdir.join("options.txt.original");

    let existing = if path.is_file() { Some(std::fs::read_to_string(&path)?) } else { None };
    if !original.exists() {
        std::fs::write(&original, existing.clone().unwrap_or_default())?;
    }
    if let Some(text) = &existing {
        std::fs::write(bdir.join(format!("options-{}.txt", chrono::Utc::now().timestamp())), text)?;
    }

    let mut over: HashMap<String, String> = options_overrides(&inst.settings).into_iter().collect();
    // Older versions use a boolean instead of graphicsMode.
    if let Some(t) = &existing {
        if t.contains("fancyGraphics:") && !t.contains("graphicsMode:") {
            over.remove("graphicsMode");
            over.insert("fancyGraphics".into(), (inst.settings.graphics != "fast").to_string());
        }
    }

    let mut out: Vec<String> = Vec::new();
    for line in existing.as_deref().unwrap_or("").lines() {
        match line.split_once(':') {
            Some((k, _)) if over.contains_key(k) => {
                let v = over.remove(k).unwrap();
                out.push(format!("{k}:{v}"));
            }
            _ => out.push(line.to_string()),
        }
    }
    let mut rest: Vec<_> = over.into_iter().collect();
    rest.sort();
    for (k, v) in rest {
        out.push(format!("{k}:{v}"));
    }
    std::fs::write(&path, out.join("\n") + "\n")?;
    Ok(())
}

pub fn restore_options(inst: &Instance) -> Result<()> {
    let original = paths::backups().join(&inst.id).join("options.txt.original");
    if !original.is_file() {
        return fail("No backup of the original options.txt exists yet, so there is nothing to restore.");
    }
    std::fs::create_dir_all(inst.game_path())?;
    std::fs::copy(&original, inst.game_path().join("options.txt"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(ram_gb: u64, threads: usize, gpus: &[&str]) -> HardwareInfo {
        HardwareInfo {
            cpu_name: None, physical_cores: None, threads, gpus: gpus.iter().map(|s| s.to_string()).collect(),
            ram_total_mb: ram_gb * 1024, ram_available_mb: ram_gb * 1024 * 3 / 4, os: "test".into(), os_version: None,
            arch: "x86_64".into(), storage: vec![], data_dir_free_mb: None,
        }
    }

    #[test]
    fn recommends_by_hardware() {
        assert_eq!(recommend(&hw(4, 2, &[])).profile_id, "potato");
        assert_eq!(recommend(&hw(8, 4, &["Intel UHD Graphics 620"])).profile_id, "low");
        assert_eq!(recommend(&hw(16, 8, &["NVIDIA GeForce RTX 3060"])).profile_id, "high");
        assert_eq!(recommend(&hw(32, 16, &["NVIDIA GeForce RTX 4080"])).profile_id, "max");
    }

    #[test]
    fn ram_never_exceeds_half() {
        let h = hw(8, 8, &[]);
        let r = recommend_ram(find("max").unwrap(), &h);
        assert!(r <= 4096);
        assert_eq!(r % 256, 0);
    }
}
