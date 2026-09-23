//! Mod loader installer abstraction shared by Fabric, Quilt, Forge and NeoForge.
//! `resolve()` always returns a ready-to-use `LoaderProfile`: Fabric/Quilt just
//! merge a version-json delta, while Forge/NeoForge additionally download their
//! installer jar and run its "install profile" processor chain (cached per
//! Minecraft+loader version via a marker file) before returning.

pub mod fabric;
pub mod forge;
pub mod forge_common;
pub mod neoforge;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::java::JavaManager;
use crate::minecraft::manifest::{self, RawVersionJson};
use crate::paths::AppPaths;
use crate::providers::LoaderKind;
use crate::util::http_cache::MetaCache;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub name: String,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoaderProfile {
    pub extra_libraries: Vec<LibraryEntry>,
    pub main_class_override: Option<String>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    /// Pre-1.13 Forge replaces vanilla's `minecraftArguments` wholesale
    /// (it adds `--tweakClass`), rather than appending to them.
    pub game_args_override: Option<Vec<String>>,
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
    #[error("{0}")]
    App(#[from] crate::error::AppError),
    #[error("unsupported minecraft version: {0}")]
    UnsupportedVersion(String),
    #[error("loader install error: {0}")]
    Other(String),
}

impl From<LoaderError> for crate::error::AppError {
    fn from(err: LoaderError) -> Self {
        match err {
            LoaderError::App(inner) => inner,
            other => crate::error::AppError::Loader(other.to_string()),
        }
    }
}

/// Everything an installer needs, borrowed from the launch in progress.
pub struct LoaderContext<'a> {
    pub app: &'a AppHandle,
    pub paths: &'a AppPaths,
    pub meta: &'a MetaCache,
    pub downloader: &'a DownloadManager,
    pub java: &'a JavaManager,
    /// Runtime component of the vanilla version (Forge processors run on it).
    pub java_component: &'a str,
    /// Re-run install steps even when a completion marker exists (repair).
    pub force_reinstall: bool,
}

#[async_trait]
pub trait LoaderInstaller: Send + Sync {
    fn kind(&self) -> LoaderKind;
    /// Newest first.
    async fn list_versions(&self, meta: &MetaCache, mc_version: &str) -> Result<Vec<String>, LoaderError>;
    async fn resolve(
        &self,
        ctx: &LoaderContext<'_>,
        mc_version: &str,
        loader_version: &str,
    ) -> Result<LoaderProfile, LoaderError>;
}

/// Fabric's and Quilt's "profile/json" responses are a version-json *delta*
/// on top of vanilla (`inheritsFrom`): just extra libraries, a main class
/// override, and occasionally extra arguments.
pub(crate) async fn loader_profile_from_delta_json(
    ctx: &LoaderContext<'_>,
    raw: &RawVersionJson,
) -> Result<LoaderProfile, LoaderError> {
    let resolved = crate::minecraft::libraries::resolve_libraries(&raw.libraries, &ctx.paths.libraries_dir());
    ctx.downloader
        .run_batch(ctx.app, "loader-libraries", "Bibliothèques du mod loader", resolved.download_items(), 8)
        .await?;

    let extra_libraries = resolved
        .classpath
        .into_iter()
        .map(|l| LibraryEntry { name: l.name, path: l.item.dest })
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
        game_args_override: None,
    })
}

pub struct LoaderRegistry {
    installers: Vec<Box<dyn LoaderInstaller>>,
}

impl LoaderRegistry {
    pub fn with_defaults() -> Self {
        Self {
            installers: vec![
                Box::new(fabric::MetaLoaderInstaller::fabric()),
                Box::new(fabric::MetaLoaderInstaller::quilt()),
                Box::new(forge::ForgeInstaller),
                Box::new(neoforge::NeoForgeInstaller),
            ],
        }
    }

    pub fn get(&self, kind: LoaderKind) -> Option<&dyn LoaderInstaller> {
        self.installers.iter().find(|i| i.kind() == kind).map(|i| i.as_ref())
    }
}
