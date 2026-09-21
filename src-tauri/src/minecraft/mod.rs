//! Mojang piston-meta version resolution, asset/library downloading, and
//! launch-argument assembly. [`prepare_version`] is the single entry point:
//! it fetches the version JSON, downloads the client jar + libraries +
//! assets, and returns everything [`launch_args`] needs to build a command
//! line. Mod loaders layer their own extra libraries/args on top of this via
//! `launch_args::apply_loader_profile`.

pub mod assets;
pub mod libraries;
pub mod launch_args;
pub mod manifest;

use std::path::PathBuf;

use tauri::AppHandle;

use crate::download::DownloadManager;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

pub struct PreparedVersion {
    pub id: String,
    pub main_class: String,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub classpath: Vec<PathBuf>,
    pub native_jars: Vec<PathBuf>,
    pub java_major_version: u32,
    pub asset_index_id: String,
}

fn strip_builtin_jvm_args(args: Vec<String>) -> Vec<String> {
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

pub async fn prepare_version(
    app: &AppHandle,
    paths: &AppPaths,
    client: &reqwest::Client,
    downloader: &DownloadManager,
    mc_version: &str,
) -> AppResult<PreparedVersion> {
    let entry = manifest::find_version_entry(client, mc_version).await?;
    let raw = manifest::fetch_version_json(client, &entry.url).await?;

    let version_dir = paths.versions_dir().join(&raw.id);
    let client_jar_dest = version_dir.join(format!("{}.jar", raw.id));
    let client_download = raw
        .downloads
        .client
        .as_ref()
        .ok_or_else(|| AppError::Other(format!("version {} has no client download", raw.id)))?;

    let resolved_libs = libraries::resolve_libraries(&raw.libraries, &paths.libraries_dir());

    let mut items = resolved_libs.classpath_items.clone();
    items.push(crate::download::DownloadItem {
        url: client_download.url.clone(),
        dest: client_jar_dest.clone(),
        sha1: Some(client_download.sha1.clone()),
        size: Some(client_download.size),
    });

    downloader
        .run_batch(app, "libraries", "Bibliothèques", items, 16)
        .await?;

    if let Some(asset_index) = &raw.asset_index {
        assets::prepare_assets(app, downloader, &paths.assets_dir(), asset_index).await?;
    }

    let (jvm_args, game_args) = match &raw.arguments {
        Some(args) => (
            strip_builtin_jvm_args(manifest::flatten_args(&args.jvm)),
            manifest::flatten_args(&args.game),
        ),
        None => {
            let legacy_game_args = raw
                .legacy_arguments
                .as_ref()
                .map(|s| s.split_whitespace().map(String::from).collect())
                .unwrap_or_default();
            (Vec::new(), legacy_game_args)
        }
    };

    let mut classpath: Vec<PathBuf> = resolved_libs
        .classpath_items
        .iter()
        .map(|i| i.dest.clone())
        .collect();
    classpath.push(client_jar_dest);

    Ok(PreparedVersion {
        id: raw.id,
        main_class: raw.main_class,
        jvm_args,
        game_args,
        classpath,
        native_jars: resolved_libs.native_jars,
        java_major_version: raw.java_version.map(|j| j.major_version).unwrap_or(8),
        asset_index_id: raw.assets.unwrap_or_else(|| "legacy".to_string()),
    })
}
