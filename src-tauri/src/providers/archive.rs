//! Reading modpack archives (CurseForge `.zip`, Modrinth `.mrpack`, Prism
//! exports) safely: every entry name goes through `enclosed_name` (no
//! zip-slip), and extraction lands in a temp folder that is renamed into
//! place only once complete — an interrupted extraction is never mistaken
//! for a finished one.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

fn open(zip_path: &Path) -> AppResult<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(zip_path)?;
    zip::ZipArchive::new(file).map_err(|e| AppError::Provider(format!("archive illisible ({}): {e}", zip_path.display())))
}

/// UTF-8 text of `name` inside the archive, `None` when absent.
pub fn read_text(zip_path: &Path, name: &str) -> AppResult<Option<String>> {
    let mut archive = open(zip_path)?;
    let Ok(mut entry) = archive.by_name(name) else {
        return Ok(None);
    };
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    Ok(Some(text))
}

/// Every entry name in the archive.
pub fn entry_names(zip_path: &Path) -> AppResult<Vec<String>> {
    let archive = open(zip_path)?;
    Ok(archive.file_names().map(str::to_string).collect())
}

/// Extracts the entries under each of `prefixes` (e.g. `overrides/`) into
/// `dest/<prefix without slash>/…`, returning the directories that now exist
/// in the same order. Reuses a previous complete extraction.
pub fn extract_prefixes(zip_path: &Path, dest: &Path, prefixes: &[&str]) -> AppResult<Vec<PathBuf>> {
    let outputs: Vec<PathBuf> = prefixes.iter().map(|p| dest.join(p.trim_end_matches('/'))).collect();
    let done_marker = dest.join(".complete");
    if !done_marker.exists() {
        let tmp = dest.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        let result = extract_into(zip_path, &tmp, prefixes);
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
        std::fs::write(tmp.join(".complete"), b"ok")?;
        if dest.exists() {
            std::fs::remove_dir_all(dest)?;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&tmp, dest)?;
    }
    Ok(outputs.into_iter().filter(|p| p.is_dir()).collect())
}

fn extract_into(zip_path: &Path, tmp: &Path, prefixes: &[&str]) -> AppResult<()> {
    let mut archive = open(zip_path)?;
    std::fs::create_dir_all(tmp)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| AppError::Provider(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            tracing::warn!("skipping unsafe archive entry {:?}", entry.name());
            continue;
        };
        let name_str = name.to_string_lossy().replace('\\', "/");
        let Some(prefix) = prefixes.iter().find(|p| name_str.starts_with(*p)) else {
            continue;
        };
        let out = tmp.join(prefix.trim_end_matches('/')).join(&name_str[prefix.len()..]);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn extracts_only_the_requested_prefixes_and_refuses_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("pack.zip");
        make_zip(
            &zip_path,
            &[
                ("manifest.json", b"{}"),
                ("overrides/config/a.toml", b"a"),
                ("client-overrides/options.txt", b"o"),
                ("overrides/../../evil.txt", b"x"),
            ],
        );
        let dest = dir.path().join("out");

        let dirs = extract_prefixes(&zip_path, &dest, &["overrides/", "client-overrides/"]).unwrap();

        assert_eq!(dirs, vec![dest.join("overrides"), dest.join("client-overrides")]);
        assert_eq!(std::fs::read(dest.join("overrides/config/a.toml")).unwrap(), b"a");
        assert!(!dir.path().join("evil.txt").exists());
        assert!(!dest.join("manifest.json").exists());
        assert_eq!(read_text(&zip_path, "manifest.json").unwrap().as_deref(), Some("{}"));
        assert_eq!(read_text(&zip_path, "missing.json").unwrap(), None);
    }
}
