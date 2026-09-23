//! Mojang-provided Java runtime resolution/download (the same runtimes the
//! official launcher ships), plus discovery/probing of Java installs already
//! on the machine for per-instance overrides.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::download::{DownloadItem, DownloadManager, Verify};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::util::fs::{safe_join, write_atomic};
use crate::util::http_cache::{MetaCache, DAILY, IMMUTABLE};

const RUNTIME_INDEX_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const MARKER_FILE: &str = ".largy-runtime.json";

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

/// Written once a runtime finished installing: its presence (not just
/// `java.exe`'s) is what proves the install wasn't interrupted, and the
/// manifest URL tells us when Mojang has published a newer build.
#[derive(Debug, Serialize, Deserialize)]
struct RuntimeMarker {
    manifest_url: String,
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

/// Fallback only: a version JSON's `javaVersion.component` should always be
/// used directly when present (see [`resolve_component`]).
pub fn component_for_major(major: u32) -> &'static str {
    if major <= 8 {
        "jre-legacy"
    } else if major <= 16 {
        "java-runtime-alpha"
    } else if major <= 17 {
        "java-runtime-gamma"
    } else if major <= 21 {
        "java-runtime-delta"
    } else {
        "java-runtime-epsilon"
    }
}

pub fn resolve_component(java_version: Option<&crate::minecraft::manifest::JavaVersionRef>) -> String {
    match java_version {
        Some(jv) => jv
            .component
            .clone()
            .unwrap_or_else(|| component_for_major(jv.major_version).to_string()),
        None => component_for_major(8).to_string(),
    }
}

#[derive(Clone)]
pub struct JavaManager {
    meta: MetaCache,
}

impl JavaManager {
    pub fn new(meta: MetaCache) -> Self {
        Self { meta }
    }

    /// Path to a `java` executable for runtime `component`, downloading or
    /// updating it in the shared runtime cache as needed. Works offline once
    /// installed.
    pub async fn ensure_runtime(
        &self,
        app: &AppHandle,
        paths: &AppPaths,
        downloader: &DownloadManager,
        component: &str,
    ) -> AppResult<JavaRuntime> {
        let component_dir = paths.runtime_dir().join(component);
        let exe = component_dir.join(java_binary_name());
        let marker_path = component_dir.join(MARKER_FILE);
        let marker: Option<RuntimeMarker> =
            std::fs::read(&marker_path).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let ready = || JavaRuntime { component: component.to_string(), path: exe.clone() };

        let manifest_url = match self.meta.get_json::<RuntimeIndex>(RUNTIME_INDEX_URL, DAILY).await {
            Ok(index) => index
                .0
                .get(os_key())
                .and_then(|os| os.get(component))
                .and_then(|list| list.first())
                .map(|e| e.manifest.url.clone())
                .ok_or_else(|| AppError::Java(format!("aucun runtime {component} publié pour {}", os_key())))?,
            Err(e) if marker.is_some() && exe.exists() => {
                tracing::warn!("java runtime index unavailable ({e}), using installed {component}");
                return Ok(ready());
            }
            Err(e) => return Err(e),
        };

        let up_to_date = marker.as_ref().is_some_and(|m| m.manifest_url == manifest_url);
        if up_to_date && exe.exists() && downloader.verify_mode() == Verify::Fast {
            return Ok(ready());
        }

        let manifest: RuntimeManifest = self.meta.get_json(&manifest_url, IMMUTABLE).await?;
        let mut items = Vec::new();
        let mut links = Vec::new();
        for (rel_path, file) in &manifest.files {
            let Some(dest) = safe_join(&component_dir, rel_path) else {
                continue;
            };
            match file {
                RuntimeManifestFile::File { downloads, .. } => items.push(DownloadItem {
                    url: downloads.raw.url.clone(),
                    dest,
                    sha1: Some(downloads.raw.sha1.clone()),
                    size: Some(downloads.raw.size),
                }),
                RuntimeManifestFile::Directory => {
                    tokio::fs::create_dir_all(&dest).await?;
                }
                RuntimeManifestFile::Link { target } => {
                    if let Some(target) = dest.parent().and_then(|p| safe_join(p, target)) {
                        links.push((dest, target));
                    }
                }
            }
        }

        downloader
            .run_batch(app, &format!("java-{component}"), &format!("Java ({component})"), items, 8)
            .await?;

        for (link_path, target_path) in links {
            if target_path.is_file() && !link_path.exists() {
                if let Some(parent) = link_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(&target_path, &link_path).await?;
            }
        }
        mark_executable(&component_dir, &manifest).await;

        if !exe.exists() {
            return Err(AppError::Java(format!("runtime {component} téléchargé mais {} est absent", exe.display())));
        }
        write_atomic(&marker_path, &serde_json::to_vec(&RuntimeMarker { manifest_url })?)?;
        Ok(ready())
    }
}

