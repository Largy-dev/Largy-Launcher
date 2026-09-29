//! Restore points: a snapshot of what updates change — the content folders
//! (`mods/`, `config/`, packs, scripts) plus the instance's version, loader
//! and modpack — taken automatically before mod or modpack updates and on
//! demand, so a broken update is one click away from being undone.
//!
//! Jars and pack zips are hard-linked (free: the launcher only ever replaces
//! those files, never rewrites them in place — see
//! [`crate::util::fs::copy_dir_recursive`]); configs and scripts, which the
//! game edits in place, are real copies. Worlds aren't included: they have
//! their own backups ([`super::worlds`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Instance, ModpackRef};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::providers::LoaderKind;
use crate::util::fs::{validate_file_name, write_atomic};

/// Folders a snapshot holds; the first ones are hard-linked.
const LINKED: &[&str] = &["mods", "resourcepacks", "shaderpacks"];
const COPIED: &[&str] = &["config", "defaultconfigs", "kubejs", "scripts"];
const KEEP: usize = 5;
const INFO: &str = "snapshot.json";

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Snapshot {
    /// Folder name — the snapshot's id.
    pub id: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Why it was taken, shown to the player ("Avant la mise à jour de 12 mods").
    pub reason: String,
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    /// Modpack version name/id at the time, if any.
    pub modpack_version: Option<String>,
    #[serde(default)]
    #[ts(skip)]
    pub modpack: Option<ModpackRef>,
    /// Files in the snapshot (for the UI: "183 mods").
    pub mods: u32,
}

fn root(paths: &AppPaths, instance_id: &str) -> PathBuf {
    paths.root().join("snapshots").join(instance_id)
}

fn link_or_copy(src: &Path, dest: &Path) -> std::io::Result<()> {
    if std::fs::hard_link(src, dest).is_err() {
        std::fs::copy(src, dest)?;
    }
    Ok(())
}

/// Mirrors `src` into `dest` (created), hard-linking or copying files.
fn mirror(src: &Path, dest: &Path, link: bool) -> std::io::Result<u32> {
    let mut files = 0;
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            files += mirror(&entry.path(), &target, link)?;
        } else if link {
            link_or_copy(&entry.path(), &target)?;
            files += 1;
        } else {
            std::fs::copy(entry.path(), &target)?;
            files += 1;
        }
    }
    Ok(files)
}

/// Takes a restore point of `instance`, dropping the oldest ones past [`KEEP`].
pub fn create(paths: &AppPaths, instance: &Instance, reason: &str) -> AppResult<Snapshot> {
    let snapshot = take(paths, instance, reason)?;
    prune(&root(paths, &instance.id));
    Ok(snapshot)
}

fn take(paths: &AppPaths, instance: &Instance, reason: &str) -> AppResult<Snapshot> {
    let base = root(paths, &instance.id);
    std::fs::create_dir_all(&base)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let mut id = stamp.clone();
    let mut n = 2;
    while base.join(&id).exists() {
        id = format!("{stamp}-{n}");
        n += 1;
    }
    let tmp = base.join(format!(".{id}.part"));
    let result = (|| -> AppResult<u32> {
        let mut mods = 0;
        for (folders, link) in [(LINKED, true), (COPIED, false)] {
            for folder in folders {
                let src = instance.directory.join(folder);
                if src.is_dir() {
                    let count = mirror(&src, &tmp.join(folder), link)?;
                    if *folder == "mods" {
                        mods = count;
                    }
                }
            }
        }
        Ok(mods)
    })();
    let mods = match result {
        Ok(mods) => mods,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let snapshot = Snapshot {
        id: id.clone(),
        created_at: crate::auth::now_unix(),
        reason: reason.to_string(),
        minecraft_version: instance.minecraft_version.clone(),
        loader: instance.loader,
        loader_version: instance.loader_version.clone(),
        modpack_version: instance.modpack.as_ref().map(|m| m.version_id.clone()),
        modpack: instance.modpack.clone(),
        mods,
    };
    write_atomic(&tmp.join(INFO), &serde_json::to_vec_pretty(&snapshot)?)?;
    std::fs::rename(&tmp, base.join(&id))?;
    Ok(snapshot)
}

fn read(dir: &Path) -> Option<Snapshot> {
    serde_json::from_slice(&std::fs::read(dir.join(INFO)).ok()?).ok()
}

/// Newest first.
pub fn list(paths: &AppPaths, instance_id: &str) -> Vec<Snapshot> {
    let Ok(entries) = std::fs::read_dir(root(paths, instance_id)) else {
        return Vec::new();
    };
    let mut snapshots: Vec<Snapshot> = entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| read(&e.path()))
        .collect();
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
    snapshots
}

fn prune(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else { return };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    dirs.sort();
    let excess = dirs.len().saturating_sub(KEEP);
    for old in dirs.into_iter().take(excess) {
        let _ = std::fs::remove_dir_all(old);
    }
}

fn snapshot_dir(paths: &AppPaths, instance_id: &str, id: &str) -> AppResult<PathBuf> {
    validate_file_name(id)?;
    let dir = root(paths, instance_id).join(id);
    if !dir.join(INFO).is_file() {
        return Err(AppError::Instance(format!("point de restauration introuvable : {id}")));
    }
    Ok(dir)
}

