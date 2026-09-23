//! Minecraft Forge. Every published build is listed from the Maven metadata
//! (the promotions feed only has "latest"/"recommended"), newest first with
//! the recommended build of the Minecraft version on top.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;

use crate::providers::LoaderKind;
use crate::util::http_cache::{MetaCache, HOURLY};
use crate::util::version::sort_desc;

use super::forge_common::{install_from_installer_jar, maven_versions};
use super::{LoaderContext, LoaderError, LoaderInstaller, LoaderProfile};

const METADATA_URL: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const PROMOTIONS_URL: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

#[derive(Debug, Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

pub struct ForgeInstaller;

impl ForgeInstaller {
    fn installer_url(mc_version: &str, forge_version: &str) -> String {
        format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{forge_version}/forge-{mc_version}-{forge_version}-installer.jar"
        )
    }
}

/// Forge builds for `mc_version` out of full `<mc>-<forge>` maven versions,
/// newest first, with `recommended` (if any) moved to the front.
fn versions_for(all: Vec<String>, mc_version: &str, recommended: Option<&str>) -> Vec<String> {
    let prefix = format!("{mc_version}-");
    let mut versions: Vec<String> = all.into_iter().filter_map(|v| v.strip_prefix(&prefix).map(String::from)).collect();
    sort_desc(&mut versions);
    if let Some(rec) = recommended {
        if let Some(pos) = versions.iter().position(|v| v == rec) {
            let v = versions.remove(pos);
            versions.insert(0, v);
        }
    }
    versions
}

#[async_trait]
impl LoaderInstaller for ForgeInstaller {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Forge
    }

    async fn list_versions(&self, meta: &MetaCache, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let xml = meta.get_text(METADATA_URL, HOURLY).await?;
        let recommended = meta
            .get_json::<Promotions>(PROMOTIONS_URL, HOURLY)
            .await
            .ok()
            .and_then(|p| p.promos.get(&format!("{mc_version}-recommended")).cloned());
        let versions = versions_for(maven_versions(&xml), mc_version, recommended.as_deref());
        if versions.is_empty() {
            return Err(LoaderError::UnsupportedVersion(format!(
                "aucune version Forge trouvée pour Minecraft {mc_version}"
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
        let installer_url = Self::installer_url(mc_version, loader_version);
        let cache_key = format!("forge-{mc_version}-{loader_version}");
        install_from_installer_jar(ctx, mc_version, &installer_url, &cache_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_url_matches_minecraftforge_maven_layout() {
        assert_eq!(
            ForgeInstaller::installer_url("1.20.1", "47.2.20"),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.20/forge-1.20.1-47.2.20-installer.jar"
        );
    }

    #[test]
    fn versions_for_filters_sorts_numerically_and_promotes_recommended() {
        let all = vec![
            "1.20.1-47.2.9".to_string(),
            "1.20.1-47.2.20".to_string(),
            "1.20.1-47.1.3".to_string(),
            "1.20.10-99.0.0".to_string(),
            "1.7.10-10.13.4.1614-1.7.10".to_string(),
        ];
        assert_eq!(versions_for(all.clone(), "1.20.1", Some("47.1.3")), vec!["47.1.3", "47.2.20", "47.2.9"]);
        assert_eq!(versions_for(all, "1.7.10", None), vec!["10.13.4.1614-1.7.10"]);
    }
}
