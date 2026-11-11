use std::path::PathBuf;

/// Root data folder: %APPDATA%\SAYNAY, ~/.local/share/SAYNAY or ~/Library/Application Support/SAYNAY.
pub fn root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("SAYNAY")
}
pub fn versions() -> PathBuf { root().join("versions") }
pub fn libraries() -> PathBuf { root().join("libraries") }
pub fn assets() -> PathBuf { root().join("assets") }
pub fn runtimes() -> PathBuf { root().join("runtimes") }
pub fn instances() -> PathBuf { root().join("instances") }
pub fn backups() -> PathBuf { root().join("backups") }
pub fn logs() -> PathBuf { root().join("logs") }
pub fn cache() -> PathBuf { root().join("cache") }
pub fn bench_tools() -> PathBuf { root().join("bench") }
pub fn database() -> PathBuf { root().join("saynay.db") }

pub fn ensure_all() -> std::io::Result<()> {
    for p in [versions(), libraries(), assets(), runtimes(), instances(), backups(), bench_tools(), logs(), cache()] {
        std::fs::create_dir_all(p)?;
    }
    Ok(())
}
