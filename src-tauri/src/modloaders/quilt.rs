//! Quilt loader: meta.quiltmc.org mirrors Fabric's v2 meta API shape (v3
//! path, identical fields), so this is Fabric's installer with different URLs.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::minecraft::manifest::RawVersionJson;
use crate::providers::LoaderKind;

use super::{loader_profile_from_delta_json, LoaderError, LoaderInstaller, LoaderProfile};

const META_BASE: &str = "https://meta.quiltmc.org/v3/versions/loader";

#[derive(Debug, Deserialize)]
struct LoaderVersionEntry {
    loader: LoaderVersionInfo,
}

#[derive(Debug, Deserialize)]
struct LoaderVersionInfo {
    version: String,
}

/// Split out from [`QuiltInstaller::list_versions`] so the response-to-version-list
/// mapping is testable without a network round-trip.
fn versions(entries: Vec<LoaderVersionEntry>) -> Vec<String> {
    entries.into_iter().map(|e| e.loader.version).collect()
}

pub struct QuiltInstaller {
    client: reqwest::Client,
    downloader: DownloadManager,
    libraries_dir: PathBuf,
}

impl QuiltInstaller {
    pub fn new(client: reqwest::Client, libraries_dir: PathBuf) -> Self {
        Self {
            downloader: DownloadManager::new(client.clone()),
            client,
            libraries_dir,
        }
    }
}

#[async_trait]
impl LoaderInstaller for QuiltInstaller {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Quilt
    }

    async fn list_versions(&self, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let entries: Vec<LoaderVersionEntry> = self
            .client
            .get(format!("{META_BASE}/{mc_version}"))
            .send()
            .await?
            .json()
            .await?;
        Ok(versions(entries))
    }

    async fn resolve(
        &self,
        app: &AppHandle,
        mc_version: &str,
        loader_version: &str,
    ) -> Result<LoaderProfile, LoaderError> {
        let url = format!("{META_BASE}/{mc_version}/{loader_version}/profile/json");
        let raw: RawVersionJson = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| LoaderError::Other(format!("version Quilt introuvable: {e}")))?
            .json()
            .await?;

        loader_profile_from_delta_json(app, &self.downloader, &raw, &self.libraries_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_maps_every_entry_to_its_loader_version() {
        let entries = vec![
            LoaderVersionEntry { loader: LoaderVersionInfo { version: "0.24.0".to_string() } },
            LoaderVersionEntry { loader: LoaderVersionInfo { version: "0.23.1".to_string() } },
        ];
        assert_eq!(versions(entries), vec!["0.24.0".to_string(), "0.23.1".to_string()]);
    }
}
