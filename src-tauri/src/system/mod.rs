pub mod process;
pub mod update;

use serde::Serialize;
use std::process::{Command, Output};
use sysinfo::System;

/// Run a short-lived helper command without flashing a console window on Windows.
pub fn run(cmd: &str, args: &[&str]) -> Option<Output> {
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c.output().ok()
}

pub fn stdout_of(cmd: &str, args: &[&str]) -> Option<String> {
    let o = run(cmd, args)?;
    if !o.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&o.stdout).into_owned())
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub cpu_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
}

/// Cheap live usage sample for the Home page (only polled while that page is visible).
pub fn usage(sys: &mut System) -> Usage {
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    Usage {
        cpu_pct: sys.global_cpu_usage(),
        ram_used_mb: sys.used_memory() / 1_048_576,
        ram_total_mb: sys.total_memory() / 1_048_576,
    }
}
