use std::fs;
use std::path::PathBuf;

use crate::models::{Prefs, WorkItem};

/// Matches the Electron app: `~/.weekly-tracker/`.
fn data_dir() -> PathBuf {
    directories::UserDirs::new()
        .map(|u| u.home_dir().join(".weekly-tracker"))
        .unwrap_or_else(|| PathBuf::from(".weekly-tracker"))
}

fn data_file() -> PathBuf {
    data_dir().join("data.json")
}

fn prefs_file() -> PathBuf {
    data_dir().join("prefs.json")
}

/// Ensures `~/.weekly-tracker` (or XDG data dir) exists before any read/write.
fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn storage_dir_display() -> String {
    data_dir().display().to_string()
}

pub fn load_items() -> std::io::Result<Vec<WorkItem>> {
    ensure_data_dir()?;
    let path = data_file();
    if !path.exists() {
        fs::write(&path, "[]")?;
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save_items(items: &[WorkItem]) -> std::io::Result<()> {
    ensure_data_dir()?;
    let json = serde_json::to_string_pretty(items)?;
    fs::write(data_file(), json)
}

pub fn load_prefs() -> Prefs {
    let _ = ensure_data_dir();
    let path = prefs_file();
    if !path.exists() {
        return Prefs::default();
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_prefs(prefs: &Prefs) -> std::io::Result<()> {
    ensure_data_dir()?;
    let json = serde_json::to_string_pretty(prefs)?;
    fs::write(prefs_file(), json)
}