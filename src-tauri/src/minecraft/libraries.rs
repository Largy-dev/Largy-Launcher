//! Turns a version JSON's `libraries` array into concrete download items,
//! resolving Maven coordinates by hand for the entries (mostly Forge/NeoForge
//! -generated) that omit an explicit `downloads` block.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::download::DownloadItem;
use crate::util::fs::{safe_join, write_atomic};

use super::manifest::{current_os_name, rules_allow, RawLibrary};

pub struct ResolvedLibrary {
    pub name: String,
    pub item: DownloadItem,
}

pub struct ResolvedLibraries {
    pub classpath: Vec<ResolvedLibrary>,
    /// Native classifier jars (LWJGL 2/3 before 1.19) for the current OS;
    /// each is unzipped into an instance's `natives/` directory before launch
    /// rather than put on the classpath.
    pub natives: Vec<DownloadItem>,
    /// Library identity (see [`group_artifact`]) -> resolved path, for every
    /// classpath entry. Lets a mod loader's own library set know which
    /// vanilla-provided path to drop when it needs a different version of
    /// the same library — two versions of one library (e.g. `asm-commons`)
    /// on the classpath/module path crash the JVM at launch.
    pub library_index: HashMap<String, PathBuf>,
}

impl ResolvedLibraries {
    pub fn download_items(&self) -> Vec<DownloadItem> {
        self.classpath.iter().map(|l| l.item.clone()).chain(self.natives.iter().cloned()).collect()
    }
}

/// Library identity from a maven coordinate, ignoring the version:
/// `group:artifact`, plus the classifier when there is one — the LWJGL
/// `natives-windows` jar is a different library from the main LWJGL jar.
pub fn group_artifact(coord: &str) -> Option<String> {
    let coord = coord.split('@').next().unwrap_or(coord);
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    Some(match parts.get(3) {
        Some(classifier) => format!("{}:{}:{classifier}", parts[0], parts[1]),
        None => format!("{}:{}", parts[0], parts[1]),
    })
}

/// Version component of a maven coordinate.
pub fn coordinate_version(coord: &str) -> Option<&str> {
    coord.split('@').next()?.split(':').nth(2)
}

/// `group:artifact:version[:classifier][@ext]` -> the Maven repository-relative path.
pub fn maven_path(coord: &str) -> Option<String> {
    let (coord, ext) = match coord.split_once('@') {
        Some((c, e)) => (c, e.to_string()),
        None => (coord, "jar".to_string()),
    };
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 || parts.iter().any(|p| p.is_empty() || p.contains("..")) {
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
    let mut classpath = Vec::new();
    let mut natives = Vec::new();
    let mut library_index = HashMap::new();

    for lib in libraries {
        if !rules_allow(&lib.rules) {
            continue;
        }

        let artifact_item = match &lib.downloads {
            Some(downloads) => downloads.artifact.as_ref().and_then(|artifact| {
                let rel_path = artifact.path.clone().or_else(|| maven_path(&lib.name))?;
                Some(DownloadItem {
                    url: artifact.url.clone(),
                    dest: safe_join(libraries_dir, rel_path)?,
                    sha1: artifact.sha1.clone(),
                    size: artifact.size,
                })
            }),
            None => maven_path(&lib.name).and_then(|rel_path| {
                let base = lib.url.clone().unwrap_or_else(|| "https://libraries.minecraft.net/".to_string());
                let base = if base.ends_with('/') { base } else { format!("{base}/") };
                Some(DownloadItem {
                    url: format!("{base}{rel_path}"),
                    dest: safe_join(libraries_dir, &rel_path)?,
                    sha1: lib.sha1.clone(),
                    size: lib.size,
                })
            }),
        };
        if let Some(item) = artifact_item {
            if let Some(key) = group_artifact(&lib.name) {
                library_index.insert(key, item.dest.clone());
            }
            classpath.push(ResolvedLibrary { name: lib.name.clone(), item });
        }

        let Some(classifier_key) = lib.natives.as_ref().and_then(|n| n.get(current_os_name())) else {
            continue;
        };
        let classifier_key = classifier_key.replace("${arch}", "64");
        let Some(artifact) = lib
            .downloads
            .as_ref()
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier_key))
        else {
            continue;
        };
        let rel_path = artifact
            .path
            .clone()
            .or_else(|| maven_path(&format!("{}:{classifier_key}", lib.name)));
        if let Some(dest) = rel_path.and_then(|rel| safe_join(libraries_dir, rel)) {
            natives.push(DownloadItem {
                url: artifact.url.clone(),
                dest,
                sha1: artifact.sha1.clone(),
                size: artifact.size,
            });
        }
    }

    ResolvedLibraries { classpath, natives, library_index }
}

