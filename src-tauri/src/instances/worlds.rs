//! An instance's singleplayer worlds (`saves/`): listed with what their
//! `level.dat` says (name, mode, last played, version), backed up one by one
//! next to the whole-`saves/` snapshots of [`super::backup`], restored from
//! any backup without ever overwriting an existing world, imported from a
//! zip or folder, and deleted.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::backup::zip_dir;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::util::fs::{copy_tree, dir_size, safe_join, validate_file_name};

const SAVES: &str = "saves";
/// Per-world backups kept (the oldest go first).
const KEEP_PER_WORLD: usize = 5;
/// Largest `level.dat` read (they're a few KB; modded ones a few hundred).
const MAX_LEVEL_DAT: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct World {
    /// Folder name in `saves/` (the world's id for every other call).
    pub folder: String,
    /// In-game name, falling back to the folder name.
    pub name: String,
    /// `survival`, `creative`, `adventure` or `spectator`.
    pub game_mode: Option<String>,
    pub hardcore: bool,
    pub cheats: bool,
    /// Unix seconds.
    pub last_played: Option<i64>,
    pub version: Option<String>,
    pub size: u64,
    /// Absolute path of the world's `icon.png` (asset protocol).
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct WorldBackup {
    /// Path relative to the instance's backup folder — its id.
    pub id: String,
    /// The world it holds, or `None` for a snapshot of every world.
    pub world: Option<String>,
    /// Unix seconds.
    pub created_at: i64,
    pub size: u64,
}

#[derive(Deserialize)]
struct LevelDat {
    #[serde(rename = "Data")]
    data: LevelData,
}

#[derive(Deserialize)]
struct LevelData {
    #[serde(rename = "LevelName")]
    level_name: Option<String>,
    #[serde(rename = "GameType")]
    game_type: Option<i32>,
    hardcore: Option<i8>,
    #[serde(rename = "allowCommands")]
    allow_commands: Option<i8>,
    #[serde(rename = "LastPlayed")]
    last_played: Option<i64>,
    #[serde(rename = "Version")]
    version: Option<VersionTag>,
}

#[derive(Deserialize)]
struct VersionTag {
    #[serde(rename = "Name")]
    name: Option<String>,
}

fn read_level_dat(path: &Path) -> Option<LevelData> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(file).take(MAX_LEVEL_DAT).read_to_end(&mut bytes).ok()?;
    fastnbt::from_bytes::<LevelDat>(&bytes).ok().map(|l| l.data)
}

fn game_mode(id: i32) -> Option<String> {
    let mode = match id {
        0 => "survival",
        1 => "creative",
        2 => "adventure",
        3 => "spectator",
        _ => return None,
    };
    Some(mode.to_string())
}

fn read_world(dir: &Path) -> Option<World> {
    let level_dat = dir.join("level.dat");
    if !level_dat.is_file() {
        return None;
    }
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let data = read_level_dat(&level_dat);
    let icon = dir.join("icon.png");
    let modified = || {
        std::fs::metadata(&level_dat)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    };
    Some(World {
        name: data
            .as_ref()
            .and_then(|d| d.level_name.clone())
            .map(|n| super::installed::metadata::strip_formatting(&n))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| folder.clone()),
        game_mode: data.as_ref().and_then(|d| d.game_type).and_then(game_mode),
        hardcore: data.as_ref().and_then(|d| d.hardcore).is_some_and(|h| h != 0),
        cheats: data.as_ref().and_then(|d| d.allow_commands).is_some_and(|c| c != 0),
        last_played: data.as_ref().and_then(|d| d.last_played).map(|ms| ms / 1000).or_else(modified),
        version: data.as_ref().and_then(|d| d.version.as_ref()).and_then(|v| v.name.clone()),
        size: dir_size(dir),
        icon_path: icon.is_file().then(|| icon.display().to_string()),
        folder,
    })
}

