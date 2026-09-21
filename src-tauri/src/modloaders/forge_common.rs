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

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use tauri::AppHandle;

use crate::download::{DownloadItem, DownloadManager};
use crate::java::JavaManager;
use crate::minecraft::libraries::{group_artifact, maven_path};
use crate::minecraft::manifest::{self, RawLibrary, RawVersionJson};
use crate::paths::AppPaths;

use super::{LibraryEntry, LoaderError, LoaderProfile};

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

fn read_zip_text(archive: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Result<String, LoaderError> {
    let mut entry = archive.by_name(path)?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Extracts `zip_path` from the installer jar to `dest`. Some `data` entries
/// name a coordinate that is the *output* of a later processor step rather
/// than something embedded in the jar — in that case there's nothing to
/// extract yet, which is fine: we only need `dest`'s path to exist as a
/// classpath location, not its contents.
fn extract_zip_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    zip_path: &str,
    dest: &Path,
) -> Result<(), LoaderError> {
    if dest.exists() {
        return Ok(());
    }
    let mut entry = match archive.by_name(zip_path) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = std::fs::File::create(dest)?;
    std::io::copy(&mut entry, &mut out)?;
    Ok(())
}

/// Resolves one `data`-map value, or one processor argument, against the
/// installer jar. Forge's install profile mixes three conventions for the
/// same string:
/// - `[group:artifact:version]` — a Maven coordinate; extract from this
///   jar's `maven/` folder into the shared library cache (or just compute
///   the path if it's a processor *output* that doesn't exist yet) and
///   substitute the resulting file path.
/// - `'literal value'` — a literal string (a hash, a version string, ...)
///   to use as-is, quotes stripped; never a path.
/// - `/relative/path/in/jar` (no quotes) — a real embedded resource;
///   extract into a scratch folder and substitute the resulting file path.
///
/// Anything else is returned unchanged (e.g. plain task-name arguments).
fn resolve_token(
    archive: &mut zip::ZipArchive<std::fs::File>,
    raw_value: &str,
    libraries_dir: &Path,
    scratch_dir: &Path,
) -> Result<String, LoaderError> {
    if let Some(coord) = raw_value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let rel_path = maven_path(coord)
            .ok_or_else(|| LoaderError::Other(format!("coordonnée maven invalide: {coord}")))?;
        let dest = libraries_dir.join(&rel_path);
        extract_zip_entry(archive, &format!("maven/{rel_path}"), &dest)?;
        return Ok(dest.display().to_string());
    }

    if let Some(literal) = raw_value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(literal.to_string());
    }

    if raw_value.starts_with('/') {
        let zip_path = raw_value.trim_start_matches('/');
        let dest = scratch_dir.join(zip_path);
        extract_zip_entry(archive, zip_path, &dest)?;
        return Ok(dest.display().to_string());
    }

    Ok(raw_value.to_string())
}

fn read_main_class(jar_path: &Path) -> Result<String, LoaderError> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let manifest_text = read_zip_text(&mut archive, "META-INF/MANIFEST.MF")?;
    for line in manifest_text.lines() {
        if let Some(value) = line.strip_prefix("Main-Class:") {
            return Ok(value.trim().to_string());
        }
    }
    Err(LoaderError::Other(format!(
        "pas de Main-Class dans {}",
        jar_path.display()
    )))
}

/// Splits a library list into ones that need downloading over HTTP and
/// leaves out entries with no real `downloads.artifact.url` — those are
/// produced locally by the processor chain instead.
fn downloadable_items(libraries: &[RawLibrary], libraries_dir: &Path) -> Vec<DownloadItem> {
    libraries
        .iter()
        .filter_map(|lib| {
            let artifact = lib.downloads.as_ref()?.artifact.as_ref()?;
            if artifact.url.is_empty() {
                return None;
            }
            let rel_path = artifact
                .path
                .clone()
                .or_else(|| maven_path(&lib.name))
                .unwrap_or_else(|| lib.name.replace(':', "/"));
            Some(DownloadItem {
                url: artifact.url.clone(),
                dest: libraries_dir.join(rel_path),
                sha1: artifact.sha1.clone(),
                size: artifact.size,
            })
        })
        .collect()
}

