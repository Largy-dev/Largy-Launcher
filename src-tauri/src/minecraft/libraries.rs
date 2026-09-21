//! Turns a version JSON's `libraries` array into concrete download items,
//! resolving Maven coordinates by hand for the entries (mostly Forge/NeoForge
//! -generated) that omit an explicit `downloads` block.

use std::path::{Path, PathBuf};

use crate::download::DownloadItem;

use super::manifest::{current_os_name, rules_allow, RawLibrary};

pub struct ResolvedLibraries {
    pub classpath_items: Vec<DownloadItem>,
    /// Native classifier jars (LWJGL, etc.) for the current OS; each must be
    /// unzipped into an instance's `natives/` directory before launch.
    pub native_jars: Vec<PathBuf>,
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
            classpath_items.push(DownloadItem {
                url: format!("{base}{rel_path}"),
                dest,
                sha1: None,
                size: None,
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
