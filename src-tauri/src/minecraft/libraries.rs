//! Turns a version JSON's `libraries` array into concrete download items,
//! resolving Maven coordinates by hand for the entries (mostly Forge/NeoForge
//! -generated) that omit an explicit `downloads` block.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::download::DownloadItem;

use super::manifest::{current_os_name, rules_allow, RawLibrary};

pub struct ResolvedLibraries {
    pub classpath_items: Vec<DownloadItem>,
    /// Native classifier jars (LWJGL, etc.) for the current OS; each must be
    /// unzipped into an instance's `natives/` directory before launch.
    pub native_jars: Vec<PathBuf>,
    /// `group:artifact` -> resolved path, for every classpath entry with a
    /// parseable coordinate. Lets a mod loader's own library set know which
    /// vanilla-provided path to drop when it needs a different version of
    /// the same library (see [`crate::launch::orchestrator`]) — mixing two
    /// versions of one library (e.g. `asm-commons`) on the classpath/module
    /// path crashes the JVM at launch.
    pub library_index: HashMap<String, PathBuf>,
}

/// `group:artifact` identity from a full maven coordinate, ignoring
/// version/classifier — two entries with the same key are the *same*
/// library at (potentially) different versions.
pub fn group_artifact(coord: &str) -> Option<String> {
    let mut parts = coord.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    Some(format!("{group}:{artifact}"))
}

/// `group:artifact:version[:classifier][@ext]` -> the Maven repository-relative path.
pub fn maven_path(coord: &str) -> Option<String> {
    let (coord, ext) = match coord.split_once('@') {
        Some((c, e)) => (c, e.to_string()),
        None => (coord, "jar".to_string()),
    };
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let group_path = group.replace('.', "/");
    let file_name = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{group_path}/{artifact}/{version}/{file_name}"))
}

pub fn resolve_libraries(libraries: &[RawLibrary], libraries_dir: &Path) -> ResolvedLibraries {
    let mut classpath_items = Vec::new();
    let mut native_jars = Vec::new();
    let mut library_index = HashMap::new();

    for lib in libraries {
        if !rules_allow(&lib.rules) {
            continue;
        }

        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                let rel_path = artifact
                    .path
                    .clone()
                    .or_else(|| maven_path(&lib.name))
                    .unwrap_or_else(|| lib.name.replace(':', "/"));
                let dest = libraries_dir.join(&rel_path);
                if let Some(key) = group_artifact(&lib.name) {
                    library_index.insert(key, dest.clone());
                }
                classpath_items.push(DownloadItem {
                    url: artifact.url.clone(),
                    dest,
                    sha1: artifact.sha1.clone(),
                    size: artifact.size,
                });
            }
        } else if let Some(rel_path) = maven_path(&lib.name) {
            let base = lib
                .url
                .clone()
                .unwrap_or_else(|| "https://libraries.minecraft.net/".to_string());
            let base = if base.ends_with('/') { base } else { format!("{base}/") };
            let dest = libraries_dir.join(&rel_path);
            if let Some(key) = group_artifact(&lib.name) {
                library_index.insert(key, dest.clone());
            }
            classpath_items.push(DownloadItem {
                url: format!("{base}{rel_path}"),
                dest,
                sha1: lib.sha1.clone(),
                size: lib.size,
            });
        }

        if let Some(natives_map) = &lib.natives {
            if let Some(classifier_key) = natives_map.get(current_os_name()) {
                let classifier_key = classifier_key.replace("${arch}", "64");
                if let Some(artifact) = lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.classifiers.as_ref())
                    .and_then(|c| c.get(&classifier_key))
                {
                    let rel_path = artifact.path.clone().unwrap_or_else(|| {
                        format!(
                            "{}-{}.jar",
                            lib.name.replace(':', "/"),
                            classifier_key
                        )
                    });
                    let dest = libraries_dir.join(&rel_path);
                    classpath_items.push(DownloadItem {
                        url: artifact.url.clone(),
                        dest: dest.clone(),
                        sha1: artifact.sha1.clone(),
                        size: artifact.size,
                    });
                    native_jars.push(dest);
                }
            }
        }
    }

    ResolvedLibraries {
        classpath_items,
        native_jars,
        library_index,
    }
}

/// Unzips every native jar's contents (skipping `META-INF/`) into `target_dir`,
/// which is where the JVM's `-Djava.library.path` will point at launch.
pub fn extract_natives(native_jars: &[PathBuf], target_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target_dir)?;
    for jar_path in native_jars {
        let file = std::fs::File::open(jar_path)?;
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(a) => a,
            Err(_) => continue,
        };
        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = entry.name().to_string();
            if name.starts_with("META-INF/") || name.ends_with('/') {
                continue;
            }
            let out_name = name.rsplit('/').next().unwrap_or(&name);
            let out_path = target_dir.join(out_name);
            if out_path.exists() {
                continue;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::manifest::{LibraryArtifact, LibraryDownloads};

    #[test]
    fn group_artifact_drops_version_and_classifier() {
        assert_eq!(group_artifact("org.ow2.asm:asm-commons:9.10.1"), Some("org.ow2.asm:asm-commons".to_string()));
        assert_eq!(
            group_artifact("org.lwjgl:lwjgl:3.3.3:natives-windows"),
            Some("org.lwjgl:lwjgl".to_string())
        );
    }

    #[test]
    fn group_artifact_rejects_malformed_coordinates() {
        assert_eq!(group_artifact("just-a-name"), None);
    }

    fn lib_with_version(version: &str) -> RawLibrary {
        RawLibrary {
            name: format!("org.ow2.asm:asm-commons:{version}"),
            downloads: Some(LibraryDownloads {
                artifact: Some(LibraryArtifact {
                    path: None,
                    url: format!("https://example.com/asm-commons-{version}.jar"),
                    sha1: None,
                    size: None,
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
    fn resolve_libraries_indexes_every_classpath_entry_by_group_artifact() {
        let libs = vec![lib_with_version("9.3"), lib_with_version("9.10.1")];
        let resolved = resolve_libraries(&libs, Path::new("/libs"));

        // Both versions still get downloaded/placed on the classpath here —
        // it's the orchestrator's job to drop the stale one when a mod
        // loader supplies a different version of the same library.
        assert_eq!(resolved.classpath_items.len(), 2);
        // The index reflects the *last* occurrence for a given key, matching
        // insertion order (later entries overwrite earlier ones).
        let path = resolved.library_index.get("org.ow2.asm:asm-commons").unwrap();
        assert!(path.to_string_lossy().contains("9.10.1"));
    }
}
