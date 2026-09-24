//! Instance CRUD and per-instance settings. Modpack install / update /
//! import / export live in [`super::modpacks`].

use std::path::Path;

use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AppResult};
use crate::instances::{self, CreateInstanceInput, Instance};
use crate::providers::LoaderKind;
use crate::state::AppState;
use crate::util::fs::is_plain_file_name;

pub(super) async fn spawn_blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("tâche de fond interrompue: {e}")))?
}

pub(super) fn instances_changed(app: &AppHandle) {
    let _ = app.emit("instances-changed", ());
}

pub(super) fn ensure_not_running(state: &AppState, id: &str) -> AppResult<()> {
    if state.running.lock().contains_key(id) {
        return Err(AppError::Instance("ferme le jeu de cette instance avant de faire ça".to_string()));
    }
    if state.installs.lock().contains_key(id) {
        return Err(AppError::Instance("une installation est en cours sur cette instance".to_string()));
    }
    Ok(())
}

#[tauri::command]
pub fn instances_list(state: State<'_, AppState>) -> AppResult<Vec<Instance>> {
    instances::list(&state.paths)
}

#[tauri::command]
pub fn instances_get(state: State<'_, AppState>, id: String) -> AppResult<Instance> {
    instances::get(&state.paths, &id)
}

#[tauri::command]
pub fn instances_create(
    state: State<'_, AppState>,
    name: String,
    minecraft_version: String,
    loader: LoaderKind,
    loader_version: Option<String>,
) -> AppResult<Instance> {
    let name = instances::validate_name(&name)?;
    if loader != LoaderKind::Vanilla && loader_version.as_deref().is_none_or(|v| v.trim().is_empty()) {
        return Err(AppError::Instance("choisis une version du mod loader".to_string()));
    }
    instances::create(
        &state.paths,
        CreateInstanceInput { name, minecraft_version, loader, loader_version, modpack: None, icon_url: None },
    )
}

#[tauri::command]
pub async fn instances_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    ensure_not_running(&state, &id)?;
    let paths = state.paths.clone();
    spawn_blocking(move || instances::delete(&paths, &id)).await
}

#[tauri::command]
pub fn instances_rename(state: State<'_, AppState>, id: String, name: String) -> AppResult<Instance> {
    instances::rename(&state.paths, &id, &name)
}

#[tauri::command]
pub async fn instances_duplicate(app: AppHandle, state: State<'_, AppState>, id: String, name: String) -> AppResult<Instance> {
    let paths = state.paths.clone();
    let instance = spawn_blocking(move || instances::duplicate(&paths, &id, &name)).await?;
    instances_changed(&app);
    Ok(instance)
}

#[derive(Debug, serde::Deserialize)]
pub struct InstanceSettingsInput {
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub extra_jvm_args: Vec<String>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub window_width: Option<u32>,
    #[serde(default)]
    pub window_height: Option<u32>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub auto_join_server: Option<String>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[tauri::command]
pub fn instances_update_settings(
    state: State<'_, AppState>,
    id: String,
    settings: InstanceSettingsInput,
) -> AppResult<Instance> {
    let mut instance = instances::get(&state.paths, &id)?;
    let (min, max) = match (settings.min_memory_mb, settings.max_memory_mb) {
        (Some(min), Some(max)) => {
            let (min, max) = crate::settings::sanitize_memory(min, max);
            (Some(min), Some(max))
        }
        (min, max) => (
            min.map(|m| m.max(128)),
            max.map(|m| m.max(crate::settings::MIN_HEAP_MB)),
        ),
    };
    let valid_size = |v: Option<u32>| v.filter(|&n| (320..=16384).contains(&n));
    instance.min_memory_mb = min;
    instance.max_memory_mb = max;
    instance.extra_jvm_args = settings.extra_jvm_args.into_iter().filter(|a| !a.trim().is_empty()).collect();
    instance.java_path = non_empty(settings.java_path);
    instance.window_width = valid_size(settings.window_width);
    instance.window_height = valid_size(settings.window_height);
    instance.fullscreen = settings.fullscreen;
    instance.auto_join_server = non_empty(settings.auto_join_server);
    instances::save(&instance)?;
    Ok(instance)
}

#[tauri::command]
pub fn instances_set_pinned(state: State<'_, AppState>, id: String, pinned: bool) -> AppResult<Instance> {
    instances::set_pinned(&state.paths, &id, pinned)
}

#[tauri::command]
pub fn instances_set_protected(state: State<'_, AppState>, id: String, protected: bool) -> AppResult<Instance> {
    instances::set_protected(&state.paths, &id, protected)
}

#[tauri::command]
pub fn instances_set_notes(state: State<'_, AppState>, id: String, notes: String) -> AppResult<Instance> {
    instances::set_notes(&state.paths, &id, &notes)
}

/// Copies `source_id`'s memory/JVM/window/server settings onto `target_id`.
#[tauri::command]
pub fn instances_copy_settings(state: State<'_, AppState>, source_id: String, target_id: String) -> AppResult<Instance> {
    instances::copy_settings(&state.paths, &source_id, &target_id)
}

/// Opens the instance folder, or one of its sub-folders (`mods`, `saves`,
/// `logs`, ...) — created on demand.
#[tauri::command]
pub fn instances_open_folder(state: State<'_, AppState>, id: String, sub: Option<String>) -> AppResult<()> {
    let instance = instances::get(&state.paths, &id)?;
    let target = match sub.as_deref() {
        Some("backups") => state.paths.backups_dir(&id),
        Some(sub) if is_plain_file_name(sub) => instance.directory.join(sub),
        Some(sub) => return Err(AppError::Instance(format!("dossier invalide: {sub}"))),
        None => instance.directory.clone(),
    };
    std::fs::create_dir_all(&target)?;
    open_in_file_manager(&target);
    Ok(())
}

/// Reveals a crash report (or any file) that lives inside an instance.
#[tauri::command]
pub fn instances_reveal_file(state: State<'_, AppState>, id: String, path: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &id)?;
    let file = std::fs::canonicalize(&path)?;
    let root = std::fs::canonicalize(&instance.directory)?;
    if !file.starts_with(&root) {
        return Err(AppError::Instance("ce fichier n'appartient pas à l'instance".to_string()));
    }
    open_in_file_manager(&file);
    Ok(())
}

fn open_in_file_manager(path: &Path) {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("explorer");
        if path.is_file() {
            cmd.arg(format!("/select,{}", path.display()));
        } else {
            cmd.arg(path);
        }
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

/// Puts a "play this instance" shortcut on the desktop; returns its path.
#[tauri::command]
pub fn instances_create_shortcut(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<String> {
    use tauri::Manager;
    let instance = instances::get(&state.paths, &id)?;
    let desktop = app.path().desktop_dir().map_err(|e| AppError::Other(format!("bureau introuvable : {e}")))?;
    let path = crate::shortcuts::create_shortcut(&desktop, &instance.id, &instance.name)?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn instance_screenshots_list(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<crate::instances::screenshots::Screenshot>> {
    let instance = instances::get(&state.paths, &id)?;
    crate::instances::screenshots::list(&instance.directory)
}

#[tauri::command]
pub fn instance_screenshots_delete(state: State<'_, AppState>, id: String, file_name: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &id)?;
    crate::instances::screenshots::delete(&instance.directory, &file_name)
}
