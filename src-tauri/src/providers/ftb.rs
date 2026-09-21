//! FTB modpacks via the public `api.modpacks.ch` API (the same backend the
//! official FTB App uses) — no API key required.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::task::JoinSet;

use super::{
    FileDownloadInfo, LoaderKind, ModpackDetails, ModpackFileRef, ModpackProvider, ModpackSummary,
    ModpackVersionSummary, ProviderError, ResolvedModpackVersion, SearchQuery,
};

const BASE: &str = "https://api.modpacks.ch/public";

pub struct FtbProvider {
    client: reqwest::Client,
}

impl FtbProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn fetch_pack(&self, pack_id: &str) -> Result<FtbPackResponse, ProviderError> {
        let response = self.client.get(format!("{BASE}/modpack/{pack_id}")).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ProviderError::NotFound(pack_id.to_string()));
        }
        Ok(response.error_for_status()?.json().await?)
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    packs: Vec<u64>,
}

#[derive(Debug, Deserialize, Clone)]
struct FtbArt {
    url: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize, Clone)]
struct FtbAuthor {
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
struct FtbVersionRef {
    id: u64,
    name: String,
    #[serde(default)]
    targets: Vec<FtbTarget>,
}

#[derive(Debug, Deserialize)]
struct FtbPackResponse {
    id: u64,
    name: String,
    #[serde(default)]
    synopsis: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    art: Vec<FtbArt>,
    #[serde(default)]
    authors: Vec<FtbAuthor>,
    #[serde(default)]
    versions: Vec<FtbVersionRef>,
}

impl FtbPackResponse {
    fn into_summary(&self) -> ModpackSummary {
        ModpackSummary {
            id: self.id.to_string(),
            provider: "ftb".to_string(),
            name: self.name.clone(),
            author: self.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            icon_url: self
                .art
                .iter()
                .find(|a| a.kind == "square" || a.kind == "logo")
                .or_else(|| self.art.first())
                .map(|a| a.url.clone()),
            summary: self.synopsis.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct FtbTarget {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct FtbFile {
    id: u64,
    path: String,
    name: String,
    url: String,
    #[serde(default)]
    sha1: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    serveronly: bool,
    #[serde(default)]
    clientonly: bool,
}

#[derive(Debug, Deserialize)]
struct FtbVersionDetail {
    #[serde(default)]
    targets: Vec<FtbTarget>,
    #[serde(default)]
    files: Vec<FtbFile>,
}

fn loader_kind_from_name(name: &str) -> Option<LoaderKind> {
    match name.to_lowercase().as_str() {
        "forge" => Some(LoaderKind::Forge),
        "neoforge" => Some(LoaderKind::NeoForge),
        "fabric" => Some(LoaderKind::Fabric),
        "quilt" => Some(LoaderKind::Quilt),
        _ => None,
    }
}

#[async_trait]
impl ModpackProvider for FtbProvider {
    fn id(&self) -> &'static str {
        "ftb"
    }

    fn display_name(&self) -> &'static str {
        "FTB"
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError> {
        let term = query.text.trim();
        let ids: Vec<u64> = if term.is_empty() {
            let response: SearchResponse = self
                .client
                .get(format!("{BASE}/modpack/popular/installs/24"))
                .send()
                .await?
                .json()
                .await?;
            response.packs
        } else {
            let response: SearchResponse = self
                .client
                .get(format!("{BASE}/modpack/search/25"))
                .query(&[("term", term)])
                .send()
                .await?
                .json()
                .await?;
            response.packs
        };

        let mut set = JoinSet::new();
        for id in ids.into_iter().take(24) {
            let client = self.client.clone();
            set.spawn(async move {
                client
                    .get(format!("{BASE}/modpack/{id}"))
                    .send()
                    .await?
                    .json::<FtbPackResponse>()
                    .await
            });
        }

        let mut summaries = Vec::new();
        while let Some(result) = set.join_next().await {
            if let Ok(Ok(pack)) = result {
                summaries.push(pack.into_summary());
            }
        }
        Ok(summaries)
    }

    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError> {
        let pack = self.fetch_pack(pack_id).await?;
        Ok(ModpackDetails {
            summary: pack.into_summary(),
            description: pack.description.clone(),
        })
    }

    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        // The pack detail response already embeds each version's `targets`,
        // so no per-version fetch is needed here (only `resolve_version`
        // needs a dedicated request, for the file list).
        let pack = self.fetch_pack(pack_id).await?;

        let mut versions: Vec<ModpackVersionSummary> = pack
            .versions
            .into_iter()
            .map(|version| {
                let minecraft_version = version
                    .targets
                    .iter()
                    .find(|t| t.name == "minecraft")
                    .map(|t| t.version.clone())
                    .unwrap_or_default();
                let (loader, loader_version) = version
                    .targets
                    .iter()
                    .find_map(|t| loader_kind_from_name(&t.name).map(|kind| (kind, t.version.clone())))
                    .unwrap_or((LoaderKind::Vanilla, String::new()));

                ModpackVersionSummary {
                    id: version.id.to_string(),
                    name: version.name,
                    minecraft_version,
                    loader,
                    loader_version,
                }
            })
            .collect();

        versions.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(versions)
    }

    async fn resolve_version(
        &self,
        pack_id: &str,
        version_id: &str,
    ) -> Result<ResolvedModpackVersion, ProviderError> {
        let detail: FtbVersionDetail = self
            .client
            .get(format!("{BASE}/modpack/{pack_id}/{version_id}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let minecraft_version = detail
            .targets
            .iter()
            .find(|t| t.name == "minecraft")
            .map(|t| t.version.clone())
            .ok_or_else(|| ProviderError::Other("version FTB sans cible Minecraft".to_string()))?;

        let (loader, loader_version) = detail
            .targets
            .iter()
            .find_map(|t| loader_kind_from_name(&t.name).map(|k| (k, t.version.clone())))
            .unwrap_or((LoaderKind::Vanilla, String::new()));

        let files = detail
            .files
            .into_iter()
            .filter(|f| !f.serveronly || f.clientonly)
            .map(|f| ModpackFileRef {
                project_id: pack_id.to_string(),
                file_id: f.id.to_string(),
                path: std::path::PathBuf::from(f.path.trim_start_matches("./")).join(&f.name),
                sha1: (!f.sha1.is_empty()).then_some(f.sha1),
                size: f.size,
                direct_url: (!f.url.is_empty()).then_some(f.url),
            })
            .collect();

        Ok(ResolvedModpackVersion {
            minecraft_version,
            loader,
            loader_version,
            files,
            overrides_dir: None,
        })
    }

    async fn resolve_file_download(&self, file: &ModpackFileRef) -> Result<FileDownloadInfo, ProviderError> {
        match &file.direct_url {
            Some(url) => Ok(FileDownloadInfo::Direct { url: url.clone() }),
            None => Err(ProviderError::Other(format!(
                "fichier FTB {} sans URL directe",
                file.file_id
            ))),
        }
    }
}
