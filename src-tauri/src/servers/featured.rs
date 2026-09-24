//! The "Serveurs" catalog: popular servers with the Minecraft version to
//! play them on and optional client-mod presets. Read from the repository's
//! `servers/featured.json` so servers can be added without a release; the
//! copy built into the app is used when GitHub can't be reached.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const REMOTE_URL: &str = "https://raw.githubusercontent.com/Largy-dev/Largy-Launcher/main/servers/featured.json";
const BUNDLED: &str = include_str!("../../../servers/featured.json");
/// Catalog format this build understands; newer remote files are ignored.
const SCHEMA: u32 = 1;
/// Mods and shaders one preparation may install, all presets included.
pub const MAX_PROJECTS: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub default: bool,
    /// Modrinth project slugs (Fabric mods).
    #[serde(default)]
    pub mods: Vec<String>,
    /// Modrinth shader pack slugs; the first one is switched on.
    #[serde(default)]
    pub shaders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeaturedServer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub language: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub website: Option<String>,
    /// Mods the server itself needs (e.g. a voice chat), always installed.
    #[serde(default)]
    pub required_mods: Vec<String>,
    /// Modded servers: the exact modpack version to install instead of the
    /// Fabric presets.
    #[serde(default)]
    pub modpack: Option<ServerModpack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerModpack {
    /// `ftb`, `modrinth` or `curseforge`.
    pub provider: String,
    pub pack_id: String,
    pub version_id: String,
    pub name: String,
    /// Version as the pack author names it (e.g. `1.18.1`).
    pub version_name: String,
}

impl ServerModpack {
    fn is_valid(&self) -> bool {
        let id = |v: &str| {
            (1..=64).contains(&v.len()) && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        };
        ["ftb", "modrinth", "curseforge"].contains(&self.provider.as_str())
            && id(&self.pack_id)
            && id(&self.version_id)
            && !self.name.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Catalog {
    pub schema: u32,
    pub presets: Vec<Preset>,
    pub servers: Vec<FeaturedServer>,
}

/// A Modrinth slug: lowercase letters, digits, `-` and `_`.
pub fn is_slug(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// `1.21.4`, `26.3`, `1.20.1-rc1`… — what Mojang and Modrinth call versions.
pub fn is_minecraft_version(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.chars().next().is_some_and(|c| c.is_ascii_digit())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

impl Catalog {
    /// Parses a catalog, dropping entries that don't hold up (bad address,
    /// version or slug) rather than rejecting the whole file.
    pub fn parse(json: &str) -> AppResult<Catalog> {
        let mut catalog: Catalog =
            serde_json::from_str(json).map_err(|e| AppError::Other(format!("liste de serveurs illisible : {e}")))?;
        if catalog.schema > SCHEMA {
            return Err(AppError::Other("liste de serveurs trop récente pour cette version".to_string()));
        }
        catalog.presets.retain(|p| {
            is_slug(&p.id) && p.mods.iter().chain(&p.shaders).all(|s| is_slug(s))
        });
        catalog.servers.retain(|s| {
            is_slug(&s.id)
                && super::validate_address(&s.address).is_ok()
                && is_minecraft_version(&s.minecraft_version)
                && s.required_mods.iter().all(|m| is_slug(m))
                && s.website.as_deref().is_none_or(|w| w.starts_with("https://"))
                && s.modpack.as_ref().is_none_or(ServerModpack::is_valid)
        });
        Ok(catalog)
    }

    pub fn bundled() -> Catalog {
        Catalog::parse(BUNDLED).expect("the bundled servers/featured.json must be valid")
    }
}

/// The online catalog, or the bundled one when it can't be fetched or read.
pub async fn load(client: &reqwest::Client) -> Catalog {
    let fetched = async {
        let response = client.get(REMOTE_URL).timeout(Duration::from_secs(8)).send().await.ok()?;
        let text = response.error_for_status().ok()?.text().await.ok()?;
        Catalog::parse(&text).ok().filter(|c| !c.servers.is_empty())
    };
    match fetched.await {
        Some(catalog) => catalog,
        None => {
            tracing::info!("using the bundled server catalog");
            Catalog::bundled()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_catalog_is_valid_and_complete() {
        let raw: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let catalog = Catalog::bundled();
        assert_eq!(catalog.servers.len(), raw["servers"].as_array().unwrap().len(), "no bundled server dropped");
        assert_eq!(catalog.presets.len(), raw["presets"].as_array().unwrap().len(), "no bundled preset dropped");
        let mut ids: Vec<_> = catalog.servers.iter().map(|s| &s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), catalog.servers.len(), "server ids are unique");
    }

    #[test]
    fn invalid_entries_are_dropped_not_fatal() {
        let json = r#"{
            "schema": 1,
            "presets": [{ "id": "bad", "label": "x", "description": "", "mods": ["../evil"] }],
            "servers": [
                { "id": "ok", "name": "Ok", "address": "play.ok.net", "description": "", "minecraft_version": "1.21.4" },
                { "id": "bad-address", "name": "B", "address": "a b", "description": "", "minecraft_version": "1.21.4" },
                { "id": "bad-site", "name": "C", "address": "c.net", "description": "", "minecraft_version": "1.21.4",
                  "website": "javascript:alert(1)" }
            ]
        }"#;
        let catalog = Catalog::parse(json).unwrap();
        assert!(catalog.presets.is_empty());
        assert_eq!(catalog.servers.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["ok"]);
    }

    #[test]
    fn modpack_servers_need_a_known_provider_and_plain_ids() {
        let server = |provider: &str, version: &str| {
            format!(
                r#"{{ "id": "s", "name": "S", "address": "s.net", "description": "", "minecraft_version": "1.21.1",
                     "modpack": {{ "provider": "{provider}", "pack_id": "126", "version_id": "{version}",
                                   "name": "Pack", "version_name": "1.0" }} }}"#
            )
        };
        let catalog = |entry: String| Catalog::parse(&format!(r#"{{ "schema": 1, "presets": [], "servers": [{entry}] }}"#));
        assert_eq!(catalog(server("ftb", "100337")).unwrap().servers.len(), 1);
        assert!(catalog(server("evil", "100337")).unwrap().servers.is_empty());
        assert!(catalog(server("ftb", "../x")).unwrap().servers.is_empty());
    }

    #[test]
    fn newer_schemas_are_refused() {
        assert!(Catalog::parse(r#"{ "schema": 99, "presets": [], "servers": [] }"#).is_err());
    }

    #[test]
    fn slugs_and_versions() {
        assert!(is_slug("ferrite-core") && is_slug("sodium"));
        assert!(!is_slug("Sodium") && !is_slug("a/b") && !is_slug(""));
        assert!(is_minecraft_version("26.3") && is_minecraft_version("1.21.11") && is_minecraft_version("1.20.1-rc1"));
        assert!(!is_minecraft_version("latest") && !is_minecraft_version("../1.21"));
    }
}
