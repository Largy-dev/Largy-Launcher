//! NeoForge: a Forge fork that reuses the same installer/processor
//! technology (see [`forge_common`]). Versions track Minecraft's own number
//! without its leading `1.` (`20.4.190` → Minecraft 1.20.4), or its full
//! number for year-based releases (`26.1.0.x` → Minecraft 26.1). Minecraft
//! 1.20.1 is the exception: NeoForge shipped there under the old
//! `net.neoforged:forge` artifact, versioned `1.20.1-47.1.x`.

use async_trait::async_trait;

use crate::providers::LoaderKind;
use crate::util::http_cache::{MetaCache, HOURLY};
use crate::util::version::sort_desc;

use super::forge_common::{install_from_installer_jar, maven_versions};
use super::{LoaderContext, LoaderError, LoaderInstaller, LoaderProfile};

const METADATA_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const LEGACY_METADATA_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml";
const LEGACY_MC: &str = "1.20.1";

pub struct NeoForgeInstaller;

impl NeoForgeInstaller {
    fn installer_url(neoforge_version: &str) -> String {
        if neoforge_version.starts_with("1.20.1-") {
            return format!(
                "https://maven.neoforged.net/releases/net/neoforged/forge/{neoforge_version}/forge-{neoforge_version}-installer.jar"
            );
        }
        format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{neoforge_version}/neoforge-{neoforge_version}-installer.jar"
        )
    }

    /// `1.20.4` -> `20.4.`, `1.21` -> `21.0.`, `26.1` -> `26.1.0.`, `26.1.2` -> `26.1.2.`.
    fn version_prefix(mc_version: &str) -> Option<String> {
        if let Some(rest) = mc_version.strip_prefix("1.") {
            let mut parts = rest.split('.');
            let minor = parts.next().filter(|p| !p.is_empty())?;
            let patch = parts.next().unwrap_or("0");
            return Some(format!("{minor}.{patch}."));
        }
        let parts: Vec<&str> = mc_version.split('.').collect();
        if parts.len() < 2 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
            return None;
        }
        Some(format!("{}.{}.{}.", parts[0], parts[1], parts.get(2).unwrap_or(&"0")))
    }

    /// Loader version as stored on an instance -> the maven version to install.
    fn maven_version(mc_version: &str, loader_version: &str) -> String {
        if mc_version == LEGACY_MC && !loader_version.starts_with("1.20.1-") {
            format!("{LEGACY_MC}-{loader_version}")
        } else {
            loader_version.to_string()
        }
    }
}

#[async_trait]
impl LoaderInstaller for NeoForgeInstaller {
    fn kind(&self) -> LoaderKind {
        LoaderKind::NeoForge
    }

    async fn list_versions(&self, meta: &MetaCache, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let mut versions: Vec<String> = if mc_version == LEGACY_MC {
            let xml = meta.get_text(LEGACY_METADATA_URL, HOURLY).await?;
            maven_versions(&xml).into_iter().filter(|v| v.starts_with("1.20.1-")).collect()
        } else {
            let prefix = Self::version_prefix(mc_version).ok_or_else(|| {
                LoaderError::UnsupportedVersion(format!("NeoForge ne supporte pas Minecraft {mc_version}"))
            })?;
            let xml = meta.get_text(METADATA_URL, HOURLY).await?;
            maven_versions(&xml).into_iter().filter(|v| v.starts_with(&prefix)).collect()
        };
        sort_desc(&mut versions);

        if versions.is_empty() {
            return Err(LoaderError::UnsupportedVersion(format!(
                "aucune version NeoForge trouvée pour Minecraft {mc_version}"
            )));
        }
        Ok(versions)
    }

    async fn resolve(
        &self,
        ctx: &LoaderContext<'_>,
        mc_version: &str,
        loader_version: &str,
    ) -> Result<LoaderProfile, LoaderError> {
        let version = Self::maven_version(mc_version, loader_version);
        let installer_url = Self::installer_url(&version);
        let cache_key = format!("neoforge-{version}");
        install_from_installer_jar(ctx, mc_version, &installer_url, &cache_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_prefix_handles_legacy_and_year_based_minecraft_versions() {
        assert_eq!(NeoForgeInstaller::version_prefix("1.20.4"), Some("20.4.".to_string()));
        assert_eq!(NeoForgeInstaller::version_prefix("1.21"), Some("21.0.".to_string()));
        assert_eq!(NeoForgeInstaller::version_prefix("26.1"), Some("26.1.0.".to_string()));
        assert_eq!(NeoForgeInstaller::version_prefix("26.1.2"), Some("26.1.2.".to_string()));
        assert_eq!(NeoForgeInstaller::version_prefix("24w14a"), None);
        assert_eq!(NeoForgeInstaller::version_prefix(""), None);
    }

    #[test]
    fn installer_url_matches_neoforged_maven_layout() {
        assert_eq!(
            NeoForgeInstaller::installer_url("20.4.190"),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/20.4.190/neoforge-20.4.190-installer.jar"
        );
        assert_eq!(
            NeoForgeInstaller::installer_url("1.20.1-47.1.106"),
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        );
    }

    #[test]
    fn maven_version_prefixes_bare_1_20_1_versions() {
        assert_eq!(NeoForgeInstaller::maven_version("1.20.1", "47.1.106"), "1.20.1-47.1.106");
        assert_eq!(NeoForgeInstaller::maven_version("1.20.1", "1.20.1-47.1.106"), "1.20.1-47.1.106");
        assert_eq!(NeoForgeInstaller::maven_version("1.21.1", "21.1.77"), "21.1.77");
    }
}
