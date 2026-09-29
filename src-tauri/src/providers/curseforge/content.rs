//! CurseForge mods, resource packs and shader packs for one instance:
//! search filtered to its Minecraft version and loader, and the files a
//! project offers for them (newest first).

use super::api_types::{CfFile, CfMod, ListResponse};
use super::{CurseForgeProvider, MINECRAFT_GAME_ID};
use crate::providers::{LoaderKind, ProviderError};

pub const CLASS_MODS: u32 = 6;
pub const CLASS_RESOURCE_PACKS: u32 = 12;
pub const CLASS_SHADERS: u32 = 6552;

/// CurseForge's `modLoaderType` for a loader.
pub fn loader_type(loader: LoaderKind) -> Option<u32> {
    match loader {
        LoaderKind::Vanilla => None,
        LoaderKind::Forge => Some(1),
        LoaderKind::Fabric => Some(4),
        LoaderKind::Quilt => Some(5),
        LoaderKind::NeoForge => Some(6),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CfContentHit {
    pub id: u32,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub website_url: Option<String>,
}

impl From<&CfMod> for CfContentHit {
    fn from(m: &CfMod) -> Self {
        CfContentHit {
            id: m.id,
            slug: m.slug.clone(),
            name: m.name.clone(),
            summary: m.summary.clone(),
            author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone()),
            downloads: m.download_count as u64,
            website_url: m.links.website_url.clone(),
        }
    }
}

/// One downloadable file of a project.
#[derive(Debug, Clone, PartialEq)]
pub struct CfContentFile {
    pub id: u32,
    pub mod_id: u32,
    pub display_name: String,
    pub file_name: String,
    /// `None` when the author forbids third-party downloads.
    pub download_url: Option<String>,
    pub sha1: Option<String>,
    pub size: u64,
    pub fingerprint: u32,
    /// 1 release, 2 beta, 3 alpha.
    pub release_type: u32,
    /// Projects this file can't run without.
    pub required_projects: Vec<u32>,
}

impl From<CfFile> for CfContentFile {
    fn from(f: CfFile) -> Self {
        CfContentFile {
            id: f.id,
            mod_id: f.mod_id,
            sha1: f.sha1(),
            display_name: f.display_name,
            file_name: f.file_name,
            download_url: f.download_url,
            size: f.file_length,
            fingerprint: f.file_fingerprint,
            release_type: f.release_type,
            required_projects: f.dependencies.iter().filter(|d| d.relation_type == 3).map(|d| d.mod_id).collect(),
        }
    }
}

impl CurseForgeProvider {
    /// Projects of `class_id` compatible with `game_version` (and `loader`
    /// for mods), most popular first.
    pub async fn search_content(
        &self,
        class_id: u32,
        game_version: &str,
        loader: Option<u32>,
        query: &str,
        offset: u32,
    ) -> Result<Vec<CfContentHit>, ProviderError> {
        let mut params = vec![
            ("gameId", MINECRAFT_GAME_ID.to_string()),
            ("classId", class_id.to_string()),
            ("gameVersion", game_version.to_string()),
            ("searchFilter", query.to_string()),
            ("sortField", "2".to_string()),
            ("sortOrder", "desc".to_string()),
            ("pageSize", "30".to_string()),
            ("index", offset.to_string()),
        ];
        if let Some(loader) = loader {
            params.push(("modLoaderType", loader.to_string()));
        }
        let response: ListResponse<CfMod> = self.get("/mods/search", &params).await?;
        Ok(response.data.iter().map(CfContentHit::from).collect())
    }

    /// Files of `mod_id` for this game version / loader, newest first.
    pub async fn content_files(
        &self,
        mod_id: u32,
        game_version: &str,
        loader: Option<u32>,
    ) -> Result<Vec<CfContentFile>, ProviderError> {
        let mut params = vec![("gameVersion", game_version.to_string()), ("pageSize", "50".to_string())];
        if let Some(loader) = loader {
            params.push(("modLoaderType", loader.to_string()));
        }
        let response: ListResponse<CfFile> = self.get(&format!("/mods/{mod_id}/files"), &params).await?;
        let mut files: Vec<CfContentFile> =
            response.data.into_iter().filter(|f| f.is_server_pack != Some(true)).map(CfContentFile::from).collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.id));
        Ok(files)
    }
}

/// The file to install: the newest release, else the newest file at all.
pub fn pick_install_file(files: &[CfContentFile]) -> Option<&CfContentFile> {
    files.iter().find(|f| f.release_type == 1).or_else(|| files.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u32, release_type: u32) -> CfContentFile {
        CfContentFile {
            id,
            mod_id: 1,
            display_name: String::new(),
            file_name: format!("f{id}.jar"),
            download_url: None,
            sha1: None,
            size: 0,
            fingerprint: 0,
            release_type,
            required_projects: Vec::new(),
        }
    }

    #[test]
    fn install_prefers_the_newest_release() {
        let files = vec![file(30, 2), file(20, 1), file(10, 1)];
        assert_eq!(pick_install_file(&files).unwrap().id, 20);
        assert_eq!(pick_install_file(&[file(5, 3)]).unwrap().id, 5);
        assert!(pick_install_file(&[]).is_none());
    }

    #[test]
    fn loader_types_match_curseforge_ids() {
        assert_eq!(loader_type(LoaderKind::Forge), Some(1));
        assert_eq!(loader_type(LoaderKind::NeoForge), Some(6));
        assert_eq!(loader_type(LoaderKind::Vanilla), None);
    }

    #[test]
    fn required_dependencies_are_kept_from_file_json() {
        let json = serde_json::json!({
            "id": 7, "modId": 3, "displayName": "X", "fileName": "x.jar", "downloadUrl": "https://edge/x.jar",
            "hashes": [{"value": "AB", "algo": 1}], "fileLength": 9, "fileFingerprint": 42, "releaseType": 1,
            "dependencies": [{"modId": 100, "relationType": 3}, {"modId": 200, "relationType": 2}]
        });
        let parsed: CfFile = serde_json::from_value(json).unwrap();
        let f = CfContentFile::from(parsed);
        assert_eq!(f.required_projects, vec![100]);
        assert_eq!(f.sha1.as_deref(), Some("ab"));
        assert_eq!(f.fingerprint, 42);
    }
}