/// Keeps only the last occurrence of each `group:artifact`. Applied to
/// `version.json`'s own library list as a defensive net in case it ever
/// declares the same coordinate twice — the real duplicate-library fix is
/// building the runtime classpath from `version.json` alone rather than
/// merging in `install_profile.libraries` (see [`install_from_installer_jar`]).
fn dedupe_libraries(libraries: Vec<RawLibrary>) -> Vec<RawLibrary> {
    let mut keyed: HashMap<String, RawLibrary> = HashMap::new();
    let mut unkeyed: Vec<RawLibrary> = Vec::new();
    for lib in libraries {
        match group_artifact(&lib.name) {
            Some(key) => {
                keyed.insert(key, lib);
            }
            None => unkeyed.push(lib),
        }
    }
    unkeyed.into_iter().chain(keyed.into_values()).collect()
}

/// Every library becomes a classpath entry regardless of how it got onto
/// disk (downloaded now, downloaded earlier, or just produced by a processor).
fn library_entries(libraries: &[RawLibrary], libraries_dir: &Path) -> Vec<LibraryEntry> {
    libraries
        .iter()
        .filter_map(|lib| {
            let rel_path = lib
                .downloads
                .as_ref()
                .and_then(|d| d.artifact.as_ref())
                .and_then(|a| a.path.clone())
                .or_else(|| maven_path(&lib.name))?;
            Some(LibraryEntry {
                name: lib.name.clone(),
                url: lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.artifact.as_ref())
                    .map(|a| a.url.clone())
                    .unwrap_or_default(),
                sha1: lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).and_then(|a| a.sha1.clone()),
                path: libraries_dir.join(rel_path),
            })
        })
        .collect()
}

