//! Real benchmarking. Frame times come from the SAYNAY Bench companion mod (bench-mod/), which records
//! every rendered frame in the same world after a warm-up. The launcher adds process RAM/CPU sampling and
//! load time. If any piece is unavailable the result says so; nothing is ever estimated or invented.

use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, path::PathBuf};
use tauri::{AppHandle, Manager};

use crate::{
    error::*,
    minecraft::{launch, Instance},
    paths,
    state::AppState,
    system::process::{self, Sample},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BenchResult {
    #[serde(default)]
    pub id: i64,
    pub instance_id: String,
    pub label: String,
    pub mc_version: String,
    pub profile: String,
    pub created_at: i64,
    pub duration_s: u32,
    pub avg_fps: Option<f64>,
    pub low1_fps: Option<f64>,
    pub frame_ms_stddev: Option<f64>,
    pub frame_count: usize,
    /// Downsampled (bucket-averaged) for charting only; statistics use every frame.
    pub frame_times_ms: Vec<f32>,
    pub ram_avg_mb: Option<f64>,
    pub ram_peak_mb: Option<f64>,
    pub cpu_avg_pct: Option<f64>,
    pub load_secs: Option<f64>,
    pub notes: Vec<String>,
}

/// (average FPS, 1% low FPS, frame-time standard deviation in ms)
pub fn frame_stats(frames: &[f32]) -> Option<(f64, f64, f64)> {
    if frames.len() < 30 {
        return None;
    }
    let n = frames.len() as f64;
    let mean = frames.iter().map(|&x| x as f64).sum::<f64>() / n;
    let mut sorted = frames.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
    let k = ((n * 0.01).ceil() as usize).max(1);
    let worst = sorted[..k].iter().map(|&x| x as f64).sum::<f64>() / k as f64;
    let var = frames.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / n;
    Some((1000.0 / mean, 1000.0 / worst, var.sqrt()))
}

/// CSV: optional header `# start_ms=<unix ms>;end_ms=<unix ms>`, then one frame time (ms) per line.
pub fn parse_csv(text: &str) -> (Vec<f32>, Option<(i64, i64)>) {
    let mut window = None;
    let mut v = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Some(h) = l.strip_prefix('#') {
            let (mut s, mut e) = (None, None);
            for kv in h.split(';') {
                if let Some((k, val)) = kv.trim().split_once('=') {
                    match k {
                        "start_ms" => s = val.parse().ok(),
                        "end_ms" => e = val.parse().ok(),
                        _ => {}
                    }
                }
            }
            if let (Some(s), Some(e)) = (s, e) {
                window = Some((s, e));
            }
        } else if let Ok(x) = l.parse::<f32>() {
            if x > 0.0 && x < 5000.0 {
                v.push(x);
            }
        }
    }
    (v, window)
}

fn downsample(frames: &[f32], max: usize) -> Vec<f32> {
    if frames.len() <= max {
        return frames.to_vec();
    }
    let size = (frames.len() as f64 / max as f64).ceil() as usize;
    frames.chunks(size).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect()
}

fn find_bench_jar(app: &AppHandle) -> Option<PathBuf> {
    let user = paths::bench_tools().join("saynay-bench.jar");
    if user.is_file() {
        return Some(user);
    }
    let bundled = app.path().resource_dir().ok()?.join("bench").join("saynay-bench.jar");
    bundled.is_file().then_some(bundled)
}

