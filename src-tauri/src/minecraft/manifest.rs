//! Raw JSON shapes for Mojang's `version_manifest_v2.json` and per-version
//! manifests. Fabric/Quilt "profile" responses and Forge/NeoForge-generated
//! `version.json` files reuse this exact schema, so mod loaders parse their
//! own manifests with [`RawVersionJson`] too instead of duplicating it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Deserialize)]
pub struct VersionManifestRoot {
    pub latest: LatestVersions,
    pub versions: Vec<VersionManifestEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

pub async fn fetch_version_manifest(client: &reqwest::Client) -> AppResult<VersionManifestRoot> {
    let root: VersionManifestRoot = client.get(VERSION_MANIFEST_URL).send().await?.json().await?;
    Ok(root)
}

pub async fn find_version_entry(
    client: &reqwest::Client,
    mc_version: &str,
) -> AppResult<VersionManifestEntry> {
    let manifest = fetch_version_manifest(client).await?;
    manifest
        .versions
        .into_iter()
        .find(|v| v.id == mc_version)
        .ok_or_else(|| AppError::Other(format!("unknown Minecraft version: {mc_version}")))
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

pub fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn rule_condition_matches(rule: &Rule) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != current_os_name() {
                return false;
            }
        }
    }
    if let Some(features) = &rule.features {
        // We never opt into any of Mojang's optional features (demo mode,
        // custom resolution, quick play, ...), so a rule that requires one
        // to be `true` can never match.
        if features.values().any(|v| *v) {
            return false;
        }
    }
    true
}

/// Mirrors Mojang's own rule evaluation: default to allowed when there are no
/// rules, otherwise the last matching rule decides.
pub fn rules_allow(rules: &Option<Vec<Rule>>) -> bool {
    match rules {
        None => true,
        Some(rules) => {
            let mut allowed = false;
            for rule in rules {
                if rule_condition_matches(rule) {
                    allowed = rule.action == "allow";
                }
            }
            allowed
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ArgValue {
    Single(String),
    Multi(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ArgEntry {
    Conditional { rules: Vec<Rule>, value: ArgValue },
    Plain(String),
}

pub fn flatten_args(entries: &[ArgEntry]) -> Vec<String> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            ArgEntry::Plain(s) => out.push(s.clone()),
            ArgEntry::Conditional { rules, value } => {
                if rules_allow(&Some(rules.clone())) {
                    match value {
                        ArgValue::Single(s) => out.push(s.clone()),
                        ArgValue::Multi(items) => out.extend(items.clone()),
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RawArguments {
    #[serde(default)]
    pub game: Vec<ArgEntry>,
    #[serde(default)]
    pub jvm: Vec<ArgEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryArtifact {
    #[serde(default)]
    pub path: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<LibraryArtifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, LibraryArtifact>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawLibrary {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    /// Maven repository base URL. Present on every schema variant: as the
    /// artifact's own base repo (vanilla/Forge-style, alongside `downloads`)
    /// or as the *only* location hint (Fabric/Quilt-style, no `downloads` block).
    #[serde(default)]
    pub url: Option<String>,
    /// Only present on Fabric's flat library schema (no `downloads` block);
    /// Quilt's doesn't even have this much.
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub total_size: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DownloadRef {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<DownloadRef>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JavaVersionRef {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawVersionJson {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(default)]
    pub arguments: Option<RawArguments>,
    #[serde(rename = "minecraftArguments", default)]
    pub legacy_arguments: Option<String>,
    #[serde(default)]
    pub libraries: Vec<RawLibrary>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: VersionDownloads,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersionRef>,
    #[serde(default)]
    pub inherits_from: Option<String>,
}

pub async fn fetch_version_json(client: &reqwest::Client, url: &str) -> AppResult<RawVersionJson> {
    let json: RawVersionJson = client.get(url).send().await?.json().await?;
    Ok(json)
}
