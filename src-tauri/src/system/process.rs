use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{mpsc, Notify},
};

use crate::{
    error::*,
    state::{AppState, RunningGame},
};

pub struct PreparedLaunch {
    pub instance_id: String,
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub ts_ms: i64,
    pub ram_mb: f32,
    pub cpu_pct: f32,
}

pub struct Finished {
    pub code: Option<i32>,
    pub stopped: bool,
    pub load_secs: Option<f64>,
    pub samples: Vec<Sample>,
}

fn newest_crash_report(game_dir: &Path, since: SystemTime) -> Option<String> {
    let rd = std::fs::read_dir(game_dir.join("crash-reports")).ok()?;
    let floor = since.checked_sub(Duration::from_secs(5)).unwrap_or(since);
    rd.flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?.modified().ok()?;
            (m >= floor).then(|| (m, e.path()))
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p.to_string_lossy().into_owned())
}

/// Spawn the game, stream its output to the UI and a log file, optionally sample its resource use,
/// and wait for it to exit. The game is never started through a shell.
pub async fn run(app: &AppHandle, p: PreparedLaunch, sample: bool) -> Result<Finished> {
    let state = app.state::<AppState>();
    let mut cmd = tokio::process::Command::new(&p.java);
    cmd.args(&p.args)
        .current_dir(&p.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| AppError::Msg(format!("Could not start Java: {e}")))?;
    let pid = child.id().unwrap_or(0);

    let stop = Arc::new(Notify::new());
    state.games.lock().unwrap().insert(p.instance_id.clone(), RunningGame { pid, stop: stop.clone() });
    let _ = app.emit("game-started", json!({ "instanceId": p.instance_id, "pid": pid }));
    let started = Instant::now();
    let started_sys = SystemTime::now();

    // ---- log pipeline ----
    let log_dir = p.game_dir.join("saynay-logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("launch-{}.log", chrono::Utc::now().timestamp()));
    let mut log_file = tokio::fs::OpenOptions::new().create(true).append(true).open(&log_path).await.ok();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    if let Some(o) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(o).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
    }
    if let Some(o) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(o).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    let load: Arc<Mutex<Option<f64>>> = Arc::new(Mutex::new(None));
    let writer = {
        let app = app.clone();
        let id = p.instance_id.clone();
        let load = load.clone();
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if let Some(f) = log_file.as_mut() {
                    let _ = f.write_all(line.as_bytes()).await;
                    let _ = f.write_all(b"\n").await;
                }
                {
                    let mut g = load.lock().unwrap();
                    // "Sound engine started" is logged once the client finished loading and reached the menu.
                    if g.is_none() && line.contains("Sound engine started") {
                        *g = Some(started.elapsed().as_secs_f64());
                    }
                }
                let _ = app.emit("game-log", json!({ "instanceId": id, "line": line }));
            }
        })
    };

    // ---- resource sampler (benchmarks only) ----
    let stop_sampler = Arc::new(AtomicBool::new(false));
    let samples: Arc<Mutex<Vec<Sample>>> = Arc::new(Mutex::new(Vec::new()));
    let sampler = if sample && pid != 0 {
        let st = stop_sampler.clone();
        let sm = samples.clone();
        Some(tokio::spawn(async move {
            let mut sys = System::new();
            let pid = Pid::from_u32(pid);
            let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
            while !st.load(Ordering::Relaxed) {
                sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                if let Some(pr) = sys.process(pid) {
                    sm.lock().unwrap().push(Sample {
                        ts_ms: chrono::Utc::now().timestamp_millis(),
                        ram_mb: pr.memory() as f32 / 1_048_576.0,
                        cpu_pct: (pr.cpu_usage() / ncpu).min(100.0),
                    });
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }))
    } else {
        None
    };

    // ---- wait for exit or a stop request ----
    let mut stopped = false;
    let status = tokio::select! {
        s = child.wait() => s?,
        _ = stop.notified() => {
            stopped = true;
            let _ = child.kill().await;
            child.wait().await?
        }
    };

    stop_sampler.store(true, Ordering::Relaxed);
    if let Some(h) = sampler {
        let _ = h.await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(3), writer).await;
    state.games.lock().unwrap().remove(&p.instance_id);

    let code = status.code();
    let crash = if !stopped && code != Some(0) { newest_crash_report(&p.game_dir, started_sys) } else { None };
    let _ = app.emit(
        "game-exit",
        json!({
            "instanceId": p.instance_id,
            "code": code,
            "stopped": stopped,
            "crashReport": crash,
            "logFile": log_path.to_string_lossy(),
            "error": null
        }),
    );

    let load_secs = *load.lock().unwrap();
    let samples = samples.lock().unwrap().clone();
    Ok(Finished { code, stopped, load_secs, samples })
}