pub fn delete(paths: &AppPaths, instance_id: &str, id: &str) -> AppResult<()> {
    std::fs::remove_dir_all(snapshot_dir(paths, instance_id, id)?)?;
    Ok(())
}

/// Puts the instance back as it was in snapshot `id`. The current state is
/// saved as a restore point first, so a restore can itself be undone.
/// Returns the instance with its version, loader and modpack restored.
pub fn restore(paths: &AppPaths, instance: &Instance, id: &str) -> AppResult<Instance> {
    let dir = snapshot_dir(paths, &instance.id, id)?;
    let snapshot = read(&dir).ok_or_else(|| AppError::Instance("point de restauration illisible".to_string()))?;
    // Pruned only once done: the point being restored may be the oldest.
    take(paths, instance, "Avant une restauration")?;

    for (folders, link) in [(LINKED, true), (COPIED, false)] {
        for folder in folders {
            let target = instance.directory.join(folder);
            if target.exists() {
                std::fs::remove_dir_all(&target)?;
            }
            let saved = dir.join(folder);
            if saved.is_dir() {
                mirror(&saved, &target, link)?;
            } else if LINKED.contains(folder) {
                std::fs::create_dir_all(&target)?;
            }
        }
    }
    let restored = super::update(paths, &instance.id, |i| {
        i.minecraft_version = snapshot.minecraft_version.clone();
        i.loader = snapshot.loader;
        i.loader_version = snapshot.loader_version.clone();
        i.modpack = snapshot.modpack.clone();
        Ok(())
    })?;
    prune(&root(paths, &instance.id));
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::{create as create_instance, CreateInstanceInput};

    fn setup() -> (tempfile::TempDir, AppPaths, Instance) {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let instance = create_instance(
            &paths,
            CreateInstanceInput {
                name: "Pack".into(),
                minecraft_version: "1.20.1".into(),
                loader: LoaderKind::Forge,
                loader_version: Some("47.1".into()),
                modpack: None,
                icon_url: None,
            },
        )
        .unwrap();
        std::fs::write(instance.directory.join("mods/a-1.0.jar"), b"old mod").unwrap();
        std::fs::create_dir_all(instance.directory.join("config/sub")).unwrap();
        std::fs::write(instance.directory.join("config/sub/a.toml"), b"old=1").unwrap();
        (dir, paths, instance)
    }

    #[test]
    fn restoring_undoes_an_update_and_can_itself_be_undone() {
        let (_dir, paths, instance) = setup();
        let snap = create(&paths, &instance, "Avant la mise à jour").unwrap();
        assert_eq!(snap.mods, 1);

        // The "update": a new jar replaces the old one, a config is edited in
        // place (as the game does), the loader moves on.
        std::fs::remove_file(instance.directory.join("mods/a-1.0.jar")).unwrap();
        std::fs::write(instance.directory.join("mods/a-2.0.jar"), b"new mod").unwrap();
        std::fs::write(instance.directory.join("config/sub/a.toml"), b"old=2").unwrap();
        let updated = crate::instances::update(&paths, &instance.id, |i| {
            i.loader_version = Some("47.3".into());
            Ok(())
        })
        .unwrap();

        let restored = restore(&paths, &updated, &snap.id).unwrap();

        assert_eq!(restored.loader_version.as_deref(), Some("47.1"));
        assert_eq!(std::fs::read(instance.directory.join("mods/a-1.0.jar")).unwrap(), b"old mod");
        assert!(!instance.directory.join("mods/a-2.0.jar").exists());
        assert_eq!(std::fs::read(instance.directory.join("config/sub/a.toml")).unwrap(), b"old=1");

        let all = list(&paths, &instance.id);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].reason, "Avant une restauration");
        assert_eq!(all[0].loader_version.as_deref(), Some("47.3"));
    }

    #[test]
    fn only_the_newest_restore_points_are_kept() {
        let (_dir, paths, instance) = setup();
        for _ in 0..KEEP + 2 {
            create(&paths, &instance, "x").unwrap();
        }
        assert_eq!(list(&paths, &instance.id).len(), KEEP);
    }

    #[test]
    fn the_oldest_restore_point_can_be_restored_when_the_list_is_full() {
        let (_dir, paths, instance) = setup();
        let oldest = create(&paths, &instance, "first").unwrap();
        for _ in 1..KEEP {
            create(&paths, &instance, "x").unwrap();
        }
        restore(&paths, &instance, &oldest.id).unwrap();
        assert_eq!(list(&paths, &instance.id).len(), KEEP);
    }

    #[test]
    fn deleting_and_bad_ids() {
        let (_dir, paths, instance) = setup();
        let snap = create(&paths, &instance, "x").unwrap();
        assert!(delete(&paths, &instance.id, "../x").is_err());
        assert!(restore(&paths, &instance, "missing").is_err());
        delete(&paths, &instance.id, &snap.id).unwrap();
        assert!(list(&paths, &instance.id).is_empty());
    }
}
