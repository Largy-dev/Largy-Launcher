//! Deserialize shapes for the CurseForge Core API (`api.curseforge.com`) and
//! for the `manifest.json` bundled inside a modpack zip.

use serde::Deserialize;

use super::super::{LoaderKind, ModpackSummary};

#[derive(Debug, Deserialize)]
pub(super) struct ListResponse<T> {
    pub(super) data: Vec<T>,
    #[serde(default)]
    pub(super) pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Pagination {
    #[serde(rename = "totalCount", default)]
    pub(super) total_count: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct ItemResponse<T> {
    pub(super) data: T,
}

#[derive(Debug, Deserialize, Clone)]
struct CfLogo {
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct CfAuthor {
    name: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub(super) struct CfLinks {
    #[serde(rename = "websiteUrl", default)]
    pub(super) website_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CfMod {
    pub(super) id: u32,
    name: String,
    pub(super) summary: String,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(rename = "classId", default)]
    pub(super) class_id: Option<u32>,
    #[serde(default)]
    pub(super) links: CfLinks,
    #[serde(rename = "downloadCount", default)]
    download_count: f64,
}

impl CfMod {
    pub(super) fn to_summary(&self) -> ModpackSummary {
        ModpackSummary {
            id: self.id.to_string(),
            provider: "curseforge".to_string(),
            name: self.name.clone(),
            author: self.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            icon_url: self.logo.as_ref().and_then(|l| l.thumbnail_url.clone()),
            summary: self.summary.clone(),
            downloads: Some(self.download_count as u64),
        }
    }

    /// Instance folder a file of this project belongs in.
    pub(super) fn target_folder(&self) -> &'static str {
        match self.class_id {
            Some(12) => "resourcepacks",
            Some(6552) => "shaderpacks",
            _ => "mods",
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CfHash {
    pub(super) value: String,
    pub(super) algo: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CfFile {
    pub(super) id: u32,
    #[serde(rename = "displayName")]
    pub(super) display_name: String,
    #[serde(rename = "fileName")]
    pub(super) file_name: String,
    #[serde(rename = "downloadUrl")]
    pub(super) download_url: Option<String>,
    #[serde(rename = "gameVersions", default)]
    game_versions: Vec<String>,
    #[serde(default)]
    pub(super) hashes: Vec<CfHash>,
    #[serde(rename = "fileLength", default)]
    pub(super) file_length: u64,
    #[serde(rename = "isServerPack", default)]
    pub(super) is_server_pack: Option<bool>,
}

impl CfFile {
    pub(super) fn minecraft_version(&self) -> Option<String> {
        self.game_versions
            .iter()
            .find(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .cloned()
    }

    /// A file's `gameVersions` tags aren't mutually exclusive: many 1.20.1+
    /// packs are tagged both `Forge` and `NeoForge`. A pack actually built
    /// for NeoForge is often *also* tagged `Forge`, but the reverse
    /// essentially never happens — so prefer the more specific loader.
    pub(super) fn loader(&self) -> Option<(LoaderKind, String)> {
        const PRIORITY: [LoaderKind; 4] = [LoaderKind::NeoForge, LoaderKind::Quilt, LoaderKind::Fabric, LoaderKind::Forge];
        PRIORITY
            .into_iter()
            .find(|kind| self.game_versions.iter().any(|v| LoaderKind::from_name(v) == Some(*kind)))
            .map(|kind| (kind, String::new()))
    }

    /// CurseForge's algo 1 is SHA-1.
    pub(super) fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.to_lowercase())
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CfManifest {
    pub(super) minecraft: CfManifestMinecraft,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) files: Vec<CfManifestFile>,
    #[serde(default = "default_overrides")]
    pub(super) overrides: String,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Debug, Deserialize)]
pub(super) struct CfManifestMinecraft {
    pub(super) version: String,
    #[serde(rename = "modLoaders", default)]
    pub(super) mod_loaders: Vec<CfManifestLoader>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CfManifestLoader {
    pub(super) id: String,
    #[serde(default)]
    pub(super) primary: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CfManifestFile {
    #[serde(rename = "projectID")]
    pub(super) project_id: u32,
    #[serde(rename = "fileID")]
    pub(super) file_id: u32,
    #[serde(default = "default_true")]
    pub(super) required: bool,
}

fn default_true() -> bool {
    true
}

pub(super) fn parse_loader_id(id: &str) -> Option<(LoaderKind, String)> {
    let (name, version) = id.split_once('-')?;
    LoaderKind::from_name(name).map(|kind| (kind, version.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(game_versions: &[&str]) -> CfFile {
        CfFile {
            id: 1,
            display_name: "Test".to_string(),
            file_name: "test.zip".to_string(),
            download_url: None,
            game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
            hashes: vec![CfHash { value: "ABC".to_string(), algo: 1 }, CfHash { value: "md5".to_string(), algo: 2 }],
            file_length: 10,
            is_server_pack: None,
        }
    }

    #[test]
    fn parse_loader_id_splits_name_and_version() {
        assert_eq!(parse_loader_id("forge-47.2.20"), Some((LoaderKind::Forge, "47.2.20".to_string())));
        assert_eq!(parse_loader_id("neoforge-20.4.190"), Some((LoaderKind::NeoForge, "20.4.190".to_string())));
        assert_eq!(parse_loader_id("fabric-0.15.11"), Some((LoaderKind::Fabric, "0.15.11".to_string())));
    }

    #[test]
    fn parse_loader_id_rejects_unknown_loader_or_missing_dash() {
        assert_eq!(parse_loader_id("optifine-1.0"), None);
        assert_eq!(parse_loader_id("forge"), None);
    }

    #[test]
    fn cf_file_minecraft_version_and_loader() {
        let f = file(&["Forge", "1.20.1", "Client"]);
        assert_eq!(f.minecraft_version(), Some("1.20.1".to_string()));
        assert_eq!(f.loader(), Some((LoaderKind::Forge, String::new())));
        assert_eq!(f.sha1().as_deref(), Some("abc"));
    }

    #[test]
    fn cf_file_loader_prefers_neoforge_when_a_file_is_tagged_both() {
        assert_eq!(file(&["1.20.1", "Forge", "NeoForge"]).loader(), Some((LoaderKind::NeoForge, String::new())));
    }

    #[test]
    fn cf_manifest_deserializes_from_real_shape() {
        let json = r#"{
            "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.20", "primary": true}]},
            "name": "My Pack",
            "files": [{"projectID": 123, "fileID": 456, "required": true}]
        }"#;
        let manifest: CfManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.minecraft.version, "1.20.1");
        assert_eq!(manifest.files[0].project_id, 123);
        assert_eq!(manifest.overrides, "overrides");
        assert_eq!(manifest.name.as_deref(), Some("My Pack"));
    }
}