#[cfg(unix)]
async fn mark_executable(component_dir: &Path, manifest: &RuntimeManifest) {
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
async fn mark_executable(_component_dir: &Path, _manifest: &RuntimeManifest) {}

// ---------------------------------------------------------------------------
// Local Java discovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct JavaInstallation {
    pub path: String,
    pub version: String,
    pub major: u32,
    /// `managed` (downloaded by the launcher) or `system`.
    pub source: &'static str,
}

/// `java -version` prints e.g. `openjdk version "17.0.8" 2023-07-18` or
/// `java version "1.8.0_381"` on stderr.
pub fn parse_java_version(output: &str) -> Option<(String, u32)> {
    let line = output.lines().find(|l| l.contains("version \""))?;
    let version = line.split('"').nth(1)?.to_string();
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    let major = if first == 1 { parts.next()?.parse().ok()? } else { first };
    Some((version, major))
}

/// Runs `java -version` with a timeout; `None` when it isn't a working Java.
pub async fn probe(java: &Path) -> Option<(String, u32)> {
    if !java.is_file() {
        return None;
    }
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
    crate::process_ext::hide_console_window(&mut cmd);
    let output = tokio::time::timeout(Duration::from_secs(8), cmd.output()).await.ok()?.ok()?;
    parse_java_version(&String::from_utf8_lossy(&output.stderr))
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(base) = std::env::var_os(var) {
            let base = PathBuf::from(base);
            for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "BellSoft", "Amazon Corretto", "Semeru"] {
                roots.push(base.join(vendor));
            }
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(home).join(".jdks"));
    }
    let mut dirs = Vec::new();
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            dirs.extend(entries.flatten().map(|e| e.path()));
        }
    }
    dirs
}

/// Every distinct working Java found on the machine plus the launcher's own
/// managed runtimes, newest major first.
pub async fn discover(paths: &AppPaths) -> Vec<JavaInstallation> {
    let mut candidates: Vec<(PathBuf, &'static str)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(paths.runtime_dir()) {
        for entry in entries.flatten() {
            candidates.push((entry.path().join(java_binary_name()), "managed"));
        }
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push((PathBuf::from(home).join(java_binary_name()), "system"));
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let exe = dir.join(if cfg!(windows) { "java.exe" } else { "java" });
            if exe.is_file() {
                candidates.push((exe, "system"));
            }
        }
    }
    for dir in candidate_dirs() {
        candidates.push((dir.join(java_binary_name()), "system"));
    }

    let mut seen = std::collections::HashSet::new();
    let unique: Vec<_> = candidates
        .into_iter()
        .filter(|(p, _)| p.is_file())
        .filter(|(p, _)| seen.insert(std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
        .collect();

    let probes = unique.into_iter().map(|(path, source)| async move {
        probe(&path).await.map(|(version, major)| JavaInstallation {
            path: path.display().to_string(),
            version,
            major,
            source,
        })
    });
    let mut found: Vec<JavaInstallation> = futures_util::future::join_all(probes).await.into_iter().flatten().collect();
    found.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.source.cmp(b.source)));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::manifest::JavaVersionRef;

    #[test]
    fn component_for_major_covers_every_bracket() {
        assert_eq!(component_for_major(8), "jre-legacy");
        assert_eq!(component_for_major(16), "java-runtime-alpha");
        assert_eq!(component_for_major(17), "java-runtime-gamma");
        assert_eq!(component_for_major(21), "java-runtime-delta");
        assert_eq!(component_for_major(22), "java-runtime-epsilon");
    }

    #[test]
    fn resolve_component_prefers_explicit_component_over_major_guess() {
        let jv = JavaVersionRef { component: Some("java-runtime-gamma".to_string()), major_version: 21 };
        assert_eq!(resolve_component(Some(&jv)), "java-runtime-gamma");
    }

    #[test]
    fn resolve_component_falls_back_to_major_guess_when_no_component() {
        let jv = JavaVersionRef { component: None, major_version: 21 };
        assert_eq!(resolve_component(Some(&jv)), "java-runtime-delta");
    }

    #[test]
    fn resolve_component_defaults_to_legacy_when_no_java_version_at_all() {
        assert_eq!(resolve_component(None), "jre-legacy");
    }

    #[test]
    fn parse_java_version_handles_modern_and_legacy_formats() {
        assert_eq!(
            parse_java_version("openjdk version \"17.0.8\" 2023-07-18\nOpenJDK Runtime"),
            Some(("17.0.8".to_string(), 17))
        );
        assert_eq!(parse_java_version("java version \"1.8.0_381\""), Some(("1.8.0_381".to_string(), 8)));
        assert_eq!(parse_java_version("openjdk version \"21\" 2023-09-19"), Some(("21".to_string(), 21)));
        assert_eq!(parse_java_version("garbage"), None);
    }
}
