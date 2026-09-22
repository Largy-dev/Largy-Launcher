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

impl LoaderKind {
    /// Case-insensitive match on a mod loader's name, as it appears in FTB's
    /// `targets` array, CurseForge's `gameVersions` list, and manifest
    /// `modLoaders[].id` prefixes. Shared by every provider instead of each
    /// duplicating the same four-way match.
    pub fn from_name(name: &str) -> Option<LoaderKind> {
        match name.to_lowercase().as_str() {
            "forge" => Some(LoaderKind::Forge),
            "neoforge" => Some(LoaderKind::NeoForge),
            "fabric" => Some(LoaderKind::Fabric),
            "quilt" => Some(LoaderKind::Quilt),
            _ => None,
        }
    }
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

/// Scans `(name, version)` pairs — e.g. FTB's `targets` array — and returns
/// the first one whose name [`LoaderKind::from_name`] recognizes. Shared so
/// callers with the same "list of named targets" shape don't each reimplement
/// the same `find_map`.
pub fn find_loader_target<'a>(mut items: impl Iterator<Item = (&'a str, &'a str)>) -> Option<(LoaderKind, String)> {
    items.find_map(|(name, version)| LoaderKind::from_name(name).map(|kind| (kind, version.to_string())))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_matches_known_loaders_case_insensitively() {
        assert_eq!(LoaderKind::from_name("forge"), Some(LoaderKind::Forge));
        assert_eq!(LoaderKind::from_name("Forge"), Some(LoaderKind::Forge));
        assert_eq!(LoaderKind::from_name("NEOFORGE"), Some(LoaderKind::NeoForge));
        assert_eq!(LoaderKind::from_name("fabric"), Some(LoaderKind::Fabric));
        assert_eq!(LoaderKind::from_name("Quilt"), Some(LoaderKind::Quilt));
    }

    #[test]
    fn from_name_rejects_unknown_or_vanilla() {
        assert_eq!(LoaderKind::from_name("vanilla"), None);
        assert_eq!(LoaderKind::from_name("minecraft"), None);
        assert_eq!(LoaderKind::from_name(""), None);
    }

    #[test]
    fn registry_get_returns_none_for_unregistered_provider() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("ftb").is_none());
    }

    #[test]
    fn find_loader_target_returns_first_recognized_loader() {
        let items = vec![("minecraft", "1.20.1"), ("neoforge", "20.1.80")];
        let found = find_loader_target(items.into_iter());
        assert_eq!(found, Some((LoaderKind::NeoForge, "20.1.80".to_string())));
    }

    #[test]
    fn find_loader_target_returns_none_when_nothing_recognized() {
        let items = vec![("minecraft", "1.20.1"), ("something-else", "1.0")];
        assert_eq!(find_loader_target(items.into_iter()), None);
    }
}
