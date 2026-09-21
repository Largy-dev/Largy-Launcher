//! Global launcher settings (default RAM/JVM args, external API credentials),
//! persisted as JSON under the app data dir. Per-instance overrides live on
//! `instances::Instance` instead of here.

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::paths::AppPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub default_min_memory_mb: u32,
    pub default_max_memory_mb: u32,
    pub default_jvm_args: Vec<String>,
    /// Public-client Azure AD application id (device code flow, `consumers`
    /// tenant) — required for Microsoft login. See Settings screen for setup.
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
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            default_min_memory_mb: 1024,
            default_max_memory_mb: 4096,
            default_jvm_args: Vec::new(),
            azure_client_id: String::new(),
            curseforge_api_key: String::new(),
            java_path_override: None,
            offline_mode: false,
            offline_username: String::new(),
        }
    }
}

impl GlobalSettings {
    pub fn load(paths: &AppPaths) -> AppResult<Self> {
        let path = paths.settings_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self, paths: &AppPaths) -> AppResult<()> {
        std::fs::create_dir_all(paths.root())?;
        std::fs::write(paths.settings_file(), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
