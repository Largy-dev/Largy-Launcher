//! Pre-1.13 Forge installers (1.5.2–1.12.2). No processors: the installer
//! carries the "universal" jar itself (`install.filePath`), and
//! `versionInfo` is a complete legacy version JSON — LaunchWrapper main
//! class, full `minecraftArguments` (with `--tweakClass`), and a library
//! list whose URLs still point at Forge's retired `files.minecraftforge.net`
//! host.

use futures_util::StreamExt;
use serde::Deserialize;

use crate::download::DownloadItem;
use crate::error::AppError;
use crate::minecraft::libraries::maven_path;
use crate::util::fs::safe_join;

use super::super::{LibraryEntry, LoaderContext, LoaderError, LoaderProfile};

#[derive(Debug, Deserialize)]
struct LegacyProfile {
    install: LegacyInstall,
    #[serde(rename = "versionInfo")]
    version_info: LegacyVersionInfo,
}

#[derive(Debug, Deserialize)]
struct LegacyInstall {
    path: String,
    #[serde(rename = "filePath")]
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct LegacyVersionInfo {
    #[serde(rename = "mainClass")]
    main_class: String,
    #[serde(rename = "minecraftArguments")]
    minecraft_arguments: String,
    #[serde(default)]
    libraries: Vec<LegacyLibrary>,
}

#[derive(Debug, Deserialize)]
struct LegacyLibrary {
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    clientreq: Option<bool>,
}

/// Where a legacy library may live: its declared repo first, then the two
/// repos old Forge versions' libraries were mirrored to.
fn candidate_urls(declared: Option<&str>, rel: &str) -> Vec<String> {
    let mut bases = vec![modernize_repo_url(declared.unwrap_or("https://libraries.minecraft.net/"))];
    for fallback in ["https://maven.minecraftforge.net/", "https://repo1.maven.org/maven2/"] {
        if !bases.iter().any(|b| b == fallback) {
            bases.push(fallback.to_string());
        }
    }
    bases.into_iter().map(|base| format!("{base}{rel}")).collect()
}

/// Maps Forge's retired maven hosts (and plain http) onto the live one.
fn modernize_repo_url(url: &str) -> String {
    let url = url
        .replace("http://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/")
        .replace("https://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/")
        .replacen("http://", "https://", 1);
    if url.ends_with('/') {
        url
    } else {
        format!("{url}/")
    }
}

pub(super) async fn install(
    ctx: &LoaderContext<'_>,
    archive: &mut zip::ZipArchive<std::fs::File>,
    raw: serde_json::Value,
) -> Result<LoaderProfile, LoaderError> {
    let profile: LegacyProfile = serde_json::from_value(raw)?;
    let libraries_dir = ctx.paths.libraries_dir();

    // The loader's own jar comes out of the installer, not from a repo.
    let forge_rel = maven_path(&profile.install.path)
        .ok_or_else(|| LoaderError::Other(format!("coordonnée Forge invalide: {}", profile.install.path)))?;
    let forge_dest = safe_join(&libraries_dir, &forge_rel)
        .ok_or_else(|| LoaderError::Other(format!("chemin Forge invalide: {forge_rel}")))?;
    if !forge_dest.exists() || ctx.force_reinstall {
        let mut entry = archive.by_name(&profile.install.file_path)?;
        if let Some(parent) = forge_dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = forge_dest.with_extension("jar.part");
        std::io::copy(&mut entry, &mut std::fs::File::create(&tmp)?)?;
        std::fs::rename(&tmp, &forge_dest)?;
    }

    let mut jobs: Vec<(Vec<String>, std::path::PathBuf)> = Vec::new();
    let mut extra_libraries = Vec::new();
    for lib in &profile.version_info.libraries {
        if lib.clientreq == Some(false) {
            continue;
        }
        let Some(rel) = maven_path(&lib.name) else {
            continue;
        };
        let Some(dest) = safe_join(&libraries_dir, &rel) else {
            continue;
        };
        if lib.name != profile.install.path {
            jobs.push((candidate_urls(lib.url.as_deref(), &rel), dest.clone()));
        }
        extra_libraries.push(LibraryEntry { name: lib.name.clone(), path: dest });
    }

    let downloader = ctx.downloader;
    let failures: Vec<String> = futures_util::stream::iter(jobs)
        .map(|(urls, dest)| async move {
            let mut last_error = None;
            for url in urls {
                let item = DownloadItem { url, dest: dest.clone(), sha1: None, size: None };
                match downloader.ensure_file(&item).await {
                    Ok(_) => return None,
                    Err(e) => last_error = Some(format!("{}: {e}", item.url)),
                }
            }
            last_error
        })
        .buffer_unordered(8)
        .filter_map(|failure| async move { failure })
        .collect()
        .await;
    if !failures.is_empty() {
        return Err(AppError::Download(format!("bibliothèques Forge introuvables :\n{}", failures.join("\n"))).into());
    }

    Ok(LoaderProfile {
        extra_libraries,
        main_class_override: Some(profile.version_info.main_class),
        extra_jvm_args: Vec::new(),
        extra_game_args: Vec::new(),
        game_args_override: Some(
            profile.version_info.minecraft_arguments.split_whitespace().map(String::from).collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_forge_maven_urls_are_rewritten() {
        assert_eq!(
            modernize_repo_url("http://files.minecraftforge.net/maven/"),
            "https://maven.minecraftforge.net/"
        );
        assert_eq!(modernize_repo_url("https://libraries.minecraft.net"), "https://libraries.minecraft.net/");
    }

    #[test]
    fn candidate_urls_try_the_declared_repo_then_mirrors() {
        assert_eq!(
            candidate_urls(Some("http://files.minecraftforge.net/maven/"), "a/b/1/b-1.jar"),
            vec![
                "https://maven.minecraftforge.net/a/b/1/b-1.jar".to_string(),
                "https://repo1.maven.org/maven2/a/b/1/b-1.jar".to_string(),
            ]
        );
        assert_eq!(candidate_urls(None, "x.jar")[0], "https://libraries.minecraft.net/x.jar");
    }

    #[test]
    fn legacy_profile_parses_the_1_12_2_shape() {
        let json = serde_json::json!({
            "install": {
                "path": "net.minecraftforge:forge:1.12.2-14.23.5.2860",
                "filePath": "forge-1.12.2-14.23.5.2860-universal.jar"
            },
            "versionInfo": {
                "mainClass": "net.minecraft.launchwrapper.Launch",
                "minecraftArguments": "--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
                "libraries": [
                    {"name": "net.minecraftforge:forge:1.12.2-14.23.5.2860", "url": "http://files.minecraftforge.net/maven/"},
                    {"name": "net.minecraft:launchwrapper:1.12", "serverreq": true}
                ]
            }
        });
        let profile: LegacyProfile = serde_json::from_value(json).unwrap();
        assert_eq!(profile.version_info.libraries.len(), 2);
        assert_eq!(profile.install.file_path, "forge-1.12.2-14.23.5.2860-universal.jar");
    }
}
