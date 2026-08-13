//! Config persistence: JSON file under the app config dir, atomic writes.

use crate::models::AppConfig;
use crate::util::home_dir;
use std::fs;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    PathBuf::from(home_dir())
        .join("Library/Application Support")
        .join("com.min0504.devcockpit")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load() -> AppConfig {
    let path = config_path();
    let mut cfg = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<AppConfig>(&text).unwrap_or_else(|e| {
            eprintln!("[config] parse error ({e}), using defaults");
            AppConfig::default()
        }),
        Err(_) => {
            let fresh = AppConfig {
                roots: default_roots(),
                ..AppConfig::default()
            };
            let _ = save(&fresh);
            fresh
        }
    };
    if cfg.roots.is_empty() {
        cfg.roots = default_roots();
    }
    cfg.sanitize();
    cfg
}

pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create config dir: {e}"))?;
    let tmp = dir.join("config.json.tmp");
    let body = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&tmp, body).map_err(|e| format!("write config: {e}"))?;
    fs::rename(&tmp, config_path()).map_err(|e| format!("commit config: {e}"))?;
    Ok(())
}

/// First-run default project roots: whichever common dev dirs exist.
pub fn default_roots() -> Vec<String> {
    let home = home_dir();
    ["Dev", "Developer", "Projects", "Code", "repos", "workspace"]
        .iter()
        .map(|d| format!("{home}/{d}"))
        .filter(|p| {
            fs::metadata(p)
                .map(|m| m.is_dir())
                .unwrap_or(false)
        })
        .collect()
}
