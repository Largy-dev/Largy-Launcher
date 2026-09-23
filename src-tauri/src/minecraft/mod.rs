//! Mojang piston-meta version resolution, asset/library downloading, and
//! launch-argument assembly. [`prepare_version`] is the single entry point:
//! it fetches the version JSON, downloads the client jar + libraries +
//! assets, and returns everything the launch orchestrator needs.

pub mod assets;
pub mod launch_args;
pub mod libraries;
pub mod manifest;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

use tauri::AppHandle;

use crate::download::{DownloadItem, DownloadManager};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::util::http_cache::MetaCache;
use crate::util::version::compare_versions;
use manifest::RawArguments;

pub struct ClasspathEntry {
    pub name: String,
    pub path: PathBuf,
}

pub struct PreparedVersion {
    pub id: String,
    pub version_type: String,
    pub main_class: String,
    /// Modern (1.13+) rule-based arguments, flattened by the caller once it
    /// knows which optional features (custom resolution, quick play) apply.
    pub arguments: Option<RawArguments>,
    /// Pre-1.13 `minecraftArguments`, already split.
    pub legacy_game_args: Vec<String>,
    pub classpath: Vec<ClasspathEntry>,
    pub native_jars: Vec<PathBuf>,
    pub java_component: String,
    pub java_major: u32,
    pub asset_index_id: String,
    pub library_index: HashMap<String, PathBuf>,
    /// Extra JVM args hardening log4j against Log4Shell.
    pub logging_jvm_args: Vec<String>,
    /// The game logs log4j XML events (Mojang's hardened config for old
    /// log4j versions) that the log reader must decode.
    pub xml_logs: bool,
}

/// JVM flags the launcher always sets itself and so must not appear twice.
pub fn strip_builtin_jvm_args(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "-cp" && args.get(i + 1).map(String::as_str) == Some("${classpath}") {
            i += 2;
            continue;
        }
        if a.starts_with("-Djava.library.path=") || a.starts_with("-Dminecraft.launcher.brand=") {
            i += 1;
            continue;
        }
        out.push(a.clone());
        i += 1;
    }
    out
}

/// log4j below 2.10 ignores `formatMsgNoLookups`, so those versions need
/// Mojang's patched logging config to be safe from Log4Shell.
fn needs_hardened_log_config(libraries: &[manifest::RawLibrary]) -> bool {
    libraries
        .iter()
        .find(|l| l.name.starts_with("org.apache.logging.log4j:log4j-core:"))
        .and_then(|l| libraries::coordinate_version(&l.name))
        .is_some_and(|v| compare_versions(v, "2.10") == Ordering::Less)
}

pub async fn prepare_version(
    app: &AppHandle,
    paths: &AppPaths,
    meta: &MetaCache,
    downloader: &DownloadManager,
    mc_version: &str,
) -> AppResult<PreparedVersion> {
    let entry = manifest::find_version_entry(meta, mc_version).await?;
    let raw = manifest::fetch_version_json(meta, &entry.url).await?;

    let version_dir = paths.versions_dir().join(&raw.id);
    let client_jar_dest = version_dir.join(format!("{}.jar", raw.id));
    let client_download = raw
        .downloads
        .client
        .as_ref()
        .ok_or_else(|| AppError::Other(format!("la version {} n'a pas de client téléchargeable", raw.id)))?;

    let resolved = libraries::resolve_libraries(&raw.libraries, &paths.libraries_dir());
    let mut items = resolved.download_items();
    items.push(DownloadItem {
        url: client_download.url.clone(),
        dest: client_jar_dest.clone(),
        sha1: Some(client_download.sha1.clone()),
        size: Some(client_download.size),
    });

    let mut logging_jvm_args = vec!["-Dlog4j2.formatMsgNoLookups=true".to_string()];
    let mut xml_logs = false;
    if needs_hardened_log_config(&raw.libraries) {
        if let Some(client) = raw.logging.as_ref().and_then(|l| l.client.as_ref()) {
            let dest = paths.log_configs_dir().join(&client.file.id);
            items.push(DownloadItem {
                url: client.file.url.clone(),
                dest: dest.clone(),
                sha1: Some(client.file.sha1.clone()),
                size: Some(client.file.size),
            });
            logging_jvm_args.push(client.argument.replace("${path}", &dest.display().to_string()));
            xml_logs = true;
        }
    }

    downloader.run_batch(app, "libraries", "Bibliothèques", items, 16).await?;

    if let Some(asset_index) = &raw.asset_index {
        assets::prepare_assets(app, downloader, &paths.assets_dir(), asset_index).await?;
    }

    let legacy_game_args = raw
        .legacy_arguments
        .as_ref()
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    let mut classpath: Vec<ClasspathEntry> = resolved
        .classpath
        .into_iter()
        .map(|l| ClasspathEntry { name: l.name, path: l.item.dest })
        .collect();
    classpath.push(ClasspathEntry { name: format!("com.mojang:minecraft:{}", raw.id), path: client_jar_dest });

    Ok(PreparedVersion {
        java_component: crate::java::resolve_component(raw.java_version.as_ref()),
        java_major: raw.java_version.as_ref().map(|j| j.major_version).unwrap_or(8),
        version_type: raw.kind.clone().unwrap_or_else(|| "release".to_string()),
        asset_index_id: raw.assets.clone().unwrap_or_else(|| "legacy".to_string()),
        id: raw.id,
        main_class: raw.main_class,
        arguments: raw.arguments,
        legacy_game_args,
        classpath,
        native_jars: resolved.natives.into_iter().map(|i| i.dest).collect(),
        library_index: resolved.library_index,
        logging_jvm_args,
        xml_logs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use manifest::RawLibrary;

    fn lib(name: &str) -> RawLibrary {
        RawLibrary { name: name.to_string(), downloads: None, rules: None, natives: None, url: None, sha1: None, size: None }
    }

    #[test]
    fn old_log4j_needs_the_hardened_config_but_modern_log4j_does_not() {
        assert!(needs_hardened_log_config(&[lib("org.apache.logging.log4j:log4j-core:2.8.1")]));
        assert!(needs_hardened_log_config(&[lib("org.apache.logging.log4j:log4j-core:2.0-beta9")]));
        assert!(!needs_hardened_log_config(&[lib("org.apache.logging.log4j:log4j-core:2.17.0")]));
        assert!(!needs_hardened_log_config(&[lib("com.example:other:1.0")]));
    }

    #[test]
    fn strip_builtin_jvm_args_removes_classpath_and_library_path() {
        let args = vec![
            "-Djava.library.path=${natives_directory}".to_string(),
            "-cp".to_string(),
            "${classpath}".to_string(),
            "-Dfoo=bar".to_string(),
        ];
        assert_eq!(strip_builtin_jvm_args(args), vec!["-Dfoo=bar"]);
    }
}
