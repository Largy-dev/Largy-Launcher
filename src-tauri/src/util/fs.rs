//! Filesystem helpers that every persisted file and every externally-supplied
//! path goes through: crash-safe writes, and path joins that refuse to escape
//! their base directory (zip-slip, `..` in modpack manifests, ids coming from
//! the webview).

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Writes `bytes` to a sibling temp file, fsyncs it, then renames it over
/// `path` — a crash or power loss mid-write leaves the previous file intact
/// instead of a truncated one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4().simple()));
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Joins an untrusted relative path onto `base`, rejecting anything that
/// could land outside it: absolute paths, drive prefixes, and `..`.
pub fn safe_join(base: &Path, relative: impl AsRef<Path>) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    let mut pushed = false;
    for component in relative.as_ref().components() {
        match component {
            Component::Normal(part) => {
                out.push(part);
                pushed = true;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    pushed.then_some(out)
}

/// A single path segment with no separators or `..` — what a mod file name
/// or an instance id must look like when it comes from the frontend.
pub fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
        && name.trim() == name
}

pub fn validate_file_name(name: &str) -> AppResult<()> {
    if is_plain_file_name(name) {
        Ok(())
    } else {
        Err(AppError::Instance(format!("nom de fichier invalide: {name:?}")))
    }
}

/// Copies `src`'s tree into `dest` (merging), returning every copied file's
/// path relative to `dest`. `skip_existing` leaves files already present in
/// `dest` untouched.
pub fn copy_dir_recursive(src: &Path, dest: &Path, skip_existing: impl Fn(&Path) -> bool) -> AppResult<Vec<PathBuf>> {
    let mut copied = Vec::new();
    if src.exists() {
        copy_into(src, dest, dest, &skip_existing, &mut copied)?;
    }
    Ok(copied)
}

fn copy_into(
    src: &Path,
    dest: &Path,
    root: &Path,
    skip_existing: &impl Fn(&Path) -> bool,
    copied: &mut Vec<PathBuf>,
) -> AppResult<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_into(&entry.path(), &target, root, skip_existing, copied)?;
            continue;
        }
        let rel = target.strip_prefix(root).map(Path::to_path_buf).unwrap_or_else(|_| target.clone());
        if !(target.exists() && skip_existing(&rel)) {
            std::fs::copy(entry.path(), &target)?;
        }
        copied.push(rel);
    }
    Ok(())
}

/// Recursive copy of a whole directory (used to duplicate instances).
pub fn copy_tree(src: &Path, dest: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dest)?;
    copy_dir_recursive(src, dest, |_| false)?;
    Ok(())
}

/// Total size in bytes of every file under `path` — 0 if it doesn't exist.
/// Used to show how much a cache folder is worth clearing before doing it.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(ft) if ft.is_dir() => dir_size(&entry.path()),
            _ => entry.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_nested_relative_paths() {
        let joined = safe_join(Path::new("/base"), "mods/./a.jar").unwrap();
        assert_eq!(joined, Path::new("/base").join("mods").join("a.jar"));
    }

    #[test]
    fn safe_join_rejects_escapes_and_empty() {
        assert!(safe_join(Path::new("/base"), "../evil").is_none());
        assert!(safe_join(Path::new("/base"), "mods/../../evil").is_none());
        assert!(safe_join(Path::new("/base"), "/etc/passwd").is_none());
        assert!(safe_join(Path::new("/base"), "").is_none());
        assert!(safe_join(Path::new("/base"), ".").is_none());
        #[cfg(windows)]
        assert!(safe_join(Path::new("C:\\base"), "C:\\Windows").is_none());
    }

    #[test]
    fn plain_file_names_reject_separators_and_dots() {
        assert!(is_plain_file_name("sodium-0.5.jar"));
        assert!(!is_plain_file_name(""));
        assert!(!is_plain_file_name(".."));
        assert!(!is_plain_file_name("../x.jar"));
        assert!(!is_plain_file_name("a\\b.jar"));
        assert!(!is_plain_file_name("C:x.jar"));
    }

    #[test]
    fn write_atomic_replaces_content_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn copy_dir_recursive_can_preserve_existing_files() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("options.txt"), b"pack").unwrap();
        std::fs::create_dir_all(src.path().join("config")).unwrap();
        std::fs::write(src.path().join("config/a.toml"), b"pack").unwrap();
        std::fs::write(dest.path().join("options.txt"), b"user").unwrap();

        let mut copied = copy_dir_recursive(src.path(), dest.path(), |rel| rel == Path::new("options.txt")).unwrap();
        copied.sort();

        assert_eq!(copied, vec![PathBuf::from("config/a.toml"), PathBuf::from("options.txt")]);
        assert_eq!(std::fs::read(dest.path().join("options.txt")).unwrap(), b"user");
        assert_eq!(std::fs::read(dest.path().join("config/a.toml")).unwrap(), b"pack");
    }

    #[test]
    fn dir_size_sums_nested_files_and_ignores_missing_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jar"), b"1234").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/b.jar"), b"123").unwrap();

        assert_eq!(dir_size(dir.path()), 7);
        assert_eq!(dir_size(&dir.path().join("does-not-exist")), 0);
    }
}
