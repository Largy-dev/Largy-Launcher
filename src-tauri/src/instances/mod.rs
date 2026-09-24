//! Per-instance data model and CRUD. Each instance is an isolated folder
//! (`mods/`, `saves/`, `config/`, `resourcepacks/`, `natives/`) with its own
//! `instance.json` — no shared state between instances besides the
//! libraries/assets/java caches in [`crate::paths::AppPaths`].

pub mod backup;
pub mod content;
pub mod export;
pub mod import;
pub mod mods;
pub mod screenshots;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::providers::LoaderKind;
use crate::util::fs::{is_plain_file_name, write_atomic};

/// Instance ids come back from the webview; anything that isn't a single
/// plain path segment (`""`, `..`, `a/b`) would point `instance_dir` outside
/// its own folder — e.g. `delete("")` would wipe every instance.
pub fn validate_id(id: &str) -> AppResult<()> {
    if is_plain_file_name(id) && !id.starts_with('.') {
        Ok(())
    } else {
        Err(AppError::Instance(format!("identifiant d'instance invalide: {id:?}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackRef {
    pub provider: String,
    pub pack_id: String,
    pub version_id: String,
    pub pack_name: String,
    /// Paths (relative to the instance directory) of every file this
    /// modpack version placed on disk — downloaded mod files and any
    /// `overrides/` files alike. Used by `instances_update_modpack` to know
    /// which files a newer version no longer ships and can safely remove;
    /// anything not in this list (saves, manual additions) is never touched.
    /// `#[serde(default)]` so instances installed before this field existed
    /// just have an empty list (no retroactive pruning on their first update).
    #[serde(default)]
    pub installed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    pub directory: PathBuf,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub min_memory_mb: Option<u32>,
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    #[serde(default)]
    pub extra_jvm_args: Vec<String>,
    #[serde(default)]
    pub modpack: Option<ModpackRef>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_played_at: Option<i64>,
    /// Cumulative time spent in-game across every session, in seconds.
    #[serde(default)]
    pub play_time_seconds: u64,
    /// Java executable for this instance only; falls back to the global
    /// override, then to the Mojang-managed runtime.
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub window_width: Option<u32>,
    #[serde(default)]
    pub window_height: Option<u32>,
    #[serde(default)]
    pub fullscreen: bool,
    /// `host[:port]` to join straight from the main menu.
    #[serde(default)]
    pub auto_join_server: Option<String>,
    /// Catalog server (see `servers::featured`) this instance was prepared for.
    #[serde(default)]
    pub featured_server: Option<String>,
    /// Kept above the fold in every sort order in the instance list.
    #[serde(default)]
    pub pinned: bool,
    /// Blocks [`delete`] until explicitly turned off — a safety net for
    /// instances the player doesn't want to lose to a stray click.
    #[serde(default)]
    pub protected: bool,
    /// Free-form notes the player writes for themselves.
    #[serde(default)]
    pub notes: String,
    /// Most recent play sessions, newest first, capped to [`MAX_SESSIONS`].
    #[serde(default)]
    pub sessions: Vec<PlaySession>,
}

/// One completed play session, recorded when the game process exits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlaySession {
    pub started_at: i64,
    pub duration_seconds: u64,
}

/// How many recent sessions [`record_session`] keeps — enough for a "last few
/// games" view without the file growing forever over months of play.
pub const MAX_SESSIONS: usize = 20;

/// Longest a note can be — a personal reminder, not a wiki page.
pub const MAX_NOTES_LEN: usize = 4000;

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

pub fn list(paths: &AppPaths) -> AppResult<Vec<Instance>> {
    let dir = paths.instances_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut instances = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let instance_file = entry.path().join("instance.json");
        if !instance_file.exists() {
            continue;
        }
        let bytes = std::fs::read(&instance_file)?;
        match serde_json::from_slice::<Instance>(&bytes) {
            Ok(mut instance) => {
                instance.id = entry.file_name().to_string_lossy().into_owned();
                instance.directory = entry.path();
                instances.push(instance)
            }
            Err(e) => tracing::warn!("skipping unreadable instance {:?}: {e}", entry.path()),
        }
    }
    instances.sort_by_key(|i| std::cmp::Reverse(i.created_at));
    Ok(instances)
}

/// The stored `directory` is ignored in favour of the folder the file was
/// actually found in, so a moved or duplicated instance never points back
/// at another instance's files.
pub fn get(paths: &AppPaths, id: &str) -> AppResult<Instance> {
    validate_id(id)?;
    let dir = paths.instance_dir(id);
    let path = dir.join("instance.json");
    if !path.exists() {
        return Err(AppError::Instance(format!("instance introuvable: {id}")));
    }
    let bytes = std::fs::read(path)?;
    let mut instance: Instance = serde_json::from_slice(&bytes)?;
    instance.id = id.to_string();
    instance.directory = dir;
    Ok(instance)
}

pub fn save(instance: &Instance) -> AppResult<()> {
    write_atomic(
        &instance.directory.join("instance.json"),
        serde_json::to_string_pretty(instance)?.as_bytes(),
    )?;
    Ok(())
}

pub struct CreateInstanceInput {
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    pub modpack: Option<ModpackRef>,
    pub icon_url: Option<String>,
}

pub fn create(paths: &AppPaths, input: CreateInstanceInput) -> AppResult<Instance> {
    let id = uuid::Uuid::new_v4().to_string();
    let directory = paths.instance_dir(&id);
    for sub in ["mods", "saves", "config", "resourcepacks", "shaderpacks", "natives"] {
        std::fs::create_dir_all(directory.join(sub))?;
    }

    let instance = Instance {
        id,
        name: input.name,
        minecraft_version: input.minecraft_version,
        loader: input.loader,
        loader_version: input.loader_version,
        directory,
        icon_url: input.icon_url,
        min_memory_mb: None,
        max_memory_mb: None,
        extra_jvm_args: Vec::new(),
        modpack: input.modpack,
        created_at: now_unix(),
        last_played_at: None,
        play_time_seconds: 0,
        java_path: None,
        window_width: None,
        window_height: None,
        fullscreen: false,
        auto_join_server: None,
        featured_server: None,
        pinned: false,
        protected: false,
        notes: String::new(),
        sessions: Vec::new(),
    };

    save(&instance)?;
    Ok(instance)
}

/// Full copy of an instance (mods, configs, saves) under a fresh id.
pub fn duplicate(paths: &AppPaths, id: &str, name: &str) -> AppResult<Instance> {
    let source = get(paths, id)?;
    let name = validate_name(name)?;
    let new_id = uuid::Uuid::new_v4().to_string();
    let directory = paths.instance_dir(&new_id);
    if let Err(e) = crate::util::fs::copy_tree(&source.directory, &directory) {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(e);
    }
    let instance = Instance {
        id: new_id,
        name,
        directory,
        created_at: now_unix(),
        last_played_at: None,
        play_time_seconds: 0,
        pinned: false,
        protected: false,
        sessions: Vec::new(),
        ..source
    };
    save(&instance)?;
    Ok(instance)
}

pub fn delete(paths: &AppPaths, id: &str) -> AppResult<()> {
    validate_id(id)?;
    if let Ok(instance) = get(paths, id) {
        if instance.protected {
            return Err(AppError::Instance(
                "cette instance est protégée contre la suppression — désactive la protection d'abord".to_string(),
            ));
        }
    }
    let dir = paths.instance_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn touch_last_played(paths: &AppPaths, id: &str) -> AppResult<()> {
    let mut instance = get(paths, id)?;
    instance.last_played_at = Some(now_unix());
    save(&instance)
}

pub fn add_play_time(paths: &AppPaths, id: &str, seconds: u64) -> AppResult<()> {
    let mut instance = get(paths, id)?;
    instance.play_time_seconds = instance.play_time_seconds.saturating_add(seconds);
    save(&instance)
}

/// Appends one finished session, newest first, trimming to [`MAX_SESSIONS`].
/// Sessions that ended in under a second (an instant crash) aren't worth
/// keeping in the history.
pub fn record_session(paths: &AppPaths, id: &str, started_at: i64, duration_seconds: u64) -> AppResult<()> {
    if duration_seconds == 0 {
        return Ok(());
    }
    let mut instance = get(paths, id)?;
    instance.sessions.insert(0, PlaySession { started_at, duration_seconds });
    instance.sessions.truncate(MAX_SESSIONS);
    save(&instance)
}

pub fn set_pinned(paths: &AppPaths, id: &str, pinned: bool) -> AppResult<Instance> {
    let mut instance = get(paths, id)?;
    instance.pinned = pinned;
    save(&instance)?;
    Ok(instance)
}

pub fn set_protected(paths: &AppPaths, id: &str, protected: bool) -> AppResult<Instance> {
    let mut instance = get(paths, id)?;
    instance.protected = protected;
    save(&instance)?;
    Ok(instance)
}

pub fn set_notes(paths: &AppPaths, id: &str, notes: &str) -> AppResult<Instance> {
    let trimmed = notes.trim();
    if trimmed.chars().count() > MAX_NOTES_LEN {
        return Err(AppError::Instance(format!("les notes ne peuvent pas dépasser {MAX_NOTES_LEN} caractères")));
    }
    let mut instance = get(paths, id)?;
    instance.notes = trimmed.to_string();
    save(&instance)?;
    Ok(instance)
}

/// Copies the launch-affecting settings of `source_id` onto `target_id`
/// (memory, JVM args, Java, window size, auto-join server) — not its name,
/// modpack or stats.
pub fn copy_settings(paths: &AppPaths, source_id: &str, target_id: &str) -> AppResult<Instance> {
    let source = get(paths, source_id)?;
    let mut target = get(paths, target_id)?;
    target.min_memory_mb = source.min_memory_mb;
    target.max_memory_mb = source.max_memory_mb;
    target.extra_jvm_args = source.extra_jvm_args;
    target.java_path = source.java_path;
    target.window_width = source.window_width;
    target.window_height = source.window_height;
    target.fullscreen = source.fullscreen;
    target.auto_join_server = source.auto_join_server;
    save(&target)?;
    Ok(target)
}

/// Longest name accepted by [`rename`] — keeps cards and the sidebar readable.
pub const MAX_NAME_LEN: usize = 64;

pub fn validate_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Instance("le nom de l'instance ne peut pas être vide".to_string()));
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(AppError::Instance(format!(
            "le nom de l'instance ne peut pas dépasser {MAX_NAME_LEN} caractères"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn rename(paths: &AppPaths, id: &str, name: &str) -> AppResult<Instance> {
    let name = validate_name(name)?;
    let mut instance = get(paths, id)?;
    instance.name = name;
    save(&instance)?;
    Ok(instance)
}

#[cfg(test)]
mod tests;
