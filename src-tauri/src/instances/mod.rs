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
}

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
        ..source
    };
    save(&instance)?;
    Ok(instance)
}

pub fn delete(paths: &AppPaths, id: &str) -> AppResult<()> {
    validate_id(id)?;
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
mod tests {
    use super::*;

    fn test_input(name: &str) -> CreateInstanceInput {
        CreateInstanceInput {
            name: name.to_string(),
            minecraft_version: "1.20.1".to_string(),
            loader: LoaderKind::Vanilla,
            loader_version: None,
            modpack: None,
            icon_url: None,
        }
    }

    #[test]
    fn create_then_get_round_trips_and_creates_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());

        let created = create(&paths, test_input("Demo")).unwrap();
        let fetched = get(&paths, &created.id).unwrap();

        assert_eq!(fetched.name, "Demo");
        assert_eq!(fetched.minecraft_version, "1.20.1");
        for sub in ["mods", "saves", "config", "resourcepacks", "shaderpacks", "natives"] {
            assert!(created.directory.join(sub).is_dir());
        }
    }

    #[test]
    fn list_sorts_newest_first_and_skips_unreadable_entries() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());

        let mut first = create(&paths, test_input("First")).unwrap();
        first.created_at = 100;
        save(&first).unwrap();

        let mut second = create(&paths, test_input("Second")).unwrap();
        second.created_at = 200;
        save(&second).unwrap();

        // An instance folder with a corrupt instance.json must not break listing.
        let broken_dir = paths.instances_dir().join("broken");
        std::fs::create_dir_all(&broken_dir).unwrap();
        std::fs::write(broken_dir.join("instance.json"), "not json").unwrap();

        let listed = list(&paths).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "Second");
        assert_eq!(listed[1].name, "First");
    }

    #[test]
    fn get_missing_instance_returns_instance_error() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        assert!(matches!(get(&paths, "does-not-exist"), Err(AppError::Instance(_))));
    }

    #[test]
    fn delete_removes_the_instance_directory() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let created = create(&paths, test_input("ToDelete")).unwrap();

        delete(&paths, &created.id).unwrap();

        assert!(!created.directory.exists());
        assert!(get(&paths, &created.id).is_err());
    }

    #[test]
    fn touch_last_played_updates_and_persists_the_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let created = create(&paths, test_input("Played")).unwrap();
        assert!(created.last_played_at.is_none());

        touch_last_played(&paths, &created.id).unwrap();

        let fetched = get(&paths, &created.id).unwrap();
        assert!(fetched.last_played_at.is_some());
    }

    #[test]
    fn add_play_time_accumulates_across_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let created = create(&paths, test_input("Timed")).unwrap();
        assert_eq!(created.play_time_seconds, 0);

        add_play_time(&paths, &created.id, 90).unwrap();
        add_play_time(&paths, &created.id, 30).unwrap();

        assert_eq!(get(&paths, &created.id).unwrap().play_time_seconds, 120);
    }

    #[test]
    fn instances_saved_before_play_time_existed_default_to_zero() {
        let json = r#"{"id":"a","name":"Old","minecraft_version":"1.20.1","loader":"vanilla",
            "loader_version":null,"directory":"x"}"#;
        let instance: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(instance.play_time_seconds, 0);
    }

    #[test]
    fn ids_that_could_escape_the_instances_folder_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        create(&paths, test_input("Survivor")).unwrap();

        for bad in ["", ".", "..", "../x", "a/b", "a\\b"] {
            assert!(delete(&paths, bad).is_err(), "{bad:?} should be rejected");
            assert!(get(&paths, bad).is_err());
        }
        assert_eq!(list(&paths).unwrap().len(), 1);
    }

    #[test]
    fn duplicate_copies_files_under_a_new_id_and_resets_stats() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let mut source = create(&paths, test_input("Original")).unwrap();
        source.play_time_seconds = 99;
        save(&source).unwrap();
        std::fs::write(source.directory.join("mods/a.jar"), b"jar").unwrap();

        let copy = duplicate(&paths, &source.id, "Copie").unwrap();

        assert_ne!(copy.id, source.id);
        assert_eq!(copy.name, "Copie");
        assert_eq!(copy.play_time_seconds, 0);
        assert_eq!(std::fs::read(copy.directory.join("mods/a.jar")).unwrap(), b"jar");
        assert_eq!(get(&paths, &copy.id).unwrap().directory, paths.instance_dir(&copy.id));
    }

    #[test]
    fn rename_trims_and_persists_the_new_name() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let created = create(&paths, test_input("Before")).unwrap();

        let renamed = rename(&paths, &created.id, "  After  ").unwrap();

        assert_eq!(renamed.name, "After");
        assert_eq!(get(&paths, &created.id).unwrap().name, "After");
    }

    #[test]
    fn rename_rejects_empty_and_too_long_names() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let created = create(&paths, test_input("Keep")).unwrap();

        assert!(rename(&paths, &created.id, "   ").is_err());
        assert!(rename(&paths, &created.id, &"x".repeat(MAX_NAME_LEN + 1)).is_err());
        assert_eq!(get(&paths, &created.id).unwrap().name, "Keep");
    }
}
