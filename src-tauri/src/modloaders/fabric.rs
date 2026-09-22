//! Fabric loader: a thin, well-documented JSON API at meta.fabricmc.net —
//! no installer jar, no processors, just a version-json delta to merge on
//! top of vanilla.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::minecraft::manifest::RawVersionJson;
use crate::providers::LoaderKind;

use super::{loader_profile_from_delta_json, LoaderError, LoaderInstaller, LoaderProfile};

const META_BASE: &str = "https://meta.fabricmc.net/v2/versions/loader";

#[derive(Debug, Deserialize)]
struct LoaderVersionEntry {
    loader: LoaderVersionInfo,
}

#[derive(Debug, Deserialize)]
struct LoaderVersionInfo {
    version: String,
    stable: bool,
}

/// Keeps only stable loader releases. Split out from
/// [`FabricInstaller::list_versions`] so this filter is testable without a
/// network round-trip.
fn stable_versions(entries: Vec<LoaderVersionEntry>) -> Vec<String> {
    entries.into_iter().filter(|e| e.loader.stable).map(|e| e.loader.version).collect()
}

pub struct FabricInstaller {
    client: reqwest::Client,
    downloader: DownloadManager,
    libraries_dir: PathBuf,
}

impl FabricInstaller {
    pub fn new(client: reqwest::Client, libraries_dir: PathBuf) -> Self {
        Self {
            downloader: DownloadManager::new(client.clone()),
            client,
            libraries_dir,
        }
    }
}

#[async_trait]
impl LoaderInstaller for FabricInstaller {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Fabric
    }

    async fn list_versions(&self, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let entries: Vec<LoaderVersionEntry> = self
            .client
            .get(format!("{META_BASE}/{mc_version}"))
            .send()
            .await?
            .json()
            .await?;
        Ok(stable_versions(entries))
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
            .map_err(|e| LoaderError::Other(format!("version Fabric introuvable: {e}")))?
            .json()
            .await?;

        loader_profile_from_delta_json(app, &self.downloader, &raw, &self.libraries_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(version: &str, stable: bool) -> LoaderVersionEntry {
        LoaderVersionEntry {
            loader: LoaderVersionInfo { version: version.to_string(), stable },
        }
    }

    #[test]
    fn stable_versions_keeps_only_stable_entries_in_order() {
        let entries = vec![entry("0.16.0", true), entry("0.17.0-beta", false), entry("0.15.11", true)];
        assert_eq!(stable_versions(entries), vec!["0.16.0".to_string(), "0.15.11".to_string()]);
    }

    #[test]
    fn stable_versions_returns_empty_when_none_are_stable() {
        let entries = vec![entry("0.17.0-beta", false)];
        assert!(stable_versions(entries).is_empty());
    }
}