pub async fn run(app: &AppHandle, inst: &Instance, label: String, seconds: u32) -> Result<BenchResult> {
    let state = app.state::<AppState>();
    let out_dir = inst.game_path().join("saynay-bench");
    std::fs::create_dir_all(&out_dir)?;
    let csv = out_dir.join("frames.csv");
    let _ = std::fs::remove_file(&csv);

    let mut notes: Vec<String> = Vec::new();
    let mut extra: Vec<String> = Vec::new();
    let mod_target = inst.game_path().join("mods").join("saynay-bench.jar");
    let mut injected = false;
    match (find_bench_jar(app), inst.loader.as_str()) {
        (Some(jar), "fabric") => {
            std::fs::create_dir_all(inst.game_path().join("mods"))?;
            std::fs::copy(jar, &mod_target)?;
            injected = true;
            extra.push(format!("-Dsaynay.bench.out={}", csv.display()));
            extra.push(format!("-Dsaynay.bench.seconds={seconds}"));
        }
        (None, _) => notes.push("FPS capture is unavailable: saynay-bench.jar was not found (see bench-mod/README.md).".into()),
        (_, _) => notes.push("FPS capture needs a Fabric instance.".into()),
    }

    let prepared = match launch::prepare(app, inst, extra).await {
        Ok(p) => p,
        Err(e) => {
            if injected {
                let _ = std::fs::remove_file(&mod_target);
            }
            return Err(e);
        }
    };
    let finished = process::run(app, prepared, true).await;
    if injected {
        let _ = std::fs::remove_file(&mod_target);
    }
    let fin = finished?;

    let (frames, window) = std::fs::read_to_string(&csv).map(|t| parse_csv(&t)).unwrap_or((vec![], None));
    let stats = frame_stats(&frames);
    if stats.is_none() && injected {
        notes.push("No frame data was recorded. Join a world and leave the game running until it closes itself.".into());
    }
    if fin.code != Some(0) && !fin.stopped {
        notes.push(format!("The game exited with code {:?}; results may be incomplete.", fin.code));
    }

    let used: Vec<&Sample> = match window {
        Some((s, e)) => fin.samples.iter().filter(|x| x.ts_ms >= s && x.ts_ms <= e).collect(),
        None => {
            if !fin.samples.is_empty() {
                notes.push("RAM and CPU figures cover the whole session because no measurement window was recorded.".into());
            }
            fin.samples.iter().collect()
        }
    };
    let (ram_avg, ram_peak, cpu_avg) = if used.is_empty() {
        (None, None, None)
    } else {
        let n = used.len() as f64;
        (
            Some(used.iter().map(|s| s.ram_mb as f64).sum::<f64>() / n),
            Some(used.iter().map(|s| s.ram_mb as f64).fold(0.0, f64::max)),
            Some(used.iter().map(|s| s.cpu_pct as f64).sum::<f64>() / n),
        )
    };
    if fin.load_secs.is_none() {
        notes.push("Load time unavailable: the 'Sound engine started' log line was not seen.".into());
    }

    let mut r = BenchResult {
        id: 0,
        instance_id: inst.id.clone(),
        label,
        mc_version: inst.mc_version.clone(),
        profile: inst.profile.clone(),
        created_at: chrono::Utc::now().timestamp(),
        duration_s: seconds,
        avg_fps: stats.map(|s| s.0),
        low1_fps: stats.map(|s| s.1),
        frame_ms_stddev: stats.map(|s| s.2),
        frame_count: frames.len(),
        frame_times_ms: downsample(&frames, 400),
        ram_avg_mb: ram_avg,
        ram_peak_mb: ram_peak,
        cpu_avg_pct: cpu_avg,
        load_secs: fin.load_secs,
        notes,
    };
    r.id = state.db.add_benchmark(&r)?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_are_correct() {
        let mut f = vec![10.0_f32; 99];
        f.push(50.0);
        let (avg, low1, _) = frame_stats(&f).unwrap();
        // mean = (99*10 + 50)/100 = 10.4 ms
        assert!((avg - 1000.0 / 10.4).abs() < 0.01);
        // worst 1% = the single 50 ms frame
        assert!((low1 - 20.0).abs() < 0.01);
    }

    #[test]
    fn too_few_frames_gives_none() {
        assert!(frame_stats(&[16.0; 10]).is_none());
    }

    #[test]
    fn csv_parsing() {
        let (v, w) = parse_csv("# start_ms=1000;end_ms=2000\n16.6\n17.1\nbad\n-3\n");
        assert_eq!(v.len(), 2);
        assert_eq!(w, Some((1000, 2000)));
    }
}
