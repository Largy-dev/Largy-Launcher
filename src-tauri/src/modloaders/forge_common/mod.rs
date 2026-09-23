//! Shared installer-jar/"install profile" execution used by both Forge and
//! NeoForge (NeoForge is a fork that reuses the exact same installer
//! technology). Two installer generations exist:
//!
//! - modern (1.13+): `install_profile.json` + `version.json` with a
//!   `processors` graph whose jars patch the vanilla client jar in place;
//! - legacy (1.5.2–1.12.2): `install_profile.json` with `install` +
//!   `versionInfo`, the universal jar embedded in the installer, and a
//!   LaunchWrapper main class — see [`legacy`].

mod legacy;
mod libraries;
mod processor;
mod zip_resolve;

use std::collections::HashMap;

use serde::Deserialize;

use crate::download::DownloadItem;
use crate::minecraft::manifest::{self, RawLibrary, RawVersionJson};

use super::{LoaderContext, LoaderError, LoaderProfile};
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

/// Every `<version>` in a Maven `maven-metadata.xml`, in document order.
pub fn maven_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|chunk| chunk.split("</version>").next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn open_installer(path: &std::path::Path) -> Result<zip::ZipArchive<std::fs::File>, LoaderError> {
    let file = std::fs::File::open(path)?;
    zip::ZipArchive::new(file).map_err(|e| {
        // A jar that doesn't open is useless; drop it so the next attempt
        // downloads it again instead of failing on the same bytes forever.
        let _ = std::fs::remove_file(path);
        LoaderError::Other(format!("installeur corrompu ({e}), réessaie"))
    })
}

/// Downloads (cached) the installer jar, installs whatever it needs into the
/// shared library cache, and returns the runtime profile.
pub async fn install_from_installer_jar(
    ctx: &LoaderContext<'_>,
    mc_version: &str,
    installer_url: &str,
    cache_key: &str,
) -> Result<LoaderProfile, LoaderError> {
    let paths = ctx.paths;
    let installer_path = paths.installers_dir().join(format!("{cache_key}-installer.jar"));
    ctx.downloader
        .ensure_file(&DownloadItem { url: installer_url.to_string(), dest: installer_path.clone(), sha1: None, size: None })
        .await?;

    let mut archive = open_installer(&installer_path)?;
    let install_profile_text = read_zip_text(&mut archive, "install_profile.json")?;
    let install_profile_raw: serde_json::Value = serde_json::from_str(&install_profile_text)?;

    if install_profile_raw.get("versionInfo").is_some() {
        return legacy::install(ctx, &mut archive, install_profile_raw).await;
    }
    if install_profile_raw.get("processors").is_none() {
        return Err(unsupported_installer_error(mc_version));
    }

    let install_profile: InstallProfile = serde_json::from_value(install_profile_raw)?;
    let version_json: RawVersionJson = serde_json::from_str(&read_zip_text(&mut archive, "version.json")?)?;

    let mut all_libraries = install_profile.libraries.clone();
    all_libraries.extend(version_json.libraries.clone());

    // Libraries without a download URL ship inside the installer's `maven/`
    // folder (e.g. the Forge jar itself on 1.12.2–1.16); the official
    // installer extracts them, so must we. Processor outputs aren't in there
    // and are simply skipped.
    for lib in &all_libraries {
        let embedded = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_some_and(|a| a.url.is_empty());
        let Some(rel) = embedded.then(|| crate::minecraft::libraries::maven_path(&lib.name)).flatten() else {
            continue;
        };
        if let Some(dest) = crate::util::fs::safe_join(&paths.libraries_dir(), &rel) {
            zip_resolve::extract_zip_entry(&mut archive, &format!("maven/{rel}"), &dest)?;
        }
    }
    ctx.downloader
        .run_batch(
            ctx.app,
            "loader-libraries",
            "Bibliothèques Forge/NeoForge",
            downloadable_items(&all_libraries, &paths.libraries_dir()),
            8,
        )
        .await?;

    let marker = paths.libraries_dir().join(".installed").join(format!("{cache_key}.done"));
    if ctx.force_reinstall {
        let _ = std::fs::remove_file(&marker);
    }

    if !marker.exists() {
        let data = install_profile.data.clone();
        let processors = install_profile.processors.clone();
        let paths_owned = paths.clone();
        let mc_version_owned = mc_version.to_string();
        let installer_path_owned = installer_path.clone();
        let cache_key_owned = cache_key.to_string();
        // Resolving the plan walks the installer jar (sync I/O): blocking pool.
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

        let runtime = ctx.java.ensure_runtime(ctx.app, paths, ctx.downloader, ctx.java_component).await?;
        let libraries_dir = paths.libraries_dir();
        for (jar, classpath, args) in &runnable_processors {
            run_processor(jar, classpath, args, &libraries_dir, &runtime.path).await?;
        }

        crate::util::fs::write_atomic(&marker, b"ok")?;
    }

    // The runtime classpath comes from version.json alone, never from
    // install_profile.libraries: several install_profile entries (e.g.
    // AutoRenamingTool, a shaded jar bundling its own gson) are
    // install-time-only processor dependencies. Putting them on the game's
    // module path makes the JPMS resolver see one package exported by two
    // modules and refuse to launch.
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
        // Late 1.12.2 installers use this format but still ship legacy
        // `minecraftArguments` (with `--tweakClass`), which replace vanilla's.
        game_args_override: version_json
            .legacy_arguments
            .as_ref()
            .map(|args| args.split_whitespace().map(String::from).collect()),
    })
}

pub fn unsupported_installer_error(mc_version: &str) -> LoaderError {
    LoaderError::UnsupportedVersion(format!(
        "L'installeur Forge/NeoForge pour Minecraft {mc_version} utilise un format non reconnu. \
         Essaie une autre version du mod loader."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_versions_parses_metadata_xml() {
        let xml = "<metadata><versioning><versions>\
                     <version>20.4.190</version>\
                     <version> 20.4.191 </version>\
                   </versions></versioning></metadata>";
        assert_eq!(maven_versions(xml), vec!["20.4.190".to_string(), "20.4.191".to_string()]);
        assert!(maven_versions("<metadata></metadata>").is_empty());
    }
}
