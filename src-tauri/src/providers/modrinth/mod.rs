//! Modrinth modpacks (`.mrpack`): a zip holding `modrinth.index.json` (the
//! file list with hashes and download URLs, plus Minecraft/loader versions)
//! and `overrides/` + `client-overrides/` folders.

pub mod api;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;

use super::archive;
use super::{
    InstallWarning, LoaderKind, ModpackDetails, ModpackFileRef, ModpackProvider, ModpackSummary,
    ModpackVersionSummary, ProviderError, ResolvedModpackVersion, SearchQuery,
};
use crate::download::{DownloadItem, DownloadManager};
use crate::util::fs::safe_join;
pub use api::ModrinthApi;

/// Hosts the `.mrpack` spec allows downloads from.
const ALLOWED_HOSTS: &[&str] =
    &["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com", "objects.githubusercontent.com"];

#[derive(Debug, Deserialize)]
struct MrpackIndex {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    files: Vec<MrpackFile>,
    dependencies: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct MrpackFile {
    path: String,
    hashes: api::Hashes,
    #[serde(default)]
    env: Option<MrpackEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
}

#[derive(Debug, Deserialize)]
struct MrpackEnv {
    #[serde(default)]
    client: Option<String>,
}

fn host_allowed(url: &str) -> bool {
    url.strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
}

fn loader_from_dependencies(deps: &std::collections::HashMap<String, String>) -> (LoaderKind, String) {
    for (key, kind) in [
        ("neoforge", LoaderKind::NeoForge),
        ("forge", LoaderKind::Forge),
        ("fabric-loader", LoaderKind::Fabric),
        ("quilt-loader", LoaderKind::Quilt),
    ] {
        if let Some(version) = deps.get(key) {
            return (kind, version.clone());
        }
    }
    (LoaderKind::Vanilla, String::new())
}

/// Parses an `.mrpack` already on disk, extracting its override folders to
/// `extract_dir`. Shared by provider installs and local `.mrpack` imports.
pub async fn resolve_mrpack(mrpack: &Path, extract_dir: &Path) -> Result<ResolvedModpackVersion, ProviderError> {
    let index_text = archive::read_text(mrpack, "modrinth.index.json")?
        .ok_or_else(|| ProviderError::Other("fichier .mrpack invalide (modrinth.index.json absent)".to_string()))?;
    let index: MrpackIndex =
        serde_json::from_str(&index_text).map_err(|e| ProviderError::Other(format!("modrinth.index.json invalide: {e}")))?;

    let minecraft_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| ProviderError::Other("le modpack ne précise pas sa version de Minecraft".to_string()))?;
    let (loader, loader_version) = loader_from_dependencies(&index.dependencies);

    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for (i, file) in index.files.iter().enumerate() {
        if file.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
            continue;
        }
        let Some(_) = safe_join(Path::new("instance"), &file.path) else {
            warnings.push(InstallWarning {
                file_name: file.path.clone(),
                message: "Chemin de fichier refusé (sort du dossier de l'instance).".to_string(),
                ..Default::default()
            });
            continue;
        };
        let Some(url) = file.downloads.iter().find(|u| host_allowed(u)).cloned() else {
            warnings.push(InstallWarning {
                file_name: file.path.clone(),
                message: "Source de téléchargement non autorisée par le format Modrinth.".to_string(),
                ..Default::default()
            });
            continue;
        };
        files.push(ModpackFileRef {
            project_id: String::new(),
            file_id: i.to_string(),
            path: PathBuf::from(&file.path),
            sha1: Some(file.hashes.sha1.clone()),
            size: file.file_size,
            direct_url: Some(url),
            browser_url: None,
        });
    }

    let overrides_dirs = {
        let (zip, dest) = (mrpack.to_path_buf(), extract_dir.to_path_buf());
        tokio::task::spawn_blocking(move || archive::extract_prefixes(&zip, &dest, &["overrides/", "client-overrides/"]))
            .await
            .map_err(|e| ProviderError::Other(format!("tâche de fond interrompue: {e}")))??
    };

    Ok(ResolvedModpackVersion {
        minecraft_version,
        loader,
        loader_version,
        files,
        overrides_dirs,
        warnings,
        pack_name: index.name,
    })
}

pub struct ModrinthProvider {
    api: ModrinthApi,
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl ModrinthProvider {
    pub fn new(client: reqwest::Client, cache_dir: PathBuf) -> Self {
        Self { api: ModrinthApi::new(client.clone()), client, cache_dir }
    }
}

fn summary_from_hit(hit: api::SearchHit) -> ModpackSummary {
    ModpackSummary {
        id: hit.project_id,
        provider: "modrinth".to_string(),
        name: hit.title,
        author: hit.author,
        icon_url: hit.icon_url,
        summary: hit.description,
        downloads: Some(hit.downloads),
    }
}

#[async_trait]
impl ModpackProvider for ModrinthProvider {
    fn id(&self) -> &'static str {
        "modrinth"
    }

