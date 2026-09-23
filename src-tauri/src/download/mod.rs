//! Generic concurrent download engine: streamed to disk with incremental
//! checksum verification, retried on transient failures, skip-if-valid, and
//! aggregated progress events emitted to the frontend as `download-progress`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::error::{AppError, AppResult};

const MAX_ATTEMPTS: u32 = 3;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub task_id: String,
    pub label: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: u32,
    pub files_total: u32,
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// How much to trust a file that's already on disk. Every file this engine
/// writes was checksum-verified *before* being renamed into place, so a size
/// match is enough day to day; `Full` re-hashes everything (the "repair" path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verify {
    Fast,
    Full,
}

#[derive(Clone)]
pub struct DownloadManager {
    client: reqwest::Client,
    verify: Verify,
}

enum Failure {
    Retryable(AppError),
    Fatal(AppError),
}

impl DownloadManager {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client, verify: Verify::Fast }
    }

    pub fn with_verify(&self, verify: Verify) -> Self {
        Self { client: self.client.clone(), verify }
    }

    pub fn verify_mode(&self) -> Verify {
        self.verify
    }

    async fn is_already_valid(&self, item: &DownloadItem) -> bool {
        let Ok(meta) = tokio::fs::metadata(&item.dest).await else {
            return false;
        };
        if !meta.is_file() {
            return false;
        }
        if let Some(size) = item.size {
            if meta.len() != size {
                return false;
            }
        }
        match (&item.sha1, self.verify) {
            (Some(expected), Verify::Full) => sha1_of_file(&item.dest)
                .await
                .is_ok_and(|actual| actual.eq_ignore_ascii_case(expected)),
            _ => true,
        }
    }

    /// Downloads a single file if missing or invalid. Returns the number of
    /// bytes actually transferred (0 when the file was already valid).
    pub async fn ensure_file(&self, item: &DownloadItem) -> AppResult<u64> {
        self.ensure_file_with_progress(item, &AtomicU64::new(0)).await
    }

    async fn ensure_file_with_progress(&self, item: &DownloadItem, progress: &AtomicU64) -> AppResult<u64> {
        if self.is_already_valid(item).await {
            return Ok(0);
        }
        if let Some(parent) = item.dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut attempt = 0;
        loop {
            attempt += 1;
            let counted = AtomicU64::new(0);
            match self.try_download(item, progress, &counted).await {
                Ok(bytes) => return Ok(bytes),
                Err(failure) => {
                    progress.fetch_sub(counted.load(Ordering::Relaxed), Ordering::Relaxed);
                    match failure {
                        Failure::Retryable(e) if attempt < MAX_ATTEMPTS => {
                            tracing::warn!("download attempt {attempt} failed for {}: {e}", item.url);
                            tokio::time::sleep(Duration::from_millis(400 * 3u64.pow(attempt - 1))).await;
                        }
                        Failure::Retryable(e) | Failure::Fatal(e) => return Err(e),
                    }
                }
            }
        }
    }

    async fn try_download(&self, item: &DownloadItem, progress: &AtomicU64, counted: &AtomicU64) -> Result<u64, Failure> {
        let response = self
            .client
            .get(&item.url)
            .send()
            .await
            .map_err(|e| Failure::Retryable(e.into()))?;
        let status = response.status();
        if !status.is_success() {
            let err = AppError::Download(format!("{} -> HTTP {status}", item.url));
            let transient = status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS;
            return Err(if transient { Failure::Retryable(err) } else { Failure::Fatal(err) });
        }

        let part_path = part_path_for(&item.dest);
        let result = self.stream_to(response, &part_path, item, progress, counted).await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&part_path).await;
        }
        let written = result?;
        tokio::fs::rename(&part_path, &item.dest)
            .await
            .map_err(|e| Failure::Fatal(e.into()))?;
        Ok(written)
    }

    async fn stream_to(
        &self,
        response: reqwest::Response,
        part_path: &Path,
        item: &DownloadItem,
        progress: &AtomicU64,
        counted: &AtomicU64,
    ) -> Result<u64, Failure> {
        let io = |e: std::io::Error| Failure::Fatal(e.into());
        let file = tokio::fs::File::create(part_path).await.map_err(io)?;
        let mut writer = tokio::io::BufWriter::with_capacity(256 * 1024, file);
        let mut hasher = Sha1::new();
        let mut written: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Failure::Retryable(e.into()))?;
            hasher.update(&chunk);
            writer.write_all(&chunk).await.map_err(io)?;
            written += chunk.len() as u64;
            if item.size.is_some() {
                progress.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                counted.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
        }
        writer.flush().await.map_err(io)?;
        drop(writer);

        if let Some(expected) = &item.sha1 {
            let actual = hex::encode(hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(Failure::Retryable(AppError::Download(format!(
                    "checksum mismatch for {}: expected {expected}, got {actual}",
                    item.url
                ))));
            }
        }
        Ok(written)
    }

    /// Downloads every item with bounded concurrency, emitting aggregated
    /// `download-progress` events on `app` under `task_id` (throttled).
    /// Fails if any file fails.
    pub async fn run_batch(
        &self,
        app: &AppHandle,
        task_id: &str,
        label: &str,
        items: Vec<DownloadItem>,
        concurrency: usize,
    ) -> AppResult<()> {
        let total = items.len();
        let failures = self.run_batch_lenient(app, task_id, label, items, concurrency).await;
        if failures.is_empty() {
            return Ok(());
        }
        let shown: Vec<String> = failures.iter().take(10).map(|(item, e)| format!("{}: {e}", item.url)).collect();
        let more = failures.len().saturating_sub(shown.len());
        Err(AppError::Download(format!(
            "{} of {} downloads failed:\n{}{}",
            failures.len(),
            total,
            shown.join("\n"),
            if more > 0 { format!("\n… et {more} autre(s)") } else { String::new() }
        )))
    }

    /// Like [`Self::run_batch`] but never fails as a whole: returns every
    /// item that couldn't be downloaded, with its error.
    pub async fn run_batch_lenient(
        &self,
        app: &AppHandle,
        task_id: &str,
        label: &str,
        items: Vec<DownloadItem>,
        concurrency: usize,
    ) -> Vec<(DownloadItem, AppError)> {
        let items = dedupe_by_dest(items);
        let files_total = items.len() as u32;
        let bytes_total: u64 = items.iter().filter_map(|i| i.size).sum();
        let bytes_done = Arc::new(AtomicU64::new(0));
        let files_done = Arc::new(AtomicU64::new(0));
        let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));

        let snapshot = {
            let (bytes_done, files_done) = (bytes_done.clone(), files_done.clone());
            let (task_id, label) = (task_id.to_string(), label.to_string());
            move || DownloadProgress {
                task_id: task_id.clone(),
                label: label.clone(),
                bytes_done: bytes_done.load(Ordering::Relaxed).min(bytes_total),
                bytes_total,
                files_done: files_done.load(Ordering::Relaxed) as u32,
                files_total,
            }
        };
        let _ = app.emit("download-progress", snapshot());

        // Aborted on drop too, so a cancelled batch stops reporting.
        struct AbortOnDrop(tokio::task::JoinHandle<()>);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _ticker = {
            let app = app.clone();
            let snapshot = snapshot.clone();
            AbortOnDrop(tokio::spawn(async move {
                let mut interval = tokio::time::interval(PROGRESS_INTERVAL);
                loop {
                    interval.tick().await;
                    let _ = app.emit("download-progress", snapshot());
                }
            }))
        };

        let mut set = JoinSet::new();
        for item in items {
            let manager = self.clone();
            let permit = semaphore.clone();
            let bytes_done = bytes_done.clone();
            let files_done = files_done.clone();
            set.spawn(async move {
                let result = match permit.acquire_owned().await {
                    Ok(_permit) => manager.ensure_file_with_progress(&item, &bytes_done).await,
                    Err(e) => Err(AppError::Other(e.to_string())),
                };
                if let (Ok(0), Some(size)) = (&result, item.size) {
                    // Already valid on disk: count it as done without transferring.
                    bytes_done.fetch_add(size, Ordering::Relaxed);
                }
                files_done.fetch_add(1, Ordering::Relaxed);
                result.err().map(|e| (item, e))
            });
        }

        let mut failures = Vec::new();
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Some(failure)) => failures.push(failure),
                Ok(None) => {}
                Err(join_err) => tracing::error!("download task panicked: {join_err}"),
            }
        }
        let _ = app.emit("download-progress", snapshot());
        failures
    }
}

