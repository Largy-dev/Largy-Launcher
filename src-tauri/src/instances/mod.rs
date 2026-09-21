//! Per-instance data model and CRUD. Each instance is an isolated folder
//! (`mods/`, `saves/`, `config/`, `resourcepacks/`, `natives/`) with its own
//! `instance.json` — no shared state between instances besides the
//! libraries/assets/java caches in [`crate::paths::AppPaths`].

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::providers::LoaderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackRef {
    pub provider: String,
    pub pack_id: String,
    pub version_id: String,
    pub pack_name: String,
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
            Ok(instance) => instances.push(instance),
            Err(e) => tracing::warn!("skipping unreadable instance {:?}: {e}", entry.path()),
        }
    }
    instances.sort_by_key(|i| std::cmp::Reverse(i.created_at));
    Ok(instances)
}

pub fn get(paths: &AppPaths, id: &str) -> AppResult<Instance> {
    let path = paths.instance_dir(id).join("instance.json");
    if !path.exists() {
        return Err(AppError::Instance(format!("instance introuvable: {id}")));
    }
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save(instance: &Instance) -> AppResult<()> {
    std::fs::create_dir_all(&instance.directory)?;
    let path = instance.directory.join("instance.json");
    std::fs::write(path, serde_json::to_string_pretty(instance)?)?;
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
    };

    save(&instance)?;
    Ok(instance)
}

pub fn delete(paths: &AppPaths, id: &str) -> AppResult<()> {
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
