//! Packs shared by link: an `.mrpack` put online by whoever runs a group or
//! a server (exported from Largy, Prism, Modrinth…). Friends paste the link
//! once; every change to the file shows up as a new version, installed by
//! the regular modpack update (files the new version drops are removed, the
//! players' options and worlds kept).
//!
//! The file's SHA-1 is its version id. Conditional requests (ETag /
//! Last-Modified) keep the periodic update check to a few hundred bytes when
//! nothing changed.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use super::modrinth::{mrpack_info, resolve_mrpack, MrpackInfo};
use super::{
    ModpackDetails, ModpackProvider, ModpackSummary, ModpackVersionSummary, ProviderError, ResolvedModpackVersion,
    SearchQuery,
};

/// A group pack's configs and resource packs can be sizeable, but not this.
const MAX_PACK_BYTES: u64 = 1024 * 1024 * 1024;

/// Share links as people paste them → the direct file URL.
pub fn normalize_url(input: &str) -> Result<String, ProviderError> {
    let url = input.trim();
    let invalid = || ProviderError::Other("lien invalide : il doit commencer par https://".to_string());
    let rest = url.strip_prefix("https://").ok_or_else(invalid)?;
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return Err(invalid());
    }
    // github.com/<user>/<repo>/blob/<ref>/<path> → raw file.
    if let Some(path) = rest.strip_prefix("github.com/") {
        let parts: Vec<&str> = path.splitn(4, '/').collect();
        if parts.len() == 4 && parts[2] == "blob" {
            return Ok(format!("https://raw.githubusercontent.com/{}/{}/{}", parts[0], parts[1], parts[3]));
        }
    }
    // Dropbox share links open a web page unless asked for the file itself.
    if rest.starts_with("www.dropbox.com/") || rest.starts_with("dropbox.com/") {
        if url.contains("dl=0") {
            return Ok(url.replace("dl=0", "dl=1"));
        }
        if !url.contains("dl=1") {
            let sep = if url.contains('?') { '&' } else { '?' };
            return Ok(format!("{url}{sep}dl=1"));
        }
    }
    Ok(url.to_string())
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Validators {
    etag: Option<String>,
    last_modified: Option<String>,
    sha1: String,
}

pub struct LinkedPackProvider {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl LinkedPackProvider {
    pub fn new(client: reqwest::Client, cache_dir: PathBuf) -> Self {
        Self { client, cache_dir }
    }

    fn key(url: &str) -> String {
        hex::encode(Sha1::digest(url.as_bytes()))
    }

    fn pack_path(&self, sha1: &str) -> PathBuf {
        self.cache_dir.join(format!("{sha1}.mrpack"))
    }

    fn validators_path(&self, url: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", Self::key(url)))
    }

    /// The current file behind `url`: downloaded when it changed, else the
    /// cached copy. Returns its SHA-1 and path.
    pub async fn fetch(&self, url: &str) -> Result<(String, PathBuf), ProviderError> {
        self.fetch_normalized(normalize_url(url)?).await
    }

    async fn fetch_normalized(&self, url: String) -> Result<(String, PathBuf), ProviderError> {
        tokio::fs::create_dir_all(&self.cache_dir).await.map_err(|e| ProviderError::Other(e.to_string()))?;
        let saved: Option<Validators> = tokio::fs::read(self.validators_path(&url))
            .await
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|v: &Validators| self.pack_path(&v.sha1).is_file());

