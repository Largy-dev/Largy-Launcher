//! NeoForge: a Forge fork that reuses the exact same installer/processor
//! technology (see [`forge_common`]), but versions independently of
//! Minecraft — a version like `20.4.190` targets Minecraft `1.20.4`.

use async_trait::async_trait;
use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::java::JavaManager;
use crate::paths::AppPaths;
use crate::providers::LoaderKind;

use super::forge_common::install_from_installer_jar;
use super::{LoaderError, LoaderInstaller, LoaderProfile};

const METADATA_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

pub struct NeoForgeInstaller {
    client: reqwest::Client,
    downloader: DownloadManager,
    java: JavaManager,
    paths: AppPaths,
}

impl NeoForgeInstaller {
    pub fn new(client: reqwest::Client, java: JavaManager, paths: AppPaths) -> Self {
        Self {
            downloader: DownloadManager::new(client.clone()),
            client,
            java,
            paths,
        }
    }

    fn installer_url(neoforge_version: &str) -> String {
        format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{neoforge_version}/neoforge-{neoforge_version}-installer.jar"
        )
    }

    /// `1.20.4` -> `20.4.` (NeoForge drops the leading `1.` from Minecraft's
    /// own version and versions itself `<minor>.<patch>.<build>`).
    fn version_prefix(mc_version: &str) -> Option<String> {
        let rest = mc_version.strip_prefix("1.")?;
        let mut parts = rest.split('.');
        let minor = parts.next()?;
        let patch = parts.next().unwrap_or("0");
        Some(format!("{minor}.{patch}."))
    }

    fn extract_versions(xml: &str) -> Vec<String> {
        xml.split("<version>")
            .skip(1)
            .filter_map(|chunk| chunk.split("</version>").next())
            .map(|s| s.trim().to_string())
            .collect()
    }
}

#[async_trait]
impl LoaderInstaller for NeoForgeInstaller {
    fn kind(&self) -> LoaderKind {
        LoaderKind::NeoForge
    }

    async fn list_versions(&self, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let prefix = Self::version_prefix(mc_version).ok_or_else(|| {
            LoaderError::UnsupportedVersion(format!("NeoForge ne supporte pas Minecraft {mc_version}"))
        })?;

        let xml = self.client.get(METADATA_URL).send().await?.text().await?;
        let mut versions: Vec<String> = Self::extract_versions(&xml)
            .into_iter()
            .filter(|v| v.starts_with(&prefix))
            .collect();
        versions.sort();
        versions.reverse();

        if versions.is_empty() {
            return Err(LoaderError::UnsupportedVersion(format!(
                "aucune version NeoForge trouvée pour Minecraft {mc_version}"
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
        let installer_url = Self::installer_url(loader_version);
        let cache_key = format!("neoforge-{loader_version}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_prefix_drops_leading_1_and_keeps_minor_patch() {
        assert_eq!(NeoForgeInstaller::version_prefix("1.20.4"), Some("20.4.".to_string()));
        assert_eq!(NeoForgeInstaller::version_prefix("1.21"), Some("21.0.".to_string()));
    }

    #[test]
    fn version_prefix_rejects_versions_not_starting_with_1_dot() {
        assert_eq!(NeoForgeInstaller::version_prefix("2.0"), None);
        assert_eq!(NeoForgeInstaller::version_prefix(""), None);
    }

    #[test]
    fn extract_versions_parses_maven_metadata_xml() {
        let xml = "<metadata><versioning><versions>\
                     <version>20.4.190</version>\
                     <version>20.4.191</version>\
                   </versions></versioning></metadata>";
        assert_eq!(
            NeoForgeInstaller::extract_versions(xml),
            vec!["20.4.190".to_string(), "20.4.191".to_string()]
        );
    }

    #[test]
    fn extract_versions_returns_empty_for_no_matches() {
        assert!(NeoForgeInstaller::extract_versions("<metadata></metadata>").is_empty());
    }

    #[test]
    fn installer_url_matches_neoforged_maven_layout() {
        assert_eq!(
            NeoForgeInstaller::installer_url("20.4.190"),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/20.4.190/neoforge-20.4.190-installer.jar"
        );
    }
}