/// Runs one processor with already fully-resolved `args` (placeholders
/// substituted and any embedded `[coord]`/`'literal'` tokens resolved).
async fn run_processor(jar: &str, classpath: &[String], args: &[String], libraries_dir: &Path, java_path: &Path) -> Result<(), LoaderError> {
    let jar_rel = maven_path(jar).ok_or_else(|| LoaderError::Other(format!("coordonnée maven invalide: {jar}")))?;
    let jar_path = libraries_dir.join(jar_rel);
    let main_class = read_main_class(&jar_path)?;

    let mut full_classpath = vec![jar_path.display().to_string()];
    full_classpath.extend(classpath.iter().cloned());
    let separator = if cfg!(windows) { ";" } else { ":" };

    let mut cmd = tokio::process::Command::new(java_path);
    cmd.arg("-cp").arg(full_classpath.join(separator)).arg(&main_class).args(args);
    crate::process_ext::hide_console_window(&mut cmd);
    let output = cmd.output().await?;

    if !output.status.success() {
        return Err(LoaderError::Other(format!(
            "processeur {main_class} a échoué:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

fn substitute_placeholders(arg: &str, placeholders: &HashMap<String, String>) -> String {
    let mut result = arg.to_string();
    for (key, value) in placeholders {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
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
        let scratch_dir = paths.installers_dir().join("extracted").join(cache_key);
        let libraries_dir = paths.libraries_dir();

        let mut placeholders = HashMap::new();
        let minecraft_jar = paths.versions_dir().join(mc_version).join(format!("{mc_version}.jar"));
        placeholders.insert("SIDE".to_string(), "client".to_string());
        placeholders.insert("MINECRAFT_JAR".to_string(), minecraft_jar.display().to_string());
        placeholders.insert("MINECRAFT_VERSION".to_string(), mc_version.to_string());
        placeholders.insert("ROOT".to_string(), paths.installers_dir().display().to_string());
        placeholders.insert("INSTALLER".to_string(), installer_path.display().to_string());
        placeholders.insert("LIBRARY_DIR".to_string(), libraries_dir.display().to_string());

        for (key, entry) in &install_profile.data {
            let resolved = resolve_token(&mut archive, &entry.client, &libraries_dir, &scratch_dir)?;
            placeholders.insert(key.clone(), resolved);
        }

        // Resolve every processor's classpath/args now, while the jar is
        // still open — a `[coord]` can appear directly in `args` (not just
        // via a `{TOKEN}`), which needs the same jar-extraction as `data`.
        let mut runnable_processors = Vec::new();
        for processor in &install_profile.processors {
            if !processor.sides.is_empty() && !processor.sides.iter().any(|s| s == "client") {
                continue;
            }
            let classpath = processor
                .classpath
                .iter()
                .filter_map(|coord| maven_path(coord).map(|rel| libraries_dir.join(rel).display().to_string()))
                .collect::<Vec<_>>();
            let mut args = Vec::with_capacity(processor.args.len());
            for raw_arg in &processor.args {
                let substituted = substitute_placeholders(raw_arg, &placeholders);
                args.push(resolve_token(&mut archive, &substituted, &libraries_dir, &scratch_dir)?);
            }
            runnable_processors.push((processor.jar.clone(), classpath, args));
        }
        drop(archive);

        let vanilla_entry = manifest::find_version_entry(client, mc_version).await.map_err(to_loader_err)?;
        let vanilla_raw = manifest::fetch_version_json(client, &vanilla_entry.url).await.map_err(to_loader_err)?;
        let java_component = crate::java::resolve_component(vanilla_raw.java_version.as_ref());
        let runtime = java.ensure_runtime(app, paths, &java_component).await.map_err(to_loader_err)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::manifest::{LibraryArtifact, LibraryDownloads};

    #[test]
    fn substitute_placeholders_replaces_all_occurrences() {
        let mut placeholders = HashMap::new();
        placeholders.insert("SIDE".to_string(), "client".to_string());
        placeholders.insert("MINECRAFT_VERSION".to_string(), "1.20.1".to_string());
        let result = substitute_placeholders("--side {SIDE} --version {MINECRAFT_VERSION} {SIDE}", &placeholders);
        assert_eq!(result, "--side client --version 1.20.1 client");
    }

    #[test]
    fn substitute_placeholders_leaves_unknown_tokens_untouched() {
        let placeholders = HashMap::new();
        let result = substitute_placeholders("{UNKNOWN}", &placeholders);
        assert_eq!(result, "{UNKNOWN}");
    }

    fn lib_with_artifact(name: &str, url: &str) -> RawLibrary {
        RawLibrary {
            name: name.to_string(),
            downloads: Some(LibraryDownloads {
                artifact: Some(LibraryArtifact {
                    path: None,
                    url: url.to_string(),
                    sha1: Some("abc123".to_string()),
                    size: Some(42),
                }),
                classifiers: None,
            }),
            rules: None,
            natives: None,
            url: None,
            sha1: None,
            size: None,
        }
    }

    #[test]
    fn downloadable_items_skips_entries_without_url() {
        let libs = vec![
            lib_with_artifact("net.minecraftforge:forge:1.0", "https://example.com/forge.jar"),
            lib_with_artifact("net.minecraftforge:patched:1.0", ""),
        ];
        let items = downloadable_items(&libs, Path::new("/libs"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://example.com/forge.jar");
        assert!(items[0].dest.ends_with("net/minecraftforge/forge/1.0/forge-1.0.jar"));
    }

    #[test]
    fn library_entries_covers_every_library_regardless_of_url() {
        let libs = vec![
            lib_with_artifact("net.minecraftforge:forge:1.0", "https://example.com/forge.jar"),
            lib_with_artifact("net.minecraftforge:patched:1.0", ""),
        ];
        let entries = library_entries(&libs, Path::new("/libs"));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn dedupe_libraries_keeps_the_later_version_json_entry() {
        // install_profile.libraries (install-time) ++ version_json.libraries
        // (runtime) both declaring org.ow2.asm:asm-commons at different
        // versions — the later (version.json) one must win.
        let libs = vec![
            lib_with_artifact("org.ow2.asm:asm-commons:9.3", "https://example.com/asm-9.3.jar"),
            lib_with_artifact("net.minecraftforge:forge:1.0", "https://example.com/forge.jar"),
            lib_with_artifact("org.ow2.asm:asm-commons:9.10.1", "https://example.com/asm-9.10.1.jar"),
        ];

        let deduped = dedupe_libraries(libs);

        assert_eq!(deduped.len(), 2);
        let asm = deduped.iter().find(|l| l.name.starts_with("org.ow2.asm:asm-commons")).unwrap();
        assert_eq!(asm.name, "org.ow2.asm:asm-commons:9.10.1");
    }
}
