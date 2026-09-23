//! Zip snapshots of an instance's worlds, taken automatically before a
//! modpack update (which can rewrite configs a world depends on) and on
//! demand. Only the newest [`KEEP`] snapshots per instance are kept.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::AppResult;
use crate::paths::AppPaths;

const KEEP: usize = 5;

/// Adds every file under `dir` to `zip`, stored under `prefix/`.
pub fn zip_dir<W: Write + std::io::Seek>(zip: &mut zip::ZipWriter<W>, dir: &Path, prefix: &str) -> AppResult<()> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            // `session.lock` is held open by a running game; it's meaningless in a backup.
            if rel.ends_with("session.lock") {
                continue;
            }
            zip.start_file(format!("{prefix}/{rel}"), options)?;
            std::io::copy(&mut std::fs::File::open(&path)?, zip)?;
        }
    }
    Ok(())
}

fn has_worlds(saves: &Path) -> bool {
    std::fs::read_dir(saves).is_ok_and(|mut entries| entries.next().is_some())
}

/// Zips `saves/` into the instance's backup folder; `None` when there's
/// nothing to back up.
pub fn backup_saves(paths: &AppPaths, instance_id: &str, instance_dir: &Path) -> AppResult<Option<PathBuf>> {
    let saves = instance_dir.join("saves");
    if !has_worlds(&saves) {
        return Ok(None);
    }
    let dir = paths.backups_dir(instance_id);
    std::fs::create_dir_all(&dir)?;
    let name = format!("mondes-{}.zip", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let dest = dir.join(&name);
    let tmp = dir.join(format!("{name}.part"));

    let result = (|| -> AppResult<()> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
        zip_dir(&mut zip, &saves, "saves")?;
        zip.finish()?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &dest)?;
    prune(&dir);
    Ok(Some(dest))
}

fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "zip"))
        .collect();
    // Names embed a sortable timestamp.
    backups.sort();
    let excess = backups.len().saturating_sub(KEEP);
    for old in backups.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_zips_worlds_and_keeps_only_the_newest_ones() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(root.path().to_path_buf());
        let instance_dir = root.path().join("inst");
        std::fs::create_dir_all(instance_dir.join("saves/World")).unwrap();
        std::fs::write(instance_dir.join("saves/World/level.dat"), b"lvl").unwrap();
        std::fs::write(instance_dir.join("saves/World/session.lock"), b"x").unwrap();

        let backup_dir = paths.backups_dir("id");
        std::fs::create_dir_all(&backup_dir).unwrap();
        for i in 0..6 {
            std::fs::write(backup_dir.join(format!("mondes-2000-01-0{i}.zip")), b"old").unwrap();
        }

        let dest = backup_saves(&paths, "id", &instance_dir).unwrap().unwrap();

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        assert!(archive.by_name("saves/World/level.dat").is_ok());
        assert!(archive.by_name("saves/World/session.lock").is_err());
        assert_eq!(std::fs::read_dir(&backup_dir).unwrap().count(), KEEP);
        assert!(dest.exists());
    }

    #[test]
    fn nothing_to_back_up_without_worlds() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(root.path().to_path_buf());
        assert!(backup_saves(&paths, "id", root.path()).unwrap().is_none());
    }
}