const NATIVES_MARKER: &str = ".largy-natives";

/// Unzips every native jar's contents (skipping `META-INF/`) into
/// `target_dir`, the JVM's `-Djava.library.path`. The directory is wiped
/// first whenever `key` (the resolved version) changed, so DLLs from a
/// previous Minecraft version never linger next to the new ones.
pub fn extract_natives(native_jars: &[PathBuf], target_dir: &Path, key: &str) -> std::io::Result<()> {
    let marker = target_dir.join(NATIVES_MARKER);
    let current = std::fs::read_to_string(&marker).unwrap_or_default();
    if current == key && target_dir.exists() {
        return Ok(());
    }
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir)?;
    }
    std::fs::create_dir_all(target_dir)?;

    for jar_path in native_jars {
        let file = std::fs::File::open(jar_path)?;
        let Ok(mut archive) = zip::ZipArchive::new(file) else {
            tracing::warn!("skipping unreadable natives jar {}", jar_path.display());
            continue;
        };
        for i in 0..archive.len() {
            let Ok(mut entry) = archive.by_index(i) else {
                continue;
            };
            let name = entry.name().to_string();
            if name.starts_with("META-INF/") || name.ends_with('/') {
                continue;
            }
            let Some(out_name) = name.rsplit('/').next().filter(|n| !n.is_empty() && *n != "..") else {
                continue;
            };
            let out_path = target_dir.join(out_name);
            if out_path.exists() {
                continue;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }
    write_atomic(&marker, key.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::manifest::{LibraryArtifact, LibraryDownloads};

    #[test]
    fn group_artifact_drops_version_but_keeps_classifier() {
        assert_eq!(group_artifact("org.ow2.asm:asm-commons:9.10.1"), Some("org.ow2.asm:asm-commons".to_string()));
        assert_eq!(
            group_artifact("org.lwjgl:lwjgl:3.3.3:natives-windows"),
            Some("org.lwjgl:lwjgl:natives-windows".to_string())
        );
        assert_eq!(group_artifact("de.oceanlabs.mcp:mcp_config:1.20.1@zip"), Some("de.oceanlabs.mcp:mcp_config".to_string()));
    }

    #[test]
    fn group_artifact_rejects_malformed_coordinates() {
        assert_eq!(group_artifact("just-a-name"), None);
    }

    #[test]
    fn maven_path_builds_repository_layout_and_rejects_traversal() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.0").unwrap(),
            "net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar"
        );
        assert_eq!(
            maven_path("de.oceanlabs.mcp:mcp_config:1.20.1-2023@zip").unwrap(),
            "de/oceanlabs/mcp/mcp_config/1.20.1-2023/mcp_config-1.20.1-2023.zip"
        );
        assert_eq!(maven_path("a:b:../../evil"), None);
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

        assert_eq!(resolved.classpath.len(), 2);
        assert_eq!(resolved.classpath[0].name, "org.ow2.asm:asm-commons:9.3");
        let path = resolved.library_index.get("org.ow2.asm:asm-commons").unwrap();
        assert!(path.to_string_lossy().contains("9.10.1"));
    }

    #[test]
    fn fabric_style_libraries_keep_their_maven_name() {
        let lib = RawLibrary {
            name: "net.fabricmc:intermediary:1.20.1".to_string(),
            downloads: None,
            rules: None,
            natives: None,
            url: Some("https://maven.fabricmc.net".to_string()),
            sha1: None,
            size: None,
        };
        let resolved = resolve_libraries(&[lib], Path::new("/libs"));
        assert_eq!(resolved.classpath[0].name, "net.fabricmc:intermediary:1.20.1");
        assert_eq!(
            resolved.classpath[0].item.url,
            "https://maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar"
        );
    }

    #[test]
    fn extract_natives_wipes_the_folder_when_the_version_changes() {
        let dir = tempfile::tempdir().unwrap();
        let natives = dir.path().join("natives");
        std::fs::create_dir_all(&natives).unwrap();
        std::fs::write(natives.join("stale.dll"), b"old").unwrap();

        extract_natives(&[], &natives, "1.20.1").unwrap();
        assert!(!natives.join("stale.dll").exists());

        std::fs::write(natives.join("kept.dll"), b"x").unwrap();
        extract_natives(&[], &natives, "1.20.1").unwrap();
        assert!(natives.join("kept.dll").exists(), "same key must not re-extract");
    }
}
