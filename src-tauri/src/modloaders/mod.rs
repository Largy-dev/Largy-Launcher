//! Mod loader installer abstraction shared by Fabric, Quilt, Forge and NeoForge.
//! `resolve()` always returns a ready-to-use `LoaderProfile`: Fabric/Quilt just
//! merge a version-json delta, while Forge/NeoForge additionally download their
//! installer jar and run its "install profile" processor chain (cached per
//! Minecraft+loader version via a marker file) before returning.

pub mod fabric;
pub mod forge;
pub mod forge_common;
pub mod neoforge;
pub mod quilt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::minecraft::manifest::{self, RawVersionJson};
use crate::providers::LoaderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub name: String,
    pub url: String,
    pub sha1: Option<String>,
    pub path: std::path::PathBuf,
}

/// A record of one already-executed Forge/NeoForge "install profile"
/// processor step (`java -cp <classpath> <main_class> <args>`), kept only
/// for diagnostics/logging — by the time a `LoaderProfile` exists, every
/// step in here has already run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorStep {
    pub main_class: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderProfile {
    pub extra_libraries: Vec<LibraryEntry>,
    pub main_class_override: Option<String>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    pub install_side_effects: Vec<ProcessorStep>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("(de)serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("unsupported minecraft version: {0}")]
    UnsupportedVersion(String),
    #[error("loader install error: {0}")]
    Other(String),
}

impl From<LoaderError> for crate::error::AppError {
    fn from(err: LoaderError) -> Self {
        crate::error::AppError::Loader(err.to_string())
    }
}

#[async_trait]
pub trait LoaderInstaller: Send + Sync {
    fn kind(&self) -> LoaderKind;
    async fn list_versions(&self, mc_version: &str) -> Result<Vec<String>, LoaderError>;
    async fn resolve(
        &self,
        app: &AppHandle,
        mc_version: &str,
        loader_version: &str,
    ) -> Result<LoaderProfile, LoaderError>;
}

/// Fabric's and Quilt's "profile/json" responses are a version-json *delta*
/// on top of vanilla (`inheritsFrom`): just extra libraries, a main class
/// override, and occasionally extra arguments. Both loaders share this
/// conversion into a [`LoaderProfile`].
pub(crate) async fn loader_profile_from_delta_json(
    app: &AppHandle,
    downloader: &DownloadManager,
    raw: &RawVersionJson,
    libraries_dir: &std::path::Path,
) -> Result<LoaderProfile, LoaderError> {
    let resolved = crate::minecraft::libraries::resolve_libraries(&raw.libraries, libraries_dir);

    downloader
        .run_batch(
            app,
            "loader-libraries",
            "Bibliothèques du mod loader",
            resolved.classpath_items.clone(),
            8,
        )
        .await
        .map_err(|e| LoaderError::Other(e.to_string()))?;

    let extra_libraries = resolved
        .classpath_items
        .into_iter()
        .map(|item| LibraryEntry {
            name: item.url.clone(),
            url: item.url,
            sha1: item.sha1,
            path: item.dest,
        })
        .collect();

    let (extra_jvm_args, extra_game_args) = match &raw.arguments {
        Some(args) => (manifest::flatten_args(&args.jvm), manifest::flatten_args(&args.game)),
        None => (Vec::new(), Vec::new()),
    };

    Ok(LoaderProfile {
        extra_libraries,
        main_class_override: Some(raw.main_class.clone()),
        extra_jvm_args,
        extra_game_args,
        install_side_effects: Vec::new(),
    })
}

#[derive(Default)]
pub struct LoaderRegistry {
    installers: Vec<Box<dyn LoaderInstaller>>,
}

impl LoaderRegistry {
    pub fn new() -> Self {
        Self { installers: Vec::new() }
    }

    pub fn register(&mut self, installer: Box<dyn LoaderInstaller>) {
        self.installers.push(installer);
    }

    pub fn get(&self, kind: LoaderKind) -> Option<&dyn LoaderInstaller> {
        self.installers.iter().find(|i| i.kind() == kind).map(|i| i.as_ref())
    }
}
