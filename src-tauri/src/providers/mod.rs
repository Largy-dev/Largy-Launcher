//! Modpack provider abstraction (Modrinth, FTB, CurseForge). Commands and the
//! modpack-browser screen only ever talk to the registry, never to a
//! concrete provider type.

pub mod archive;
pub mod curseforge;
pub mod ftb;
pub mod modrinth;

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// `targets`, CurseForge's `gameVersions`, Modrinth's `loaders` and
    /// manifest `modLoaders[].id` prefixes.
    pub fn from_name(name: &str) -> Option<LoaderKind> {
        match name.to_lowercase().as_str() {
            "forge" => Some(LoaderKind::Forge),
            "neoforge" => Some(LoaderKind::NeoForge),
            "fabric" | "fabric-loader" => Some(LoaderKind::Fabric),
            "quilt" | "quilt-loader" => Some(LoaderKind::Quilt),
            _ => None,
        }
    }

    /// Modrinth's loader facet name.
    pub fn modrinth_name(self) -> Option<&'static str> {
        match self {
            LoaderKind::Vanilla => None,
            LoaderKind::Forge => Some("forge"),
            LoaderKind::NeoForge => Some("neoforge"),
            LoaderKind::Fabric => Some("fabric"),
            LoaderKind::Quilt => Some("quilt"),
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
    #[serde(default)]
    pub downloads: Option<u64>,
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
    /// Instance-relative destination; already validated by the provider
    /// (no `..`, not absolute).
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: u64,
    /// Final download URL when the provider already knows it; `None` means
    /// the author disabled third-party downloads (see `browser_url`).
    #[serde(default)]
    pub direct_url: Option<String>,
    #[serde(default)]
    pub browser_url: Option<String>,
}

/// One file that didn't make it into the instance automatically — kept
/// structured so the frontend can offer real actions (open the download
/// page, open the folder).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallWarning {
    pub file_name: String,
    pub message: String,
    pub browser_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedModpackVersion {
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: String,
    pub files: Vec<ModpackFileRef>,
    /// Folders (already extracted to a cache location) copied over the
    /// instance in order — later ones win (`overrides/` then
    /// `client-overrides/` for Modrinth packs).
    #[serde(default)]
    pub overrides_dirs: Vec<PathBuf>,
    /// Problems found while resolving (files that couldn't be looked up).
    #[serde(default)]
    pub warnings: Vec<InstallWarning>,
    /// Display name embedded in an imported pack file, if any.
    #[serde(default)]
    pub pack_name: Option<String>,
}

/// Some CurseForge-hosted mod files disable third-party downloads; the
/// provider returns `ManualRequired` instead of erroring so the UI can offer
/// a "download in browser, then drop the file here" fallback.
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
    #[error("{0}")]
    App(#[from] crate::error::AppError),
    #[error("modpack provider error: {0}")]
    Other(String),
}

impl From<std::io::Error> for ProviderError {
    fn from(err: std::io::Error) -> Self {
        ProviderError::App(err.into())
    }
}

impl From<ProviderError> for crate::error::AppError {
    fn from(err: ProviderError) -> Self {
        match err {
            ProviderError::Network(e) => crate::error::AppError::Network(e),
            ProviderError::App(e) => e,
            other => crate::error::AppError::Provider(other.to_string()),
        }
    }
}

#[async_trait]
pub trait ModpackProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn search(&self, query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError>;
    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError>;
    /// Newest first.
    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError>;
    async fn resolve_version(&self, pack_id: &str, version_id: &str) -> Result<ResolvedModpackVersion, ProviderError>;

    /// Release notes of one version as plain text or Markdown; `None` when
    /// the author didn't write any.
    async fn get_changelog(&self, _pack_id: &str, _version_id: &str) -> Result<Option<String>, ProviderError> {
        Ok(None)
    }

    async fn resolve_file_download(&self, file: &ModpackFileRef) -> Result<FileDownloadInfo, ProviderError> {
        match (&file.direct_url, &file.browser_url) {
            (Some(url), _) => Ok(FileDownloadInfo::Direct { url: url.clone() }),
            (None, Some(browser_url)) => Ok(FileDownloadInfo::ManualRequired {
                browser_url: browser_url.clone(),
                expected_filename: file
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            }),
            (None, None) => Err(ProviderError::Other(format!("fichier {} sans URL de téléchargement", file.file_id))),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    #[serde(default)]
    pub offset: u32,
}

/// Scans `(name, version)` pairs — e.g. FTB's `targets` array — and returns
/// the first one whose name [`LoaderKind::from_name`] recognizes.
pub fn find_loader_target<'a>(mut items: impl Iterator<Item = (&'a str, &'a str)>) -> Option<(LoaderKind, String)> {
    items.find_map(|(name, version)| LoaderKind::from_name(name).map(|kind| (kind, version.to_string())))
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn ModpackProvider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_matches_known_loaders_case_insensitively() {
        assert_eq!(LoaderKind::from_name("forge"), Some(LoaderKind::Forge));
        assert_eq!(LoaderKind::from_name("Forge"), Some(LoaderKind::Forge));
        assert_eq!(LoaderKind::from_name("NEOFORGE"), Some(LoaderKind::NeoForge));
        assert_eq!(LoaderKind::from_name("fabric-loader"), Some(LoaderKind::Fabric));
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
        assert!(ProviderRegistry::new().get("ftb").is_none());
    }

    #[test]
    fn find_loader_target_returns_first_recognized_loader() {
        let items = vec![("minecraft", "1.20.1"), ("neoforge", "20.1.80")];
        assert_eq!(find_loader_target(items.into_iter()), Some((LoaderKind::NeoForge, "20.1.80".to_string())));
        let none = vec![("minecraft", "1.20.1"), ("something-else", "1.0")];
        assert_eq!(find_loader_target(none.into_iter()), None);
    }
}
