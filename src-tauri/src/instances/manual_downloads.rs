//! Picks up modpack files the player had to download by hand (CurseForge
//! authors who block third-party downloads): once the browser drops them in
//! the Downloads folder, they're moved to their place in the instance.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::download::sha1_of_file;
use crate::error::AppResult;
use crate::util::fs::safe_join;

#[derive(Debug, Clone, Deserialize)]
pub struct ManualFile {
    /// Instance-relative destination, e.g. `mods/foo-1.2.jar`.
    pub path: PathBuf,
    pub sha1: Option<String>,
}

/// Moves every expected file found in `downloads_dir` into `instance_dir`.
/// Returns the instance-relative paths now in place, including files the
/// player already dropped there themselves. A file whose hash doesn't match
/// (another version, or a download still being written) is left alone.
pub async fn collect(downloads_dir: &Path, instance_dir: &Path, files: &[ManualFile]) -> AppResult<Vec<PathBuf>> {
    let mut placed = Vec::new();
    for file in files {
        let Some(dest) = safe_join(instance_dir, &file.path) else {
            continue;
        };
        let Some(name) = file.path.file_name() else {
            continue;
        };
        if dest.is_file() && hash_matches(&dest, file.sha1.as_deref()).await {
            placed.push(file.path.clone());
            continue;
        }
        for candidate in candidates(downloads_dir, &name.to_string_lossy(), file.sha1.is_some()) {
            if !hash_matches(&candidate, file.sha1.as_deref()).await {
                continue;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            move_file(&candidate, &dest)?;
            placed.push(file.path.clone());
            break;
        }
    }
    Ok(placed)
}

/// `name` itself, plus — only when a hash can confirm them — the renamed
/// copies browsers make when the name is taken (`foo (1).jar`).
fn candidates(dir: &Path, name: &str, verifiable: bool) -> Vec<PathBuf> {
    let mut out = vec![dir.join(name)];
    if verifiable {
        let (stem, ext) = name.rsplit_once('.').map_or((name, ""), |(s, e)| (s, e));
        let prefix = format!("{stem} (");
        let suffix = if ext.is_empty() { ")".to_string() } else { format!(").{ext}") };
        if let Ok(entries) = std::fs::read_dir(dir) {
            out.extend(
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(&suffix))
                    }),
            );
        }
    }
    out.into_iter().filter(|p| p.is_file()).collect()
}

async fn hash_matches(path: &Path, sha1: Option<&str>) -> bool {
    match sha1 {
        None => true,
        Some(expected) => sha1_of_file(path).await.is_ok_and(|actual| actual.eq_ignore_ascii_case(expected)),
    }
}

/// Rename, or copy + delete when Downloads is on another drive.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::sha1_of_bytes;

    fn manual(path: &str, bytes: Option<&[u8]>) -> ManualFile {
        ManualFile { path: PathBuf::from(path), sha1: bytes.map(sha1_of_bytes) }
    }

    #[tokio::test]
    async fn moves_matching_downloads_and_skips_wrong_or_missing_ones() {
        let downloads = tempfile::tempdir().unwrap();
        let instance = tempfile::tempdir().unwrap();
        std::fs::write(downloads.path().join("a.jar"), b"A").unwrap();
        std::fs::write(downloads.path().join("b.jar"), b"not B").unwrap();
        std::fs::write(downloads.path().join("c (1).jar"), b"C").unwrap();

        let files = [
            manual("mods/a.jar", Some(b"A")),
            manual("mods/b.jar", Some(b"B")),
            manual("resourcepacks/c.jar", Some(b"C")),
            manual("mods/missing.jar", None),
            manual("../escape.jar", None),
        ];
        let placed = collect(downloads.path(), instance.path(), &files).await.unwrap();

        assert_eq!(placed, vec![PathBuf::from("mods/a.jar"), PathBuf::from("resourcepacks/c.jar")]);
        assert_eq!(std::fs::read(instance.path().join("mods/a.jar")).unwrap(), b"A");
        assert!(!downloads.path().join("a.jar").exists());
        assert!(downloads.path().join("b.jar").exists(), "a mismatched file stays put");
        assert!(!instance.path().join("mods/b.jar").exists());
    }

    #[tokio::test]
    async fn counts_files_already_in_place() {
        let downloads = tempfile::tempdir().unwrap();
        let instance = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(instance.path().join("mods")).unwrap();
        std::fs::write(instance.path().join("mods/a.jar"), b"A").unwrap();

        let placed = collect(downloads.path(), instance.path(), &[manual("mods/a.jar", Some(b"A"))]).await.unwrap();
        assert_eq!(placed, vec![PathBuf::from("mods/a.jar")]);
    }

    #[test]
    fn renamed_copies_are_only_considered_when_verifiable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x (2).jar"), b"").unwrap();
        assert_eq!(candidates(dir.path(), "x.jar", true).len(), 1);
        assert!(candidates(dir.path(), "x.jar", false).is_empty());
    }
}
