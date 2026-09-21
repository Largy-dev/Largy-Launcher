//! Mojang-provided Java runtime resolution/download (the same runtimes the
//! official launcher ships) and local JDK detection via `JAVA_HOME`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::download::{DownloadItem, DownloadManager};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

const RUNTIME_INDEX_URL: &str =
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39536781077231585/all.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntime {
    pub component: String,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RuntimeIndex(HashMap<String, HashMap<String, Vec<RuntimeIndexEntry>>>);

#[derive(Debug, Deserialize)]
struct RuntimeIndexEntry {
    manifest: RuntimeManifestRef,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifestRef {
    url: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeManifestFile>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RuntimeManifestFile {
    File {
        downloads: RuntimeFileDownloads,
        // Only consumed by `mark_executable` on Unix; harmless on Windows.
        #[serde(default)]
        #[allow(dead_code)]
        executable: bool,
    },
    Directory,
    Link {
        target: String,
    },
}

#[derive(Debug, Deserialize)]
struct RuntimeFileDownloads {
    raw: RuntimeRawDownload,
}

#[derive(Debug, Deserialize)]
struct RuntimeRawDownload {
    sha1: String,
    size: u64,
    url: String,
}

fn os_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "mac-os-arm64"
        } else {
            "mac-os"
        }
    } else {
        "linux"
    }
}

fn java_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "bin/java.exe"
    } else {
        "bin/java"
    }
}

/// Mojang ships four runtime "flavors"; which one a given `javaVersion.majorVersion`
/// from a version manifest maps to isn't published anywhere machine-readable, so
/// this mirrors the mapping other open-source launchers (PrismLauncher, etc.) use.
pub fn component_for_major(major: u32) -> &'static str {
    if major <= 8 {
        "jre-legacy"
    } else if major <= 16 {
        "java-runtime-alpha"
    } else if major <= 17 {
        "java-runtime-beta"
    } else {
        "java-runtime-gamma"
    }
}

#[derive(Clone)]
pub struct JavaManager {
    client: reqwest::Client,
    downloader: DownloadManager,
}

impl JavaManager {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            downloader: DownloadManager::new(client.clone()),
            client,
        }
    }

    /// Returns the path to a `java` executable satisfying `major_version`,
    /// downloading it into the shared runtime cache on first use.
    pub async fn ensure_runtime(
        &self,
        app: &AppHandle,
        paths: &AppPaths,
        major_version: u32,
    ) -> AppResult<JavaRuntime> {
        let component = component_for_major(major_version);
        let component_dir = paths.runtime_dir().join(component);
        let exe = component_dir.join(java_binary_name());

        if exe.exists() {
            return Ok(JavaRuntime {
                component: component.to_string(),
                path: exe,
            });
        }

        let index: RuntimeIndex = self
            .client
            .get(RUNTIME_INDEX_URL)
            .send()
            .await?
            .json()
            .await?;

        let os_entries = index.0.get(os_key()).ok_or_else(|| {
            AppError::Java(format!("no Java runtime published for platform {}", os_key()))
        })?;

        let entry = os_entries
            .get(component)
            .and_then(|list| list.first())
            .ok_or_else(|| {
                AppError::Java(format!("no {component} runtime published for {}", os_key()))
            })?;

        let manifest: RuntimeManifest = self
            .client
            .get(&entry.manifest.url)
            .send()
            .await?
            .json()
            .await?;

        let mut items = Vec::new();
        let mut links = Vec::new();
        for (rel_path, file) in &manifest.files {
            let dest = component_dir.join(rel_path);
            match file {
                RuntimeManifestFile::File { downloads, .. } => {
                    items.push(DownloadItem {
                        url: downloads.raw.url.clone(),
                        dest,
                        sha1: Some(downloads.raw.sha1.clone()),
                        size: Some(downloads.raw.size),
                    });
                }
                RuntimeManifestFile::Directory => {
                    tokio::fs::create_dir_all(&dest).await.ok();
                }
                RuntimeManifestFile::Link { target } => {
                    links.push((dest, component_dir.join(target)));
                }
            }
        }

        self.downloader
            .run_batch(
                app,
                &format!("java-{component}"),
                &format!("Java ({component})"),
                items,
                8,
            )
            .await?;

        for (link_path, target_path) in links {
            if let Some(parent) = link_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            if target_path.exists() && !link_path.exists() {
                tokio::fs::copy(&target_path, &link_path).await.ok();
            }
        }

        mark_executable(&component_dir, &manifest).await;

        if !exe.exists() {
            return Err(AppError::Java(format!(
                "downloaded {component} runtime but {} is missing",
                exe.display()
            )));
        }

        Ok(JavaRuntime {
            component: component.to_string(),
            path: exe,
        })
    }
}

#[cfg(unix)]
async fn mark_executable(component_dir: &std::path::Path, manifest: &RuntimeManifest) {
    use std::os::unix::fs::PermissionsExt;
    for (rel_path, file) in &manifest.files {
        if let RuntimeManifestFile::File { executable: true, .. } = file {
            let path = component_dir.join(rel_path);
            if let Ok(meta) = tokio::fs::metadata(&path).await {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = tokio::fs::set_permissions(&path, perms).await;
            }
        }
    }
}

#[cfg(not(unix))]
async fn mark_executable(_component_dir: &std::path::Path, _manifest: &RuntimeManifest) {}

/// Looks for a usable local JDK via `JAVA_HOME` before falling back to a
/// Mojang-managed download.
pub fn detect_java_home() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let exe = PathBuf::from(home).join(java_binary_name());
    exe.exists().then_some(exe)
}