    fn display_name(&self) -> &'static str {
        "Modrinth"
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError> {
        let hits = self
            .api
            .search(&query.text, vec![vec!["project_type:modpack".to_string()]], query.offset, 24)
            .await?;
        Ok(hits.into_iter().map(summary_from_hit).collect())
    }

    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError> {
        let (project, author) = tokio::join!(self.api.project(pack_id), self.api.author(pack_id));
        let project = project?;
        Ok(ModpackDetails {
            summary: ModpackSummary {
                id: project.id.clone(),
                provider: "modrinth".to_string(),
                name: project.title.clone(),
                author: author.unwrap_or_default(),
                icon_url: project.icon_url.clone(),
                summary: project.description.clone(),
                downloads: Some(project.downloads),
            },
            description: project.body,
        })
    }

    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        let versions = self.api.project_versions(pack_id, &[], &[]).await?;
        Ok(versions
            .into_iter()
            .map(|v| ModpackVersionSummary {
                minecraft_version: v.game_versions.first().cloned().unwrap_or_default(),
                loader: v.loaders.iter().find_map(|l| LoaderKind::from_name(l)).unwrap_or(LoaderKind::Vanilla),
                loader_version: String::new(),
                name: if v.name.is_empty() { v.version_number.clone() } else { v.name.clone() },
                id: v.id,
            })
            .collect())
    }

    async fn resolve_version(&self, pack_id: &str, version_id: &str) -> Result<ResolvedModpackVersion, ProviderError> {
        let version = self.api.version(version_id).await?;
        if version.project_id != pack_id {
            return Err(ProviderError::Other("cette version n'appartient pas à ce modpack".to_string()));
        }
        let file = version
            .files
            .iter()
            .find(|f| f.filename.ends_with(".mrpack"))
            .or_else(|| version.primary_file())
            .ok_or_else(|| ProviderError::Other("cette version ne contient pas de fichier .mrpack".to_string()))?;

        let mrpack = self.cache_dir.join(format!("{version_id}.mrpack"));
        DownloadManager::new(self.client.clone())
            .ensure_file(&DownloadItem {
                url: file.url.clone(),
                dest: mrpack.clone(),
                sha1: Some(file.hashes.sha1.clone()),
                size: (file.size > 0).then_some(file.size),
            })
            .await?;
        resolve_mrpack(&mrpack, &self.cache_dir.join(format!("{version_id}-extracted"))).await
    }

    async fn get_changelog(&self, _pack_id: &str, version_id: &str) -> Result<Option<String>, ProviderError> {
        let version = self.api.version(version_id).await?;
        Ok(version.changelog.filter(|c| !c.trim().is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn only_spec_allowed_hosts_are_accepted() {
        assert!(host_allowed("https://cdn.modrinth.com/data/abc/x.jar"));
        assert!(host_allowed("https://github.com/owner/repo/releases/download/x.jar"));
        assert!(!host_allowed("http://cdn.modrinth.com/data/abc/x.jar"));
        assert!(!host_allowed("https://evil.example/x.jar"));
    }

    #[tokio::test]
    async fn resolve_mrpack_reads_index_filters_client_files_and_extracts_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let mrpack = dir.path().join("pack.mrpack");
        let index = serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1.0", "name": "Demo",
            "files": [
                {"path": "mods/a.jar", "hashes": {"sha1": "aa", "sha512": "bb"}, "downloads": ["https://cdn.modrinth.com/a.jar"], "fileSize": 3},
                {"path": "mods/server.jar", "hashes": {"sha1": "cc"}, "env": {"client": "unsupported", "server": "required"}, "downloads": ["https://cdn.modrinth.com/s.jar"], "fileSize": 1},
                {"path": "../evil.jar", "hashes": {"sha1": "dd"}, "downloads": ["https://cdn.modrinth.com/e.jar"], "fileSize": 1}
            ],
            "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.15.11"}
        });
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&mrpack).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("modrinth.index.json", opts).unwrap();
            zip.write_all(index.to_string().as_bytes()).unwrap();
            zip.start_file("overrides/config/x.toml", opts).unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }

        let resolved = resolve_mrpack(&mrpack, &dir.path().join("extracted")).await.unwrap();

        assert_eq!(resolved.minecraft_version, "1.20.1");
        assert_eq!(resolved.loader, LoaderKind::Fabric);
        assert_eq!(resolved.loader_version, "0.15.11");
        assert_eq!(resolved.files.len(), 1);
        assert_eq!(resolved.files[0].path, PathBuf::from("mods/a.jar"));
        assert_eq!(resolved.warnings.len(), 1);
        assert_eq!(resolved.overrides_dirs.len(), 1);
        assert_eq!(resolved.pack_name.as_deref(), Some("Demo"));
    }
}
