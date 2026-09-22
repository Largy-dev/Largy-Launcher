//! Deserialize shapes for the CurseForge Core API (`api.curseforge.com`) and
//! for the `manifest.json` bundled inside a modpack zip.

use serde::Deserialize;

use super::super::{LoaderKind, ModpackSummary};

#[derive(Debug, Deserialize)]
pub(super) struct SearchResponse {
    pub(super) data: Vec<CfMod>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ModResponse {
    pub(super) data: CfMod,
}

#[derive(Debug, Deserialize)]
pub(super) struct FilesResponse {
    pub(super) data: Vec<CfFile>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FileResponse {
    pub(super) data: CfFile,
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

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CfMod {
    id: u32,
    name: String,
    pub(super) summary: String,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
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
        }
    }
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
}

impl CfFile {
    pub(super) fn minecraft_version(&self) -> Option<String> {
        self.game_versions
            .iter()
            .find(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .cloned()
    }

    /// A file's `gameVersions` tags aren't mutually exclusive: many 1.20.1+
    /// packs are tagged both `Forge` and `NeoForge` since NeoForge stayed
    /// binary-compatible with Forge mods for a while, and authors tag both
    /// for search visibility. When several loader tags are present, prefer
    /// the more specific/newer one — a pack actually built for NeoForge is
    /// often *also* tagged `Forge`, but the reverse (a real Forge pack tagged
    /// `NeoForge`) essentially never happens. Real per-loader files are the
    /// normal case and only ever carry one tag, so this ordering doesn't
    /// affect them.
    pub(super) fn loader(&self) -> Option<(LoaderKind, String)> {
        const PRIORITY: [LoaderKind; 4] =
            [LoaderKind::NeoForge, LoaderKind::Quilt, LoaderKind::Fabric, LoaderKind::Forge];
        PRIORITY
            .into_iter()
            .find(|kind| self.game_versions.iter().any(|v| LoaderKind::from_name(v) == Some(*kind)))
            .map(|kind| (kind, String::new()))
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CfManifest {
    pub(super) minecraft: CfManifestMinecraft,
    #[serde(default)]
    pub(super) files: Vec<CfManifestFile>,
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

    #[test]
    fn parse_loader_id_splits_name_and_version() {
        assert_eq!(
            parse_loader_id("forge-47.2.20"),
            Some((LoaderKind::Forge, "47.2.20".to_string()))
        );
        assert_eq!(
            parse_loader_id("neoforge-20.4.190"),
            Some((LoaderKind::NeoForge, "20.4.190".to_string()))
        );
    }

    #[test]
    fn parse_loader_id_rejects_unknown_loader_or_missing_dash() {
        assert_eq!(parse_loader_id("optifine-1.0"), None);
        assert_eq!(parse_loader_id("forge"), None);
    }

    #[test]
    fn cf_file_minecraft_version_picks_first_numeric_game_version() {
        let file = CfFile {
            id: 1,
            display_name: "Test".to_string(),
            file_name: "test.zip".to_string(),
            download_url: None,
            game_versions: vec!["Forge".to_string(), "1.20.1".to_string(), "Client".to_string()],
        };
        assert_eq!(file.minecraft_version(), Some("1.20.1".to_string()));
        assert_eq!(file.loader(), Some((LoaderKind::Forge, String::new())));
    }

    #[test]
    fn cf_file_loader_prefers_neoforge_when_a_file_is_tagged_both() {
        // Real-world case: a 1.20.1 modpack file tagged both `Forge` and
        // `NeoForge` for search visibility is a NeoForge pack, not Forge.
        let file = CfFile {
            id: 1,
            display_name: "Test".to_string(),
            file_name: "test.zip".to_string(),
            download_url: None,
            game_versions: vec!["1.20.1".to_string(), "Forge".to_string(), "NeoForge".to_string()],
        };
        assert_eq!(file.loader(), Some((LoaderKind::NeoForge, String::new())));
    }

    #[test]
    fn cf_manifest_deserializes_from_real_shape() {
        let json = r#"{
            "minecraft": {
                "version": "1.20.1",
                "modLoaders": [{"id": "forge-47.2.20", "primary": true}]
            },
            "files": [{"projectID": 123, "fileID": 456, "required": true}]
        }"#;
        let manifest: CfManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.minecraft.version, "1.20.1");
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].project_id, 123);
        let (loader, version) = parse_loader_id(&manifest.minecraft.mod_loaders[0].id).unwrap();
        assert_eq!(loader, LoaderKind::Forge);
        assert_eq!(version, "47.2.20");
    }
}
