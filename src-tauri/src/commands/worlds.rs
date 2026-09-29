//! Singleplayer worlds of an instance: list, back up, restore, import,
//! delete — see [`crate::instances::worlds`].

use tauri::State;

use crate::error::AppResult;
use crate::instances::worlds::{self, World, WorldBackup};
use crate::instances::{self};
use crate::state::AppState;

use super::instances::{ensure_not_running, open_in_file_manager, spawn_blocking};

#[tauri::command]
pub async fn instance_worlds_list(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<World>> {
    let paths = state.paths.clone();
    spawn_blocking(move || worlds::list(&instances::get(&paths, &instance_id)?.directory)).await
}

#[tauri::command]
pub async fn instance_world_backup(
    state: State<'_, AppState>,
    instance_id: String,
    folder: String,
) -> AppResult<WorldBackup> {
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        worlds::backup(&paths, &instance.id, &instance.directory, &folder)
    })
    .await
}

/// Deletes a world, after zipping it into its backups when `backup_first`.
#[tauri::command]
pub async fn instance_world_delete(
    state: State<'_, AppState>,
    instance_id: String,
    folder: String,
    backup_first: bool,
) -> AppResult<()> {
    ensure_not_running(&state, &instance_id)?;
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        if backup_first {
            worlds::backup(&paths, &instance.id, &instance.directory, &folder)?;
        }
        worlds::delete(&instance.directory, &folder)
    })
    .await
}

#[tauri::command]
pub fn instance_world_open_folder(state: State<'_, AppState>, instance_id: String, folder: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &instance_id)?;
    open_in_file_manager(&worlds::world_dir(&instance.directory, &folder)?);
    Ok(())
}

/// Imports worlds from zips or world folders; resolves to the folders created.
#[tauri::command]
pub async fn instance_worlds_import(
    state: State<'_, AppState>,
    instance_id: String,
    source_paths: Vec<String>,
) -> AppResult<Vec<String>> {
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        let mut created = Vec::new();
        for source in &source_paths {
            created.extend(worlds::import(&instance.directory, std::path::Path::new(source))?);
        }
        Ok(created)
    })
    .await
}

#[tauri::command]
pub fn instance_world_backups(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<WorldBackup>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    Ok(worlds::list_backups(&state.paths, &instance.id))
}

/// Restores every world of a backup (never over an existing one); resolves
/// to the folders created.
#[tauri::command]
pub async fn instance_world_restore(
    state: State<'_, AppState>,
    instance_id: String,
    backup_id: String,
) -> AppResult<Vec<String>> {
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        worlds::restore_backup(&paths, &instance.id, &instance.directory, &backup_id)
    })
    .await
}

#[tauri::command]
pub fn instance_world_backup_delete(state: State<'_, AppState>, instance_id: String, backup_id: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &instance_id)?;
    worlds::delete_backup(&state.paths, &instance.id, &backup_id)
}
