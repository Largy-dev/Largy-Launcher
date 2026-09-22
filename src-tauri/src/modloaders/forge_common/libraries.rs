//! Turning a Forge/NeoForge install profile's raw library list into
//! downloadable items and runtime classpath entries.

use std::collections::HashMap;
use std::path::Path;

use crate::download::DownloadItem;
use crate::minecraft::libraries::{group_artifact, maven_path};
use crate::minecraft::manifest::RawLibrary;

use super::super::LibraryEntry;

/// Splits a library list into ones that need downloading over HTTP and
/// leaves out entries with no real `downloads.artifact.url` — those are
/// produced locally by the processor chain instead.
pub(super) fn downloadable_items(libraries: &[RawLibrary], libraries_dir: &Path) -> Vec<DownloadItem> {
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
/// merging in `install_profile.libraries` (see
/// [`super::install_from_installer_jar`]).
pub(super) fn dedupe_libraries(libraries: Vec<RawLibrary>) -> Vec<RawLibrary> {
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
pub(super) fn library_entries(libraries: &[RawLibrary], libraries_dir: &Path) -> Vec<LibraryEntry> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::manifest::{LibraryArtifact, LibraryDownloads};

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
