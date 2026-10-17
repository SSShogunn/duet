use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub targets: Vec<String>,
    pub offsets: HashMap<String, f32>,
    pub stereo: bool,
    pub swap: bool,
    pub auto_sync: bool,
    pub debug_logging: bool,
    pub theme: String,
    pub sync_sound: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            targets: Vec::new(),
            offsets: HashMap::new(),
            stereo: false,
            swap: false,
            auto_sync: true,
            debug_logging: false,
            theme: "system".into(),
            sync_sound: "silent".into(),
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    pub data: Settings,
}

impl SettingsStore {
    pub fn load(dir: &Path) -> SettingsStore {
        let path = dir.join("settings.json");
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        SettingsStore { path, data }
    }

    pub fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&self.data) {
            let _ = fs::write(&self.path, text);
        }
    }
}
