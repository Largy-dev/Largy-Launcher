//! CurseForge modpacks via the official Core API (`api.curseforge.com`).
//! Requires a personal API key from https://console.curseforge.com/ — set it
//! in Paramètres. Modpacks are distributed as a zip (`manifest.json` +
//! `overrides/`), so `resolve_version` downloads and unpacks it, then
//! resolves each referenced mod file's real download URL individually
//! (CurseForge lets mod authors disable third-party redistribution, in
//! which case we fall back to `FileDownloadInfo::ManualRequired`).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::task::JoinSet;

use super::{
    FileDownloadInfo, LoaderKind, ModpackDetails, ModpackFileRef, ModpackProvider, ModpackSummary,
    ModpackVersionSummary, ProviderError, ResolvedModpackVersion, SearchQuery,
};

const BASE: &str = "https://api.curseforge.com/v1";
const MINECRAFT_GAME_ID: u32 = 432;
const MODPACK_CLASS_ID: u32 = 4471;

pub struct CurseForgeProvider {
    client: reqwest::Client,
    api_key: Arc<RwLock<String>>,
    cache_dir: PathBuf,
}

impl CurseForgeProvider {
    pub fn new(client: reqwest::Client, api_key: Arc<RwLock<String>>, cache_dir: PathBuf) -> Self {
        Self { client, api_key, cache_dir }
    }

    fn key(&self) -> Result<String, ProviderError> {
        let key = self.api_key.read().unwrap().clone();
        if key.trim().is_empty() {
            return Err(ProviderError::Other(
                "Clé API CurseForge manquante. Ouvre Paramètres et renseigne ta clé depuis \
                 console.curseforge.com pour parcourir les modpacks CurseForge."
                    .to_string(),
            ));
        }
        Ok(key)
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: Vec<CfMod>,
}

#[derive(Debug, Deserialize)]
struct ModResponse {
    data: CfMod,
}

#[derive(Debug, Deserialize)]
struct FilesResponse {
    data: Vec<CfFile>,
}

#[derive(Debug, Deserialize)]
struct FileResponse {
    data: CfFile,
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
struct CfMod {
    id: u32,
    name: String,
    summary: String,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
}

impl CfMod {
    fn to_summary(&self) -> ModpackSummary {
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
struct CfFile {
    id: u32,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(rename = "gameVersions", default)]
    game_versions: Vec<String>,
}

impl CfFile {
    fn minecraft_version(&self) -> Option<String> {
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
    fn loader(&self) -> Option<(LoaderKind, String)> {
        const PRIORITY: [LoaderKind; 4] =
            [LoaderKind::NeoForge, LoaderKind::Quilt, LoaderKind::Fabric, LoaderKind::Forge];
        PRIORITY
            .into_iter()
            .find(|kind| self.game_versions.iter().any(|v| LoaderKind::from_name(v) == Some(*kind)))
            .map(|kind| (kind, String::new()))
    }
}

#[derive(Debug, Deserialize)]
struct CfManifest {
    minecraft: CfManifestMinecraft,
    #[serde(default)]
    files: Vec<CfManifestFile>,
}

#[derive(Debug, Deserialize)]
struct CfManifestMinecraft {
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<CfManifestLoader>,
}

#[derive(Debug, Deserialize)]
struct CfManifestLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(default = "default_true")]
    required: bool,
}

fn default_true() -> bool {
    true
}

fn parse_loader_id(id: &str) -> Option<(LoaderKind, String)> {
    let (name, version) = id.split_once('-')?;
    LoaderKind::from_name(name).map(|kind| (kind, version.to_string()))
}

#[async_trait]
impl ModpackProvider for CurseForgeProvider {
    fn id(&self) -> &'static str {
        "curseforge"
    }

