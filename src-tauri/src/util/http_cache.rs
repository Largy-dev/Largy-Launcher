//! Disk cache for small metadata documents (version manifests, loader
//! profiles, Java runtime indexes). Fresh copies skip the network entirely;
//! stale ones are refreshed but still served when the network is down, which
//! is what lets an already-installed instance launch offline.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{AppError, AppResult};
use crate::util::fs::write_atomic;

/// URLs whose content never changes (they embed a hash or an exact version).
pub const IMMUTABLE: Duration = Duration::from_secs(u64::MAX / 4);
pub const HOURLY: Duration = Duration::from_secs(3600);
pub const DAILY: Duration = Duration::from_secs(86_400);

#[derive(Clone)]
pub struct MetaCache {
    client: reqwest::Client,
    dir: PathBuf,
}

fn cache_key(url: &str) -> String {
    crate::download::sha1_of_bytes(url.as_bytes())
}

fn is_fresh(path: &Path, ttl: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < ttl)
}

impl MetaCache {
    pub fn new(client: reqwest::Client, dir: PathBuf) -> Self {
        Self { client, dir }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn path_for(&self, url: &str) -> PathBuf {
        self.dir.join(cache_key(url))
    }

    /// Raw body of `url`, from cache when younger than `ttl`.
    pub async fn get_bytes(&self, url: &str, ttl: Duration) -> AppResult<Vec<u8>> {
        let path = self.path_for(url);
        if is_fresh(&path, ttl) {
            if let Ok(bytes) = tokio::fs::read(&path).await {
                return Ok(bytes);
            }
        }

        match self.fetch(url).await {
            Ok(bytes) => {
                let (path, bytes_for_disk) = (path.clone(), bytes.clone());
                let _ = tokio::task::spawn_blocking(move || write_atomic(&path, &bytes_for_disk)).await;
                Ok(bytes)
            }
            Err(err) => match tokio::fs::read(&path).await {
                Ok(stale) => {
                    tracing::warn!("using cached copy of {url} (network error: {err})");
                    Ok(stale)
                }
                Err(_) => Err(err),
            },
        }
    }

    async fn fetch(&self, url: &str) -> AppResult<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| AppError::Download(format!("{url}: {e}")))?;
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str, ttl: Duration) -> AppResult<T> {
        let bytes = self.get_bytes(url, ttl).await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            // A cached body that no longer parses is useless — drop it so the
            // next call refetches instead of failing forever.
            let _ = std::fs::remove_file(self.path_for(url));
            AppError::Serde(e)
        })
    }

    pub async fn get_text(&self, url: &str, ttl: Duration) -> AppResult<String> {
        let bytes = self.get_bytes(url, ttl).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fresh_cache_entry_is_served_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let cache = MetaCache::new(reqwest::Client::new(), dir.path().to_path_buf());
        let url = "https://example.invalid/manifest.json";
        std::fs::write(cache.path_for(url), br#"{"ok":true}"#).unwrap();

        let value: serde_json::Value = cache.get_json(url, HOURLY).await.unwrap();
        assert_eq!(value["ok"], true);
    }

    #[tokio::test]
    async fn stale_cache_entry_is_used_when_the_network_fails() {
        let dir = tempfile::tempdir().unwrap();
        let cache = MetaCache::new(reqwest::Client::new(), dir.path().to_path_buf());
        let url = "https://example.invalid/stale.json";
        std::fs::write(cache.path_for(url), br#"{"stale":1}"#).unwrap();

        let value: serde_json::Value = cache.get_json(url, Duration::ZERO).await.unwrap();
        assert_eq!(value["stale"], 1);
    }

    #[tokio::test]
    async fn missing_entry_and_no_network_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = MetaCache::new(reqwest::Client::new(), dir.path().to_path_buf());
        assert!(cache.get_bytes("https://example.invalid/none", HOURLY).await.is_err());
    }
}
