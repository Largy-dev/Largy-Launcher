//! Local mod management for an instance's `mods/` folder: list, enable/
//! disable (rename with a `.disabled` suffix — the same convention every
//! popular launcher uses, since it needs no extra bookkeeping file and a
//! disabled mod is still trivially visible on disk), delete, and add a jar
//! picked from the file system. No marketplace search here — see the
//! `instances_mods_*` command docs for why v1 stays local-only.

use std::path::Path;

use serde::Serialize;

use crate::error::{AppError, AppResult};

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Serialize)]
pub struct ModEntry {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
}

fn mods_dir(instance_dir: &Path) -> std::path::PathBuf {
    instance_dir.join("mods")
}

pub fn list(instance_dir: &Path) -> AppResult<Vec<ModEntry>> {
    let dir = mods_dir(instance_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let (file_name, enabled) = match name.strip_suffix(DISABLED_SUFFIX) {
            Some(base) => (base.to_string(), false),
            None => (name.clone(), true),
        };
        if !file_name.ends_with(".jar") {
            continue;
        }
        let size = entry.metadata()?.len();
        entries.push(ModEntry { file_name, enabled, size });
    }
    entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(entries)
}

pub fn set_enabled(instance_dir: &Path, file_name: &str, enabled: bool) -> AppResult<()> {
    let dir = mods_dir(instance_dir);
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));

    let (from, to) = if enabled { (disabled_path, enabled_path) } else { (enabled_path, disabled_path) };
    if !from.exists() {
        return Err(AppError::Instance(format!("mod introuvable: {file_name}")));
    }
    std::fs::rename(from, to)?;
    Ok(())
}

pub fn delete(instance_dir: &Path, file_name: &str) -> AppResult<()> {
    let dir = mods_dir(instance_dir);
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));

    if enabled_path.exists() {
        std::fs::remove_file(enabled_path)?;
    } else if disabled_path.exists() {
        std::fs::remove_file(disabled_path)?;
    } else {
        return Err(AppError::Instance(format!("mod introuvable: {file_name}")));
    }
    Ok(())
}

pub fn add_from_path(instance_dir: &Path, source: &Path) -> AppResult<()> {
    if source.extension().and_then(|e| e.to_str()) != Some("jar") {
        return Err(AppError::Instance("seuls les fichiers .jar peuvent être ajoutés comme mod".to_string()));
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| AppError::Instance("chemin de fichier invalide".to_string()))?;

    let dir = mods_dir(instance_dir);
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(source, dir.join(file_name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("mods")).unwrap();
        dir
    }

    #[test]
    fn list_is_empty_when_mods_dir_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn list_reports_enabled_and_disabled_mods_ignoring_non_jars() {
        let dir = setup();
        std::fs::write(dir.path().join("mods/a.jar"), b"aaaa").unwrap();
        std::fs::write(dir.path().join("mods/b.jar.disabled"), b"bb").unwrap();
        std::fs::write(dir.path().join("mods/readme.txt"), b"not a mod").unwrap();

        let entries = list(dir.path()).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file_name, "a.jar");
        assert!(entries[0].enabled);
        assert_eq!(entries[0].size, 4);
        assert_eq!(entries[1].file_name, "b.jar");
        assert!(!entries[1].enabled);
    }

    #[test]
    fn set_enabled_false_renames_to_disabled_suffix() {
        let dir = setup();
        std::fs::write(dir.path().join("mods/a.jar"), b"x").unwrap();

        set_enabled(dir.path(), "a.jar", false).unwrap();

        assert!(!dir.path().join("mods/a.jar").exists());
        assert!(dir.path().join("mods/a.jar.disabled").exists());
    }

    #[test]
    fn set_enabled_true_strips_disabled_suffix() {
        let dir = setup();
        std::fs::write(dir.path().join("mods/a.jar.disabled"), b"x").unwrap();

        set_enabled(dir.path(), "a.jar", true).unwrap();

        assert!(dir.path().join("mods/a.jar").exists());
        assert!(!dir.path().join("mods/a.jar.disabled").exists());
    }

    #[test]
    fn set_enabled_errors_when_the_mod_does_not_exist() {
        let dir = setup();
        assert!(set_enabled(dir.path(), "missing.jar", false).is_err());
    }

    #[test]
    fn delete_removes_an_enabled_or_disabled_mod() {
        let dir = setup();
        std::fs::write(dir.path().join("mods/a.jar"), b"x").unwrap();
        std::fs::write(dir.path().join("mods/b.jar.disabled"), b"x").unwrap();

        delete(dir.path(), "a.jar").unwrap();
        delete(dir.path(), "b.jar").unwrap();

        assert!(!dir.path().join("mods/a.jar").exists());
        assert!(!dir.path().join("mods/b.jar.disabled").exists());
    }

    #[test]
    fn delete_errors_when_the_mod_does_not_exist() {
        let dir = setup();
        assert!(delete(dir.path(), "missing.jar").is_err());
    }

    #[test]
    fn add_from_path_copies_a_jar_into_mods() {
        let dir = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("cool-mod.jar");
        std::fs::write(&source, b"jar bytes").unwrap();

        add_from_path(dir.path(), &source).unwrap();

        assert_eq!(std::fs::read(dir.path().join("mods/cool-mod.jar")).unwrap(), b"jar bytes");
    }

    #[test]
    fn add_from_path_rejects_non_jar_files() {
        let dir = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("not-a-mod.zip");
        std::fs::write(&source, b"x").unwrap();

        assert!(add_from_path(dir.path(), &source).is_err());
    }
}