/// Every world, most recently played first.
pub fn list(instance_dir: &Path) -> AppResult<Vec<World>> {
    let Ok(entries) = std::fs::read_dir(instance_dir.join(SAVES)) else {
        return Ok(Vec::new());
    };
    let mut worlds: Vec<World> =
        entries.flatten().filter(|e| e.path().is_dir()).filter_map(|e| read_world(&e.path())).collect();
    worlds.sort_by_key(|w| std::cmp::Reverse(w.last_played.unwrap_or(0)));
    Ok(worlds)
}

pub fn world_dir(instance_dir: &Path, folder: &str) -> AppResult<PathBuf> {
    validate_file_name(folder)?;
    let dir = instance_dir.join(SAVES).join(folder);
    if !dir.join("level.dat").is_file() {
        return Err(AppError::Instance(format!("monde introuvable : {folder}")));
    }
    Ok(dir)
}

fn world_backups_dir(paths: &AppPaths, instance_id: &str, folder: &str) -> PathBuf {
    paths.backups_dir(instance_id).join("worlds").join(folder)
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// Zips one world into its own backup folder; returns the backup.
pub fn backup(paths: &AppPaths, instance_id: &str, instance_dir: &Path, folder: &str) -> AppResult<WorldBackup> {
    let source = world_dir(instance_dir, folder)?;
    let dir = world_backups_dir(paths, instance_id, folder);
    std::fs::create_dir_all(&dir)?;
    let mut name = format!("{}.zip", timestamp());
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{}-{n}.zip", timestamp());
        n += 1;
    }
    let dest = dir.join(&name);
    let tmp = dir.join(format!("{name}.part"));
    let result = (|| -> AppResult<()> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
        zip_dir(&mut zip, &source, &format!("{SAVES}/{folder}"))?;
        zip.finish()?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &dest)?;
    prune(&dir, KEEP_PER_WORLD);
    let root = paths.backups_dir(instance_id);
    describe(&root, &dest).ok_or_else(|| AppError::Instance("sauvegarde introuvable après écriture".to_string()))
}

fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut zips: Vec<PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "zip")).collect();
    zips.sort();
    let excess = zips.len().saturating_sub(keep);
    for old in zips.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}

fn describe(root: &Path, path: &Path) -> Option<WorldBackup> {
    let meta = std::fs::metadata(path).ok()?;
    let rel = path.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    let world = rel.strip_prefix("worlds/").and_then(|r| r.split('/').next()).map(str::to_string);
    let created_at = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(WorldBackup { id: rel, world, created_at, size: meta.len() })
}

/// Snapshots of every world and backups of single worlds, newest first.
pub fn list_backups(paths: &AppPaths, instance_id: &str) -> Vec<WorldBackup> {
    let root = paths.backups_dir(instance_id);
    let mut files: Vec<PathBuf> = Vec::new();
    let zips_in = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "zip")).collect()
            })
            .unwrap_or_default()
    };
    files.extend(zips_in(&root));
    if let Ok(worlds) = std::fs::read_dir(root.join("worlds")) {
        for world in worlds.flatten().filter(|e| e.path().is_dir()) {
            files.extend(zips_in(&world.path()));
        }
    }
    let mut backups: Vec<WorldBackup> = files.iter().filter_map(|p| describe(&root, p)).collect();
    backups.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    backups
}

fn backup_path(paths: &AppPaths, instance_id: &str, id: &str) -> AppResult<PathBuf> {
    let path = safe_join(&paths.backups_dir(instance_id), id)
        .filter(|p| p.extension().is_some_and(|e| e == "zip") && p.is_file())
        .ok_or_else(|| AppError::Instance(format!("sauvegarde introuvable : {id}")))?;
    Ok(path)
}

pub fn delete_backup(paths: &AppPaths, instance_id: &str, id: &str) -> AppResult<()> {
    std::fs::remove_file(backup_path(paths, instance_id, id)?)?;
    Ok(())
}

