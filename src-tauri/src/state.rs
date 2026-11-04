use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{authentication::McSession, db::Db, error::*, hardware::HardwareInfo, paths};

pub struct RunningGame {
    pub pid: u32,
    pub stop: Arc<tokio::sync::Notify>,
}

pub struct AppState {
    pub db: Db,
    pub http: reqwest::Client,
    pub sys: Mutex<sysinfo::System>,
    pub hw: Mutex<Option<HardwareInfo>>,
    pub games: Mutex<HashMap<String, RunningGame>>,
    pub session: tokio::sync::Mutex<Option<McSession>>,
}

impl AppState {
    pub fn new() -> Result<Self> {
        paths::ensure_all()?;
        let http = reqwest::Client::builder()
            .user_agent(format!("SAYNAY-Launcher/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(45))
            .build()?;
        Ok(Self {
            db: Db::open()?,
            http,
            sys: Mutex::new(sysinfo::System::new()),
            hw: Mutex::new(None),
            games: Mutex::new(HashMap::new()),
            session: tokio::sync::Mutex::new(None),
        })
    }

    /// Cached hardware snapshot (detected once, refreshed by `get_hardware`).
    pub fn hardware(&self) -> HardwareInfo {
        let mut g = self.hw.lock().unwrap();
        if g.is_none() {
            *g = Some(crate::hardware::detect());
        }
        g.clone().unwrap()
    }
}