    fn display_name(&self) -> &'static str {
        "CurseForge"
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError> {
        let key = self.key()?;
        let response: SearchResponse = self
            .client
            .get(format!("{BASE}/mods/search"))
            .header("x-api-key", key)
            .query(&[
                ("gameId", MINECRAFT_GAME_ID.to_string()),
                ("classId", MODPACK_CLASS_ID.to_string()),
                ("searchFilter", query.text.clone()),
                ("pageSize", "25".to_string()),
                ("sortField", "2".to_string()),
                ("sortOrder", "desc".to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(response.data.iter().map(CfMod::to_summary).collect())
    }

    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError> {
        let key = self.key()?;
        let response: ModResponse = self
            .client
            .get(format!("{BASE}/mods/{pack_id}"))
            .header("x-api-key", key)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(ModpackDetails {
            summary: response.data.to_summary(),
            description: response.data.summary.clone(),
        })
    }

    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        let key = self.key()?;
        let response: FilesResponse = self
            .client
            .get(format!("{BASE}/mods/{pack_id}/files"))
            .header("x-api-key", key)
            .query(&[("pageSize", "50")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(response
            .data
            .into_iter()
            .map(|f| {
                let (loader, loader_version) = f.loader().unwrap_or((LoaderKind::Vanilla, String::new()));
                ModpackVersionSummary {
                    id: f.id.to_string(),
                    name: f.display_name.clone(),
                    minecraft_version: f.minecraft_version().unwrap_or_default(),
                    loader,
                    loader_version,
                }
            })
            .collect())
    }

    async fn resolve_version(
        &self,
        pack_id: &str,
        version_id: &str,
    ) -> Result<ResolvedModpackVersion, ProviderError> {
        let key = self.key()?;
        let file: FileResponse = self
            .client
            .get(format!("{BASE}/mods/{pack_id}/files/{version_id}"))
            .header("x-api-key", key.clone())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let download_url = file.data.download_url.clone().ok_or_else(|| {
            ProviderError::Other(
                "Cet auteur désactive le téléchargement direct de ce modpack ; télécharge-le \
                 manuellement depuis curseforge.com puis importe le zip."
                    .to_string(),
            )
        })?;

        let pack_zip_path = self.cache_dir.join(format!("{pack_id}-{version_id}.zip"));
        if !pack_zip_path.exists() {
            std::fs::create_dir_all(&self.cache_dir).map_err(|e| ProviderError::Other(e.to_string()))?;
            let bytes = self.client.get(&download_url).send().await?.bytes().await?;
            std::fs::write(&pack_zip_path, &bytes).map_err(|e| ProviderError::Other(e.to_string()))?;
        }

        let extract_dir = self.cache_dir.join(format!("{pack_id}-{version_id}-extracted"));
        let manifest = extract_manifest_and_overrides(&pack_zip_path, &extract_dir)
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let (loader, loader_version) = manifest
            .minecraft
            .mod_loaders
            .iter()
            .find(|l| l.primary)
            .or_else(|| manifest.minecraft.mod_loaders.first())
            .and_then(|l| parse_loader_id(&l.id))
            .unwrap_or((LoaderKind::Vanilla, String::new()));

        let mut set = JoinSet::new();
        for entry in manifest.files.iter().filter(|f| f.required).cloned() {
            let client = self.client.clone();
            let key = key.clone();
            set.spawn(async move {
                let response: Result<FileResponse, reqwest::Error> = client
                    .get(format!("{BASE}/mods/{}/files/{}", entry.project_id, entry.file_id))
                    .header("x-api-key", key)
                    .send()
                    .await?
                    .json()
                    .await;
                response.map(|r| (entry, r.data))
            });
        }

        let mut files = Vec::new();
        while let Some(result) = set.join_next().await {
            if let Ok(Ok((entry, file))) = result {
                files.push(ModpackFileRef {
                    project_id: entry.project_id.to_string(),
                    file_id: entry.file_id.to_string(),
                    path: PathBuf::from("mods").join(&file.file_name),
                    sha1: None,
                    size: 0,
                    direct_url: file.download_url,
                });
            }
        }

        Ok(ResolvedModpackVersion {
            minecraft_version: manifest.minecraft.version,
            loader,
            loader_version,
            files,
            overrides_dir: Some(extract_dir.join("overrides")),
        })
    }

    async fn resolve_file_download(&self, file: &ModpackFileRef) -> Result<FileDownloadInfo, ProviderError> {
        if let Some(url) = &file.direct_url {
            return Ok(FileDownloadInfo::Direct { url: url.clone() });
        }

        let key = self.key()?;
        let response: FileResponse = self
            .client
            .get(format!("{BASE}/mods/{}/files/{}", file.project_id, file.file_id))
            .header("x-api-key", key)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        match response.data.download_url {
            Some(url) => Ok(FileDownloadInfo::Direct { url }),
            None => Ok(FileDownloadInfo::ManualRequired {
                browser_url: format!("https://www.curseforge.com/minecraft/mc-mods/search?search={}", file.project_id),
                expected_filename: response.data.file_name,
            }),
        }
    }
}

fn extract_manifest_and_overrides(zip_path: &std::path::Path, extract_dir: &std::path::Path) -> std::io::Result<CfManifest> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| std::io::Error::other(e.to_string()))?;

    let manifest: CfManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text)?;
        serde_json::from_str(&text).map_err(|e| std::io::Error::other(e.to_string()))?
    };

    if !extract_dir.exists() {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| std::io::Error::other(e.to_string()))?;
            let name = entry.name().to_string();
            if !name.starts_with("overrides/") || name.ends_with('/') {
                continue;
            }
            let dest = extract_dir.join(name.trim_start_matches("overrides/"));
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }

    Ok(manifest)
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
