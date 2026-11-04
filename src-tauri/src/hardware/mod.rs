use serde::Serialize;
use sysinfo::{Disks, System};

use crate::{paths, system};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub mount: String,
    pub total_mb: u64,
    pub available_mb: u64,
}

/// Everything here is read from the OS. Anything that cannot be read is `None` / empty,
/// and the UI shows it as "unavailable" instead of guessing.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub cpu_name: Option<String>,
    pub physical_cores: Option<usize>,
    pub threads: usize,
    pub gpus: Vec<String>,
    pub ram_total_mb: u64,
    pub ram_available_mb: u64,
    pub os: String,
    pub os_version: Option<String>,
    pub arch: String,
    pub storage: Vec<StorageInfo>,
    pub data_dir_free_mb: Option<u64>,
}

pub fn detect() -> HardwareInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty());

    let disks = Disks::new_with_refreshed_list();
    let root = paths::root();
    let mut best: Option<(usize, u64)> = None;
    let mut storage = Vec::new();
    for d in disks.list() {
        let total = d.total_space() / 1_048_576;
        let avail = d.available_space() / 1_048_576;
        if total >= 4096 {
            storage.push(StorageInfo { mount: d.mount_point().to_string_lossy().into_owned(), total_mb: total, available_mb: avail });
        }
        if root.starts_with(d.mount_point()) {
            let len = d.mount_point().as_os_str().len();
            if best.map_or(true, |(l, _)| len > l) {
                best = Some((len, avail));
            }
        }
    }
    storage.dedup_by(|a, b| a.mount == b.mount);

    HardwareInfo {
        cpu_name,
        physical_cores: sys.physical_core_count(),
        threads: sys.cpus().len().max(1),
        gpus: detect_gpus(),
        ram_total_mb: sys.total_memory() / 1_048_576,
        ram_available_mb: sys.available_memory() / 1_048_576,
        os: System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
        os_version: System::os_version(),
        arch: std::env::consts::ARCH.to_string(),
        storage,
        data_dir_free_mb: best.map(|(_, a)| a),
    }
}

fn clean(lines: Vec<String>) -> Vec<String> {
    let mut v: Vec<String> = lines.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    v.dedup();
    v
}

#[cfg(windows)]
fn detect_gpus() -> Vec<String> {
    system::stdout_of(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_VideoController).Name"],
    )
    .map(|s| clean(s.lines().map(String::from).collect()))
    .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn detect_gpus() -> Vec<String> {
    let Some(out) = system::stdout_of("lspci", &[]) else { return vec![] };
    clean(
        out.lines()
            .filter(|l| l.contains("VGA compatible controller") || l.contains("3D controller") || l.contains("Display controller"))
            .filter_map(|l| l.splitn(2, ": ").nth(1).map(String::from))
            .collect(),
    )
}

#[cfg(target_os = "macos")]
fn detect_gpus() -> Vec<String> {
    let Some(out) = system::stdout_of("system_profiler", &["SPDisplaysDataType", "-json"]) else { return vec![] };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&out) else { return vec![] };
    clean(
        v["SPDisplaysDataType"]
            .as_array()
            .map(|a| a.iter().filter_map(|g| g["sppci_model"].as_str().map(String::from)).collect())
            .unwrap_or_default(),
    )
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn detect_gpus() -> Vec<String> {
    vec![]
}
