//! Modpack provider abstraction. FTB is the first implementation (see `ftb.rs`,
//! added in Phase 3); Modrinth/generic CurseForge slot in later behind the same
//! trait without touching instance-install call sites.

pub mod curseforge;
pub mod ftb;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Vanilla,
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackSummary {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackDetails {
    pub summary: ModpackSummary,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackVersionSummary {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackFileRef {
    pub project_id: String,
    pub file_id: String,
    pub path: std::path::PathBuf,
    pub sha1: Option<String>,
    pub size: u64,
    /// Set by providers (FTB) that already know the final download URL by
    /// the time `resolve_version` runs, so `resolve_file_download` doesn't
    /// need a second API round-trip. Providers that must look it up per-file
    /// (CurseForge, for distribution-restricted mods) leave this `None`.
    #[serde(default)]
    pub direct_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedModpackVersion {
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: String,
    pub files: Vec<ModpackFileRef>,
    /// CurseForge (and any other zip-distributed pack) ships an `overrides/`
    /// folder of arbitrary config/script files alongside the mod list; this
    /// points at that folder, already extracted to a cache location, for the
    /// instance installer to copy wholesale. `None` for FTB, whose files
    /// array is already the complete flat file list.
    #[serde(default)]
    pub overrides_dir: Option<std::path::PathBuf>,
}

/// Some CurseForge-hosted mod files disable third-party downloads; the provider
/// returns `ManualRequired` instead of erroring so the UI can offer a
/// "download in browser, then drop the file here" fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FileDownloadInfo {
    Direct { url: String },
    ManualRequired { browser_url: String, expected_filename: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("modpack not found: {0}")]
    NotFound(String),
    #[error("modpack provider error: {0}")]
    Other(String),
}

impl From<ProviderError> for crate::error::AppError {
    fn from(err: ProviderError) -> Self {
        crate::error::AppError::Provider(err.to_string())
    }
}

#[async_trait]
pub trait ModpackProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn search(&self, query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError>;
    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError>;
    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError>;
    async fn resolve_version(
        &self,
        pack_id: &str,
        version_id: &str,
    ) -> Result<ResolvedModpackVersion, ProviderError>;
    async fn resolve_file_download(
        &self,
        file: &ModpackFileRef,
    ) -> Result<FileDownloadInfo, ProviderError>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
}

/// Holds every registered `ModpackProvider`; commands and the modpack-browser
/// screen only ever talk to the registry, never to a concrete provider type.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<Box<dyn ModpackProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn ModpackProvider>) {
        self.providers.push(provider);
    }

    pub fn get(&self, id: &str) -> Option<&dyn ModpackProvider> {
        self.providers.iter().find(|p| p.id() == id).map(|p| p.as_ref())
    }

    pub fn all(&self) -> impl Iterator<Item = &dyn ModpackProvider> {
        self.providers.iter().map(|p| p.as_ref())
    }
}
