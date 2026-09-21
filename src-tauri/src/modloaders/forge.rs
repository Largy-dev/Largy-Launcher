//! Minecraft Forge. Version discovery uses the `promotions_slim.json` feed
//! (recommended/latest per Minecraft version); installing delegates to the
//! shared [`forge_common`] processor-chain runner.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::java::JavaManager;
use crate::paths::AppPaths;
use crate::providers::LoaderKind;

use super::forge_common::install_from_installer_jar;
use super::{LoaderError, LoaderInstaller, LoaderProfile};

const PROMOTIONS_URL: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

#[derive(Debug, Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

pub struct ForgeInstaller {
    client: reqwest::Client,
    downloader: DownloadManager,
    java: JavaManager,
    paths: AppPaths,
}

impl ForgeInstaller {
    pub fn new(client: reqwest::Client, java: JavaManager, paths: AppPaths) -> Self {
        Self {
            downloader: DownloadManager::new(client.clone()),
            client,
            java,
            paths,
        }
    }

    fn installer_url(mc_version: &str, forge_version: &str) -> String {
        format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{forge_version}/forge-{mc_version}-{forge_version}-installer.jar"
        )
    }
}

#[async_trait]
impl LoaderInstaller for ForgeInstaller {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Forge
    }

    async fn list_versions(&self, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let promotions: Promotions = self.client.get(PROMOTIONS_URL).send().await?.json().await?;
        let prefix = format!("{mc_version}-");
        let mut versions: Vec<String> = promotions
            .promos
            .into_iter()
            .filter_map(|(key, version)| key.starts_with(&prefix).then_some(version))
            .collect();
        versions.sort();
        versions.dedup();
        versions.reverse();
        if versions.is_empty() {
            return Err(LoaderError::UnsupportedVersion(format!(
                "aucune version Forge trouvée pour Minecraft {mc_version}"
            )));
        }
        Ok(versions)
    }

    async fn resolve(
        &self,
        app: &AppHandle,
        mc_version: &str,
        loader_version: &str,
    ) -> Result<LoaderProfile, LoaderError> {
        let installer_url = Self::installer_url(mc_version, loader_version);
        let cache_key = format!("forge-{mc_version}-{loader_version}");
        install_from_installer_jar(
            app,
            &self.client,
            &self.downloader,
            &self.java,
            &self.paths,
            mc_version,
            &installer_url,
            &cache_key,
        )
        .await
    }
}