/// Two items with the same destination would race on the same `.part` file.
fn dedupe_by_dest(items: Vec<DownloadItem>) -> Vec<DownloadItem> {
    let mut seen = std::collections::HashSet::new();
    items.into_iter().filter(|i| seen.insert(i.dest.clone())).collect()
}

/// Unique per download, so two launches fetching the same shared library at
/// once never write into each other's temp file.
fn part_path_for(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".{}.part", &uuid::Uuid::new_v4().simple().to_string()[..8]));
    dest.with_file_name(name)
}

pub fn sha1_of_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Streams the file through the hasher on the blocking pool — never loads
/// a whole (possibly 100+ MB) jar into memory.
pub async fn sha1_of_file(path: &Path) -> AppResult<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> AppResult<String> {
        use std::io::Read;
        let mut file = std::fs::File::open(&path)?;
        let mut hasher = Sha1::new();
        let mut buf = vec![0u8; 128 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    })
    .await
    .map_err(|e| AppError::Other(format!("tâche de fond interrompue: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(dest: PathBuf, sha1: Option<String>, size: Option<u64>) -> DownloadItem {
        DownloadItem { url: "https://example.invalid/should-not-be-fetched".to_string(), dest, sha1, size }
    }

    #[test]
    fn sha1_of_bytes_matches_known_vectors() {
        assert_eq!(sha1_of_bytes(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_of_bytes(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[tokio::test]
    async fn sha1_of_file_streams_the_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(sha1_of_file(&path).await.unwrap(), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[tokio::test]
    async fn ensure_file_skips_existing_file_with_matching_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("already-here.txt");
        std::fs::write(&dest, b"hello").unwrap();

        let manager = DownloadManager::new(reqwest::Client::new()).with_verify(Verify::Full);
        let it = item(dest.clone(), Some(sha1_of_bytes(b"hello")), Some(5));
        assert_eq!(manager.ensure_file(&it).await.unwrap(), 0);
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
    }

    #[tokio::test]
    async fn ensure_file_skips_existing_file_when_no_checksum_requested() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("no-checksum.txt");
        std::fs::write(&dest, b"anything").unwrap();

        let manager = DownloadManager::new(reqwest::Client::new());
        assert_eq!(manager.ensure_file(&item(dest, None, None)).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn fast_verify_trusts_a_size_match_but_full_verify_rehashes() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("corrupt.bin");
        std::fs::write(&dest, b"xxxxx").unwrap();
        let it = item(dest, Some(sha1_of_bytes(b"hello")), Some(5));

        let fast = DownloadManager::new(reqwest::Client::new());
        assert!(fast.is_already_valid(&it).await);
        assert!(!fast.with_verify(Verify::Full).is_already_valid(&it).await);
    }

    #[tokio::test]
    async fn size_mismatch_is_never_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("truncated.bin");
        std::fs::write(&dest, b"hel").unwrap();
        let fast = DownloadManager::new(reqwest::Client::new());
        assert!(!fast.is_already_valid(&item(dest, None, Some(5))).await);
    }

    #[test]
    fn part_paths_are_siblings_and_unique() {
        let a = part_path_for(Path::new("/a/lib-1.0.jar"));
        let b = part_path_for(Path::new("/a/lib-1.0.jar"));
        assert_ne!(a, b);
        assert_eq!(a.parent(), Some(Path::new("/a")));
        let name = a.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("lib-1.0.jar.") && name.ends_with(".part"), "{name}");
    }

    #[test]
    fn dedupe_by_dest_keeps_the_first_item_per_destination() {
        let items = vec![item("/a".into(), None, None), item("/a".into(), None, Some(1)), item("/b".into(), None, None)];
        assert_eq!(dedupe_by_dest(items).len(), 2);
    }
}
