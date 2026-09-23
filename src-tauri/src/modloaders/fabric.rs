//! Fabric and Quilt: both publish a thin meta API (`meta.fabricmc.net/v2`,
//! `meta.quiltmc.org/v3`, same response shape) — no installer jar, no
//! processors, just a version-json delta to merge on top of vanilla.

use async_trait::async_trait;
use serde::Deserialize;

use crate::minecraft::manifest::RawVersionJson;
use crate::providers::LoaderKind;
use crate::util::http_cache::{MetaCache, HOURLY, IMMUTABLE};

use super::{loader_profile_from_delta_json, LoaderContext, LoaderError, LoaderInstaller, LoaderProfile};

#[derive(Debug, Deserialize)]
struct LoaderVersionEntry {
    loader: LoaderVersionInfo,
}

#[derive(Debug, Deserialize)]
struct LoaderVersionInfo {
    version: String,
    /// Fabric flags pre-releases; Quilt omits the field (and marks betas in
    /// the version string instead).
    #[serde(default)]
    stable: Option<bool>,
}

/// Stable releases only when the meta says which ones are stable; otherwise
/// drops versions carrying a pre-release suffix.
/// Falls back to every version when a Minecraft release only has pre-releases.
fn stable_versions(entries: Vec<LoaderVersionEntry>) -> Vec<String> {
    let is_stable = |e: &LoaderVersionEntry| match e.loader.stable {
        Some(stable) => stable,
        None => !e.loader.version.contains('-'),
    };
    if entries.iter().any(is_stable) {
        entries.into_iter().filter(is_stable).map(|e| e.loader.version).collect()
    } else {
        entries.into_iter().map(|e| e.loader.version).collect()
    }
}

pub struct MetaLoaderInstaller {
    kind: LoaderKind,
    base: &'static str,
    label: &'static str,
}

impl MetaLoaderInstaller {
    pub fn fabric() -> Self {
        Self { kind: LoaderKind::Fabric, base: "https://meta.fabricmc.net/v2/versions/loader", label: "Fabric" }
    }

    pub fn quilt() -> Self {
        Self { kind: LoaderKind::Quilt, base: "https://meta.quiltmc.org/v3/versions/loader", label: "Quilt" }
    }
}

#[async_trait]
impl LoaderInstaller for MetaLoaderInstaller {
    fn kind(&self) -> LoaderKind {
        self.kind
    }

    async fn list_versions(&self, meta: &MetaCache, mc_version: &str) -> Result<Vec<String>, LoaderError> {
        let entries: Vec<LoaderVersionEntry> = meta.get_json(&format!("{}/{mc_version}", self.base), HOURLY).await?;
        let versions = stable_versions(entries);
        if versions.is_empty() {
            return Err(LoaderError::UnsupportedVersion(format!(
                "aucune version {} pour Minecraft {mc_version}",
                self.label
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
        let url = format!("{}/{mc_version}/{loader_version}/profile/json", self.base);
        let raw: RawVersionJson = ctx
            .meta
            .get_json(&url, IMMUTABLE)
            .await
            .map_err(|e| LoaderError::Other(format!("version {} {loader_version} introuvable: {e}", self.label)))?;
        loader_profile_from_delta_json(ctx, &raw).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(version: &str, stable: Option<bool>) -> LoaderVersionEntry {
        LoaderVersionEntry { loader: LoaderVersionInfo { version: version.to_string(), stable } }
    }

    #[test]
    fn stable_versions_uses_the_stable_flag_when_present() {
        let entries = vec![entry("0.16.0", Some(true)), entry("0.17.0", Some(false)), entry("0.15.11", Some(true))];
        assert_eq!(stable_versions(entries), vec!["0.16.0".to_string(), "0.15.11".to_string()]);
    }

    #[test]
    fn stable_versions_drops_prerelease_suffixes_without_a_flag() {
        let entries = vec![entry("0.26.0-beta.1", None), entry("0.25.0", None)];
        assert_eq!(stable_versions(entries), vec!["0.25.0".to_string()]);
    }
}
