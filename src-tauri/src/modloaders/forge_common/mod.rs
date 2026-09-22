//! Shared installer-jar/"install profile" processor execution used by both
//! Forge and NeoForge (NeoForge is a fork that reuses the exact same
//! installer technology). This is the single hardest part of the launcher:
//! the installer jar ships a small graph of processor jars that patch the
//! vanilla client jar in place to produce the loader-enabled one.
//!
//! Only the modern (~1.13+) `install_profile.json` schema (with a
//! `processors` array) is supported; legacy pre-1.13 Forge installers use a
//! different, simpler-but-still-bespoke format and are rejected with a clear
//! error instead of silently producing a broken install.

mod libraries;
mod processor;
mod zip_resolve;

use std::collections::HashMap;

use serde::Deserialize;
use tauri::AppHandle;

use crate::download::{DownloadItem, DownloadManager};
use crate::java::JavaManager;
use crate::minecraft::manifest::{self, RawLibrary, RawVersionJson};
use crate::paths::AppPaths;

use super::{LoaderError, LoaderProfile};
use libraries::{dedupe_libraries, downloadable_items, library_entries};
use processor::{resolve_processor_plan, run_processor};
use zip_resolve::read_zip_text;

#[derive(Debug, Deserialize, Clone)]
struct DataEntry {
    client: String,
}

#[derive(Debug, Deserialize, Clone)]
struct ProcessorEntry {
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    sides: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct InstallProfile {
    #[serde(default)]
    data: HashMap<String, DataEntry>,
    #[serde(default)]
    processors: Vec<ProcessorEntry>,
    #[serde(default)]
    libraries: Vec<RawLibrary>,
}

fn to_loader_err(e: impl std::fmt::Display) -> LoaderError {
    LoaderError::Other(e.to_string())
}

/// Downloads `installer_url`, runs its install profile's processor chain
/// (skipped if `cache_key` was already installed), and returns the resulting
/// `LoaderProfile`. Shared by Forge and NeoForge.
#[allow(clippy::too_many_arguments)]
pub async fn install_from_installer_jar(
    app: &AppHandle,
    client: &reqwest::Client,
    downloader: &DownloadManager,
    java: &JavaManager,
    paths: &AppPaths,
    mc_version: &str,
    installer_url: &str,
    cache_key: &str,
) -> Result<LoaderProfile, LoaderError> {
    let installer_path = paths.installers_dir().join(format!("{cache_key}-installer.jar"));
    downloader
        .ensure_file(&DownloadItem {
            url: installer_url.to_string(),
            dest: installer_path.clone(),
            sha1: None,
            size: None,
        })
        .await
        .map_err(to_loader_err)?;

    let file = std::fs::File::open(&installer_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let install_profile_text = read_zip_text(&mut archive, "install_profile.json")?;
    let install_profile_raw: serde_json::Value = serde_json::from_str(&install_profile_text)?;
    if install_profile_raw.get("processors").is_none() {
        return Err(unsupported_legacy_error(mc_version));
    }

    let install_profile: InstallProfile = serde_json::from_str(&install_profile_text)?;
    let version_json: RawVersionJson = serde_json::from_str(&read_zip_text(&mut archive, "version.json")?)?;

    let mut all_libraries = install_profile.libraries.clone();
    all_libraries.extend(version_json.libraries.clone());

    downloader
        .run_batch(
            app,
            "loader-libraries",
            "Bibliothèques Forge/NeoForge",
            downloadable_items(&all_libraries, &paths.libraries_dir()),
            8,
        )
        .await
        .map_err(to_loader_err)?;

    let marker = paths.libraries_dir().join(".installed").join(format!("{cache_key}.done"));

    if !marker.exists() {
        let data = install_profile.data.clone();
        let processors = install_profile.processors.clone();
        let paths_owned = paths.clone();
        let mc_version_owned = mc_version.to_string();
        let installer_path_owned = installer_path.clone();
        let cache_key_owned = cache_key.to_string();

        // `placeholders` isn't needed here: `resolve_processor_plan` already
        // baked every resolved value into `runnable_processors`' args.
        let (_placeholders, runnable_processors) = tokio::task::spawn_blocking(move || {
            resolve_processor_plan(
                archive,
                &data,
                &processors,
                &paths_owned,
                &mc_version_owned,
                &installer_path_owned,
                &cache_key_owned,
            )
        })
        .await
        .map_err(|e| LoaderError::Other(format!("tâche de fond interrompue: {e}")))??;

        let vanilla_entry = manifest::find_version_entry(client, mc_version).await.map_err(to_loader_err)?;
        let vanilla_raw = manifest::fetch_version_json(client, &vanilla_entry.url).await.map_err(to_loader_err)?;
        let java_component = crate::java::resolve_component(vanilla_raw.java_version.as_ref());
        let runtime = java.ensure_runtime(app, paths, &java_component).await.map_err(to_loader_err)?;

        let libraries_dir = paths.libraries_dir();
        for (jar, classpath, args) in &runnable_processors {
            run_processor(jar, classpath, args, &libraries_dir, &runtime.path).await?;
        }

        if let Some(parent) = marker.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&marker, "ok")?;
    }

    // The runtime classpath comes from version.json alone, never from
    // install_profile.libraries: several install_profile entries (e.g.
    // AutoRenamingTool, a shaded jar that bundles its own copy of gson) are
    // install-time-only processor dependencies with no place on the actual
    // game's module path. Including them causes the JPMS module resolver to
    // see the same package (e.g. com.google.gson.stream) exported by two
    // modules and refuse to launch at all. `all_libraries` above still
    // includes install_profile's libraries — that's needed so every
    // processor dependency actually gets downloaded to disk.
    let extra_libraries = library_entries(&dedupe_libraries(version_json.libraries.clone()), &paths.libraries_dir());
    let (extra_jvm_args, extra_game_args) = match &version_json.arguments {
        Some(args) => (manifest::flatten_args(&args.jvm), manifest::flatten_args(&args.game)),
        None => (Vec::new(), Vec::new()),
    };

    Ok(LoaderProfile {
        extra_libraries,
        main_class_override: Some(version_json.main_class),
        extra_jvm_args,
        extra_game_args,
        install_side_effects: Vec::new(),
    })
}

pub fn unsupported_legacy_error(mc_version: &str) -> LoaderError {
    LoaderError::UnsupportedVersion(format!(
        "Forge/NeoForge pour Minecraft {mc_version} utilise un format d'installeur trop ancien \
         (pré-1.13), qui n'est pas encore supporté par Largy Launcher. Utilise Fabric ou Quilt \
         pour cette version."
    ))
}
