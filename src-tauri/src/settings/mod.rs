//! Global launcher settings (default RAM/JVM args, external API credentials),
//! persisted as JSON under the app data dir. Per-instance overrides live on
//! `instances::Instance` instead of here.

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::paths::AppPaths;
use crate::util::fs::write_atomic;

pub const MIN_HEAP_MB: u32 = 512;

/// `(min, max)` heap sizes the JVM will accept: max at least [`MIN_HEAP_MB`],
/// min at least 128 and never above max.
pub fn sanitize_memory(min_mb: u32, max_mb: u32) -> (u32, u32) {
    let max = max_mb.max(MIN_HEAP_MB);
    (min_mb.clamp(128, max), max)
}

/// Largy Launcher's own public-client Azure AD application id (device code
/// flow, `consumers` tenant, "Allow public client flows" — no secret
/// involved, so it's safe to ship). Baked in so anyone downloading a release
/// build can log in with their own Microsoft account with zero setup; still
/// overridable per-install by editing `azure_client_id` in settings.json.
const DEFAULT_AZURE_CLIENT_ID: &str = "d3201869-49d5-4102-88b0-42495ac2ac12";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub default_min_memory_mb: u32,
    pub default_max_memory_mb: u32,
    pub default_jvm_args: Vec<String>,
    /// Public-client Azure AD application id (device code flow, `consumers`
    /// tenant). Defaults to Largy Launcher's own — see [`DEFAULT_AZURE_CLIENT_ID`].
    #[serde(default)]
    pub azure_client_id: String,
    /// CurseForge Core API key from console.curseforge.com — required to
    /// browse/install CurseForge modpacks.
    #[serde(default)]
    pub curseforge_api_key: String,
    #[serde(default)]
    pub java_path_override: Option<String>,
    /// Bypasses the Microsoft login requirement at launch, using a local
    /// name-only profile instead (see `auth::offline_session`). Only usable
    /// on singleplayer or servers explicitly running in offline mode — real
    /// online-mode servers reject it.
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default)]
    pub offline_username: String,
    /// What the launcher window does once a game is running.
    #[serde(default)]
    pub on_game_launch: LauncherBehavior,
    /// What closing the window does. Reducing to the tray keeps play-time
    /// tracking and crash reports working for games still running.
    #[serde(default)]
    pub on_close: CloseBehavior,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    /// Ask the first time (the answer can be remembered).
    #[default]
    Ask,
    Tray,
    Quit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherBehavior {
    #[default]
    KeepOpen,
    Minimize,
    /// Hidden while the game runs, shown again when it exits.
    Hide,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            default_min_memory_mb: 1024,
            default_max_memory_mb: 4096,
            default_jvm_args: Vec::new(),
            azure_client_id: DEFAULT_AZURE_CLIENT_ID.to_string(),
            curseforge_api_key: String::new(),
            java_path_override: None,
            offline_mode: false,
            offline_username: String::new(),
            on_game_launch: LauncherBehavior::KeepOpen,
            on_close: CloseBehavior::Ask,
        }
    }
}

impl GlobalSettings {
    /// Never fails on a bad file: an unreadable settings.json is moved aside
    /// (`settings.json.corrupt`) and defaults are used, so one bad write can't
    /// stop the launcher from opening.
    pub fn load(paths: &AppPaths) -> AppResult<Self> {
        let path = paths.settings_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(&path)?;
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(settings) => Ok(settings.sanitized()),
            Err(e) => {
                tracing::error!("settings.json is unreadable ({e}); falling back to defaults");
                let _ = std::fs::rename(&path, path.with_extension("json.corrupt"));
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, paths: &AppPaths) -> AppResult<()> {
        write_atomic(&paths.settings_file(), serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }

    /// Clamps values the JVM would refuse (`-Xmx0M`, min above max).
    pub fn sanitized(mut self) -> Self {
        let (min, max) = sanitize_memory(self.default_min_memory_mb, self.default_max_memory_mb);
        self.default_min_memory_mb = min;
        self.default_max_memory_mb = max;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_defaults_when_no_settings_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let settings = GlobalSettings::load(&paths).unwrap();
        assert_eq!(settings.default_min_memory_mb, 1024);
        assert!(!settings.offline_mode);
        assert_eq!(settings.azure_client_id, DEFAULT_AZURE_CLIENT_ID);
    }

    #[test]
    fn corrupt_settings_file_falls_back_to_defaults_and_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        std::fs::write(paths.settings_file(), b"{ not json").unwrap();

        let settings = GlobalSettings::load(&paths).unwrap();

        assert_eq!(settings.default_max_memory_mb, 4096);
        assert!(dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn sanitize_memory_fixes_values_the_jvm_would_reject() {
        assert_eq!(sanitize_memory(0, 0), (128, MIN_HEAP_MB));
        assert_eq!(sanitize_memory(8192, 4096), (4096, 4096));
        assert_eq!(sanitize_memory(1024, 4096), (1024, 4096));
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());

        let settings = GlobalSettings {
            azure_client_id: "client-id".to_string(),
            offline_mode: true,
            offline_username: "Steve".to_string(),
            default_jvm_args: vec!["-Xmx2G".to_string()],
            ..GlobalSettings::default()
        };
        settings.save(&paths).unwrap();

        let loaded = GlobalSettings::load(&paths).unwrap();
        assert_eq!(loaded.azure_client_id, "client-id");
        assert!(loaded.offline_mode);
        assert_eq!(loaded.offline_username, "Steve");
        assert_eq!(loaded.default_jvm_args, vec!["-Xmx2G".to_string()]);
    }
}
