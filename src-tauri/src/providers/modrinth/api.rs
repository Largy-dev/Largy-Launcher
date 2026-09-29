//! Thin typed client for the public Modrinth v2 API (no key required).
//! Shared by the modpack provider and by in-instance content browsing,
//! installation and update checks.

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::providers::ProviderError;

const BASE: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchHit {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub project_type: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Project {
    pub id: String,
    #[serde(default)]
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub downloads: u64,
}

#[derive(Debug, Deserialize)]
struct Member {
    user: MemberUser,
    #[serde(default)]
    role: String,
}

#[derive(Debug, Deserialize)]
struct MemberUser {
    username: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Hashes {
    pub sha1: String,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: u64,
    pub hashes: Hashes,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Dependency {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub changelog: Option<String>,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

#[derive(Clone)]
pub struct ModrinthApi {
    client: reqwest::Client,
}

impl ModrinthApi {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T, ProviderError> {
        let response = self.client.get(format!("{BASE}{path}")).query(query).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ProviderError::NotFound(path.to_string()));
        }
        Ok(response.error_for_status()?.json().await?)
    }

    async fn post<T: DeserializeOwned>(&self, path: &str, body: serde_json::Value) -> Result<T, ProviderError> {
        Ok(self.client.post(format!("{BASE}{path}")).json(&body).send().await?.error_for_status()?.json().await?)
    }

    /// `facets` is Modrinth's AND-of-ORs filter, e.g.
    /// `[["project_type:mod"], ["versions:1.20.1"], ["categories:fabric"]]`.
    pub async fn search(
        &self,
        query: &str,
        facets: Vec<Vec<String>>,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        let index = if query.trim().is_empty() { "downloads" } else { "relevance" };
        let response: SearchResponse = self
            .get(
                "/search",
                &[
                    ("query", query.to_string()),
                    ("facets", serde_json::to_string(&facets).unwrap_or_default()),
                    ("index", index.to_string()),
                    ("offset", offset.to_string()),
                    ("limit", limit.to_string()),
                ],
            )
            .await?;
        Ok(response.hits)
    }

    pub async fn project(&self, id: &str) -> Result<Project, ProviderError> {
        self.get(&format!("/project/{id}"), &[]).await
    }

    pub async fn projects(&self, ids: &[String]) -> Result<Vec<Project>, ProviderError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.get("/projects", &[("ids", serde_json::to_string(ids).unwrap_or_default())]).await
    }

    /// Owner's username, falling back to the first member.
    pub async fn author(&self, id: &str) -> Result<String, ProviderError> {
        let members: Vec<Member> = self.get(&format!("/project/{id}/members"), &[]).await?;
        Ok(members
            .iter()
            .find(|m| m.role.eq_ignore_ascii_case("owner"))
            .or_else(|| members.first())
            .map(|m| m.user.username.clone())
            .unwrap_or_default())
    }

    /// Versions of a project, optionally filtered — newest first.
    pub async fn project_versions(
        &self,
        id: &str,
        loaders: &[&str],
        game_versions: &[&str],
    ) -> Result<Vec<Version>, ProviderError> {
        let mut query = Vec::new();
        if !loaders.is_empty() {
            query.push(("loaders", serde_json::to_string(loaders).unwrap_or_default()));
        }
        if !game_versions.is_empty() {
            query.push(("game_versions", serde_json::to_string(game_versions).unwrap_or_default()));
        }
        self.get(&format!("/project/{id}/version"), &query).await
    }

    pub async fn version(&self, id: &str) -> Result<Version, ProviderError> {
        self.get(&format!("/version/{id}"), &[]).await
    }

    /// Current version of each file, keyed by its SHA-1.
    pub async fn versions_by_hash(&self, sha1s: &[String]) -> Result<HashMap<String, Version>, ProviderError> {
        if sha1s.is_empty() {
            return Ok(HashMap::new());
        }
        self.post("/version_files", json!({ "hashes": sha1s, "algorithm": "sha1" })).await
    }

    /// Newest compatible version of each file, keyed by its current SHA-1.
    pub async fn latest_by_hash(
        &self,
        sha1s: &[String],
        loaders: &[&str],
        game_versions: &[&str],
    ) -> Result<HashMap<String, Version>, ProviderError> {
        if sha1s.is_empty() {
            return Ok(HashMap::new());
        }
        let mut body = json!({ "hashes": sha1s, "algorithm": "sha1", "game_versions": game_versions });
        if !loaders.is_empty() {
            body["loaders"] = json!(loaders);
        }
        self.post("/version_files/update", body).await
    }
}
