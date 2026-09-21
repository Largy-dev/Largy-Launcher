//! Central place for every on-disk location the launcher uses under the
//! Tauri app data dir, so no other module hardcodes a subfolder name.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    pub fn new(app: &AppHandle) -> Self {
        let root = app
            .path()
            .app_data_dir()
            .expect("app data dir must be resolvable");
        Self { root }
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.instances_dir().join(instance_id)
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.cache_dir().join("versions")
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.cache_dir().join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.cache_dir().join("assets")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.cache_dir().join("runtime")
    }

    pub fn installers_dir(&self) -> PathBuf {
        self.cache_dir().join("installers")
    }
}

pub fn ensure_dir(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}