/// `base`, or `base (2)`, `base (3)`… — the first name free in `saves/`.
fn free_name(saves: &Path, base: &str) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "Monde" } else { base };
    if !saves.join(base).exists() {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} ({n})")).find(|name| !saves.join(name).exists()).unwrap_or_default()
}

/// Folder name safe on every OS, from a world or file name.
fn sanitize(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect();
    cleaned.trim().trim_end_matches('.').chars().take(64).collect()
}

/// Every world found in a zip: the folder holding each `level.dat`, as an
/// archive path prefix (`""` for a world zipped at the root).
fn worlds_in_zip<R: Read + std::io::Seek>(zip: &zip::ZipArchive<R>) -> Vec<String> {
    let mut roots: Vec<String> = zip
        .file_names()
        .map(|n| n.replace('\\', "/"))
        .filter_map(|n| {
            let dir = n.strip_suffix("level.dat")?;
            (dir.is_empty() || dir.ends_with('/')).then(|| dir.to_string())
        })
        .collect();
    roots.sort();
    // A world's own sub-folders can hold a `level.dat` too (old DIM folders):
    // keep only the outermost ones.
    roots.iter().filter(|r| !roots.iter().any(|o| o != *r && r.starts_with(o.as_str()))).cloned().collect()
}

fn extract_world<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, prefix: &str, dest: &Path) -> AppResult<()> {
    let tmp = dest.with_file_name(format!(".import-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| -> AppResult<()> {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let Some(name) = entry.enclosed_name() else { continue };
            let name = name.to_string_lossy().replace('\\', "/");
            let Some(rel) = name.strip_prefix(prefix) else { continue };
            let Some(out) = safe_join(&tmp, rel) else { continue };
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// Puts every world of the backup back into `saves/` — under a new name when
/// a world with that folder already exists. Returns the folders created.
pub fn restore_backup(paths: &AppPaths, instance_id: &str, instance_dir: &Path, id: &str) -> AppResult<Vec<String>> {
    let path = backup_path(paths, instance_id, id)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path)?)?;
    let saves = instance_dir.join(SAVES);
    std::fs::create_dir_all(&saves)?;
    let mut restored = Vec::new();
    for prefix in worlds_in_zip(&zip) {
        let original = prefix.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
        let folder = free_name(&saves, &sanitize(original));
        extract_world(&mut zip, &prefix, &saves.join(&folder))?;
        restored.push(folder);
    }
    if restored.is_empty() {
        return Err(AppError::Instance("aucun monde dans cette sauvegarde".to_string()));
    }
    Ok(restored)
}

/// Adds worlds from a `.zip` or a world folder; returns the folders created.
pub fn import(instance_dir: &Path, source: &Path) -> AppResult<Vec<String>> {
    let saves = instance_dir.join(SAVES);
    std::fs::create_dir_all(&saves)?;
    if source.is_dir() {
        if !source.join("level.dat").is_file() {
            return Err(AppError::Instance("ce dossier n'est pas un monde Minecraft (level.dat absent)".to_string()));
        }
        let base = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let folder = free_name(&saves, &sanitize(&base));
        copy_tree(source, &saves.join(&folder))?;
        return Ok(vec![folder]);
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(source)?)
        .map_err(|_| AppError::Instance("ce fichier n'est pas une archive zip".to_string()))?;
    let prefixes = worlds_in_zip(&zip);
    if prefixes.is_empty() {
        return Err(AppError::Instance("aucun monde Minecraft dans cette archive (level.dat absent)".to_string()));
    }
    let stem = source.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut imported = Vec::new();
    for prefix in prefixes {
        let inner = prefix.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
        let base = if inner.is_empty() { stem.as_str() } else { inner };
        let folder = free_name(&saves, &sanitize(base));
        extract_world(&mut zip, &prefix, &saves.join(&folder))?;
        imported.push(folder);
    }
    Ok(imported)
}

pub fn delete(instance_dir: &Path, folder: &str) -> AppResult<()> {
    std::fs::remove_dir_all(world_dir(instance_dir, folder)?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
