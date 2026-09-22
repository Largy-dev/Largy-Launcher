//! CurseForge modpacks via the official Core API (`api.curseforge.com`).
//! Requires a personal API key from https://console.curseforge.com/ — set it
//! in Paramètres. Modpacks are distributed as a zip (`manifest.json` +
//! `overrides/`), so `resolve_version` downloads and unpacks it, then
//! resolves each referenced mod file's real download URL individually
//! (CurseForge lets mod authors disable third-party redistribution, in
//! which case we fall back to `FileDownloadInfo::ManualRequired`).

mod api_types;
mod zip_extract;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tokio::task::JoinSet;

use super::{
    FileDownloadInfo, LoaderKind, ModpackDetails, ModpackFileRef, ModpackProvider, ModpackSummary,
    ModpackVersionSummary, ProviderError, ResolvedModpackVersion, SearchQuery,
};
use api_types::{parse_loader_id, FileResponse, FilesResponse, ModResponse, SearchResponse};
use zip_extract::extract_manifest_and_overrides;

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
        let key = self.api_key.read().clone();
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

        Ok(response.data.iter().map(|m| m.to_summary()).collect())
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
        let manifest = {
            let zip_path = pack_zip_path.clone();
            let extract_dir = extract_dir.clone();
            tokio::task::spawn_blocking(move || extract_manifest_and_overrides(&zip_path, &extract_dir))
                .await
                .map_err(|e| ProviderError::Other(format!("tâche de fond interrompue: {e}")))?
                .map_err(|e| ProviderError::Other(e.to_string()))?
        };

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
