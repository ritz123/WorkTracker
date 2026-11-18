use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub date: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub category: String,
    #[serde(rename = "isAchievement", default)]
    pub is_achievement: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_storage_mode")]
    pub storage_mode: String,
    #[serde(rename = "checkUpdatesOnStartup", default)]
    pub check_updates_on_startup: bool,
}

fn default_theme() -> String {
    "default".into()
}

fn default_storage_mode() -> String {
    "local".into()
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            storage_mode: default_storage_mode(),
            check_updates_on_startup: false,
        }
    }
}

pub const APP_NAME: &str = "Work Tracker";
/// Reported version. Comes from `version` in `Cargo.toml`, so a release tag
/// and the running app always name the same number.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_TAGLINE: &str = "Track your work. Celebrate your wins.";
/// This crate's repository. The About link and the update check both read it.
/// The Electron app is a different repository and is not this release feed.
pub const APP_REPO_URL: &str = "https://github.com/ritz123/WorkTracker";
pub const APP_RELEASES_URL: &str = "https://github.com/ritz123/WorkTracker/releases";
pub const APP_CONTACT_EMAIL: &str = "sarkarbiplab@gmail.com";
pub const APP_LICENSE: &str = "GNU General Public License v2.0 (GPLv2)";