        let mut request = self.client.get(&url);
        if let Some(v) = &saved {
            if let Some(etag) = &v.etag {
                request = request.header(reqwest::header::IF_NONE_MATCH, etag);
            }
            if let Some(date) = &v.last_modified {
                request = request.header(reqwest::header::IF_MODIFIED_SINCE, date);
            }
        }
        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(v) = saved {
                let path = self.pack_path(&v.sha1);
                return Ok((v.sha1, path));
            }
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ProviderError::Other("ce lien ne mène à aucun fichier (erreur 404)".to_string()));
        }
        let response = response.error_for_status()?;
        if response.content_length().is_some_and(|len| len > MAX_PACK_BYTES) {
            return Err(ProviderError::Other("fichier trop volumineux pour un pack".to_string()));
        }
        let header = |name: reqwest::header::HeaderName| {
            response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
        };
        let (etag, last_modified) = (header(reqwest::header::ETAG), header(reqwest::header::LAST_MODIFIED));

        let tmp = self.cache_dir.join(format!(".{}.part", uuid::Uuid::new_v4().simple()));
        let result = download(response, &tmp).await;
        let sha1 = match result {
            Ok(sha1) => sha1,
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(e);
            }
        };
        let path = self.pack_path(&sha1);
        tokio::fs::rename(&tmp, &path).await.map_err(|e| ProviderError::Other(e.to_string()))?;
        let validators = Validators { etag, last_modified, sha1: sha1.clone() };
        if let Ok(bytes) = serde_json::to_vec(&validators) {
            let _ = crate::util::fs::write_atomic(&self.validators_path(&url), &bytes);
        }
        Ok((sha1, path))
    }

    async fn info(&self, url: &str) -> Result<(String, MrpackInfo), ProviderError> {
        let (sha1, path) = self.fetch(url).await?;
        let info = tokio::task::spawn_blocking(move || mrpack_info(&path))
            .await
            .map_err(|e| ProviderError::Other(format!("tâche de fond interrompue: {e}")))??;
        Ok((sha1, info))
    }
}

async fn download(response: reqwest::Response, dest: &Path) -> Result<String, ProviderError> {
    let io = |e: std::io::Error| ProviderError::Other(e.to_string());
    let mut file = tokio::fs::File::create(dest).await.map_err(io)?;
    let mut hasher = Sha1::new();
    let mut total: u64 = 0;
    let mut head: Vec<u8> = Vec::with_capacity(2);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total += chunk.len() as u64;
        if total > MAX_PACK_BYTES {
            return Err(ProviderError::Other("fichier trop volumineux pour un pack".to_string()));
        }
        head.extend(chunk.iter().take(2 - head.len().min(2)));
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(io)?;
    }
    file.flush().await.map_err(io)?;
    // A zip starts with "PK"; anything else is most likely an HTML page.
    if head != b"PK" {
        return Err(ProviderError::Other(
            "ce lien ne donne pas un fichier .mrpack (une page web ? utilise le lien de téléchargement direct)"
                .to_string(),
        ));
    }
    Ok(hex::encode(hasher.finalize()))
}

fn host_of(url: &str) -> String {
    url.trim_start_matches("https://").split('/').next().unwrap_or_default().to_string()
}

