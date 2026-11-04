pub mod install;
pub mod launch;
pub mod version;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    /// "vanilla" or "fabric"
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    /// The id of the launchable version JSON (the Fabric profile id for Fabric instances).
    pub version_id: String,
    #[serde(default)]
    pub installed: bool,
    pub game_dir: String,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub settings: GameSettings,
    pub created_at: i64,
}

fn default_profile() -> String {
    "balanced".into()
}

impl Instance {
    pub fn game_path(&self) -> PathBuf {
        PathBuf::from(&self.game_dir)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GameSettings {
    pub ram_mb: u32,
    /// Empty = use the SAYNAY recommended JVM flags for the selected profile.
    pub custom_jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    /// fast | fancy | fabulous
    pub graphics: String,
    pub render_distance: u32,
    pub simulation_distance: u32,
    pub entity_distance: f32,
    pub vsync: bool,
    /// 260 means unlimited in Minecraft's options.txt.
    pub max_fps: u32,
    /// all | decreased | minimal
    pub particles: String,
    /// off | fast | fancy
    pub clouds: String,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            ram_mb: 4096,
            custom_jvm_args: String::new(),
            width: 1280,
            height: 720,
            fullscreen: false,
            graphics: "fancy".into(),
            render_distance: 12,
            simulation_distance: 8,
            entity_distance: 1.0,
            vsync: false,
            max_fps: 144,
            particles: "decreased".into(),
            clouds: "fast".into(),
        }
    }
}
