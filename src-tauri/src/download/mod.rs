//! Generic concurrent download engine: checksum verification, skip-if-valid,
//! and aggregated progress events emitted to the frontend as `download-progress`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::error::{AppError, AppResult};

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

#[derive(Clone)]
pub struct DownloadManager {
    client: reqwest::Client,
}

impl DownloadManager {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Downloads a single file if missing or failing its checksum. Writes to a
    /// `.part` sibling first so a crash mid-download never leaves a file that
    /// looks valid but isn't.
    pub async fn ensure_file(&self, item: &DownloadItem) -> AppResult<u64> {
        if item.dest.exists() {
            if let Some(expected) = &item.sha1 {
                if let Ok(actual) = sha1_of_file(&item.dest).await {
                    if actual.eq_ignore_ascii_case(expected) {
                        return Ok(0);
                    }
                }
            } else {
                return Ok(0);
            }
        }

        if let Some(parent) = item.dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let response = self
            .client
            .get(&item.url)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| AppError::Download(format!("{} -> {e}", item.url)))?;

        let part_path = item.dest.with_extension(
            item.dest
                .extension()
                .map(|e| format!("{}.part", e.to_string_lossy()))
                .unwrap_or_else(|| "part".to_string()),
        );

        let bytes = response.bytes().await?;

        if let Some(expected) = &item.sha1 {
            let actual = sha1_of_bytes(&bytes);
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(AppError::Download(format!(
                    "checksum mismatch for {}: expected {expected}, got {actual}",
                    item.url
                )));
            }
        }

        let mut file = tokio::fs::File::create(&part_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&part_path, &item.dest).await?;

        Ok(bytes.len() as u64)
    }

    /// Downloads every item with bounded concurrency, emitting aggregated
    /// `download-progress` events on `app` under `task_id` as bytes complete.
    pub async fn run_batch(
        &self,
        app: &AppHandle,
        task_id: &str,
        label: &str,
        items: Vec<DownloadItem>,
        concurrency: usize,
    ) -> AppResult<()> {
        let files_total = items.len() as u32;
        let bytes_total: u64 = items.iter().filter_map(|i| i.size).sum();
        let bytes_done = Arc::new(AtomicU64::new(0));
        let files_done = Arc::new(AtomicU64::new(0));
        let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));

        emit_progress(app, task_id, label, 0, bytes_total, 0, files_total);

        let mut set = JoinSet::new();
        for item in items {
            let manager = self.clone();
            let permit = semaphore.clone();
            let bytes_done = bytes_done.clone();
            let files_done = files_done.clone();
            let app = app.clone();
            let task_id = task_id.to_string();
            let label = label.to_string();
            let item_size = item.size.unwrap_or(0);

            set.spawn(async move {
                let _permit = permit.acquire().await.expect("semaphore closed");
                let result = manager.ensure_file(&item).await;
                let done = files_done.fetch_add(1, Ordering::SeqCst) + 1;
                bytes_done.fetch_add(item_size, Ordering::SeqCst);
                emit_progress(
                    &app,
                    &task_id,
                    &label,
                    bytes_done.load(Ordering::SeqCst),
                    bytes_total,
                    done as u32,
                    files_total,
                );
                result.map_err(|e| format!("{}: {e}", item.url))
            });
        }

        let mut errors = Vec::new();
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => errors.push(e),
                Err(join_err) => errors.push(join_err.to_string()),
            }
        }

        if !errors.is_empty() {
            return Err(AppError::Download(format!(
                "{} of {} downloads failed:\n{}",
                errors.len(),
                files_total,
                errors.join("\n")
            )));
        }

        Ok(())
    }
}

fn emit_progress(
    app: &AppHandle,
    task_id: &str,
    label: &str,
    bytes_done: u64,
    bytes_total: u64,
    files_done: u32,
    files_total: u32,
) {
    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            task_id: task_id.to_string(),
            label: label.to_string(),
            bytes_done,
            bytes_total,
            files_done,
            files_total,
        },
    );
}

pub fn sha1_of_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub async fn sha1_of_file(path: &std::path::Path) -> AppResult<String> {
    let bytes = tokio::fs::read(path).await?;
    Ok(sha1_of_bytes(&bytes))
}