#[async_trait]
impl ModpackProvider for LinkedPackProvider {
    fn id(&self) -> &'static str {
        "url"
    }

    fn display_name(&self) -> &'static str {
        "Lien"
    }

    async fn search(&self, _query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError> {
        Ok(Vec::new())
    }

    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError> {
        let (_, info) = self.info(pack_id).await?;
        Ok(ModpackDetails {
            summary: ModpackSummary {
                id: pack_id.to_string(),
                provider: "url".to_string(),
                name: info.name.clone().unwrap_or_else(|| "Pack partagé".to_string()),
                author: host_of(pack_id),
                summary: info.summary.clone().unwrap_or_default(),
                game_versions: vec![info.minecraft_version.clone()],
                loaders: vec![info.loader],
                ..Default::default()
            },
            description: info.summary.unwrap_or_default(),
        })
    }

    /// One version: the file currently behind the link.
    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        let (sha1, info) = self.info(pack_id).await?;
        Ok(vec![ModpackVersionSummary {
            id: sha1,
            name: info.version_id.unwrap_or_else(|| "Dernière version".to_string()),
            minecraft_version: info.minecraft_version,
            loader: info.loader,
            loader_version: info.loader_version,
        }])
    }

    async fn resolve_version(&self, pack_id: &str, version_id: &str) -> Result<ResolvedModpackVersion, ProviderError> {
        let (sha1, path) = self.fetch(pack_id).await?;
        if sha1 != version_id {
            return Err(ProviderError::Other("le pack a encore changé entre-temps : réessaie".to_string()));
        }
        resolve_mrpack(&path, &self.cache_dir.join(format!("{sha1}-extracted"))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_links_become_direct_file_links() {
        assert_eq!(
            normalize_url("https://github.com/me/pack/blob/main/pack.mrpack").unwrap(),
            "https://raw.githubusercontent.com/me/pack/main/pack.mrpack"
        );
        assert_eq!(
            normalize_url("https://www.dropbox.com/s/x/pack.mrpack?dl=0").unwrap(),
            "https://www.dropbox.com/s/x/pack.mrpack?dl=1"
        );
        assert_eq!(
            normalize_url("https://www.dropbox.com/scl/fi/x/pack.mrpack?rlkey=1").unwrap(),
            "https://www.dropbox.com/scl/fi/x/pack.mrpack?rlkey=1&dl=1"
        );
        assert_eq!(normalize_url(" https://site/p.mrpack ").unwrap(), "https://site/p.mrpack");
    }

    /// Serves `body` with ETag "v1", answering 304 to requests that already
    /// have it; counts full downloads.
    async fn serve(body: Vec<u8>) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let full = std::sync::Arc::new(AtomicUsize::new(0));
        let counter = full.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { break };
                let mut request = vec![0u8; 4096];
                let n = socket.read(&mut request).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..n]).to_lowercase();
                let reply = if request.contains("if-none-match: \"v1\"") {
                    b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nContent-Length: 0\r\n\r\n".to_vec()
                } else {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut r = format!("HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: {}\r\n\r\n", body.len())
                        .into_bytes();
                    r.extend(&body);
                    r
                };
                let _ = socket.write_all(&reply).await;
            }
        });
        (format!("http://{addr}/pack.mrpack"), full)
    }

    fn mrpack() -> Vec<u8> {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("modrinth.index.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"name":"Potes","versionId":"3","files":[],"dependencies":{"minecraft":"1.20.1","fabric-loader":"0.15.0"}}"#)
            .unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn unchanged_packs_are_not_downloaded_again() {
        let dir = tempfile::tempdir().unwrap();
        let provider = LinkedPackProvider::new(reqwest::Client::new(), dir.path().to_path_buf());
        let body = mrpack();
        let (url, downloads) = serve(body.clone()).await;

        let (sha1, path) = provider.fetch_normalized(url.clone()).await.unwrap();
        assert_eq!(sha1, hex::encode(Sha1::digest(&body)));
        let (again, _) = provider.fetch_normalized(url).await.unwrap();

        assert_eq!(again, sha1);
        assert_eq!(downloads.load(std::sync::atomic::Ordering::SeqCst), 1);
        let info = mrpack_info(&path).unwrap();
        assert_eq!(info.name.as_deref(), Some("Potes"));
        assert_eq!(info.loader, crate::providers::LoaderKind::Fabric);
    }

    #[tokio::test]
    async fn web_pages_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let provider = LinkedPackProvider::new(reqwest::Client::new(), dir.path().to_path_buf());
        let (url, _) = serve(b"<html>login</html>".to_vec()).await;
        assert!(provider.fetch_normalized(url).await.is_err());
    }

    #[test]
    fn only_https_links_are_accepted() {
        assert!(normalize_url("http://site/p.mrpack").is_err());
        assert!(normalize_url("file:///C:/p.mrpack").is_err());
        assert!(normalize_url("https://").is_err());
        assert!(normalize_url("https://a b").is_err());
    }
}
