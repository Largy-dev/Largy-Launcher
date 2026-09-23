//! Instance content commands: local mod management (list / enable / disable
//! / delete / add jars) plus Modrinth browsing, installation and updates.

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::instances::content::{self, ContentKind, ModUpdate};
use crate::instances::{self, mods};
use crate::providers::modrinth::api::SearchHit;
use crate::providers::modrinth::ModrinthApi;
use crate::state::AppState;

#[tauri::command]
pub fn instance_mods_list(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<mods::ModEntry>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    mods::list(&instance.directory)
}

#[tauri::command]
pub fn instance_mods_set_enabled(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
    enabled: bool,
) -> AppResult<()> {
    let instance = instances::get(&state.paths, &instance_id)?;
    mods::set_enabled(&instance.directory, &file_name, enabled)
}

#[tauri::command]
pub fn instance_mods_delete(state: State<'_, AppState>, instance_id: String, file_name: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &instance_id)?;
    mods::delete(&instance.directory, &file_name)
}

/// Copies jars picked in a file dialog (or dropped on the window) into `mods/`.
#[tauri::command]
pub fn instance_mods_add(state: State<'_, AppState>, instance_id: String, source_paths: Vec<String>) -> AppResult<()> {
    let instance = instances::get(&state.paths, &instance_id)?;
    for source in source_paths {
        mods::add_from_path(&instance.directory, std::path::Path::new(&source))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn content_search(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    query: String,
    offset: u32,
) -> AppResult<Vec<SearchHit>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    content::search(&ModrinthApi::new(state.client.clone()), &instance, kind, &query, offset).await
}

#[tauri::command]
pub async fn content_install(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    kind: ContentKind,
) -> AppResult<Vec<String>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    content::install(&ModrinthApi::new(state.client.clone()), &state.downloader, &instance, &project_id, kind).await
}

#[tauri::command]
pub async fn instance_mods_check_updates(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<ModUpdate>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    content::check_updates(&ModrinthApi::new(state.client.clone()), &instance).await
}

/// Applies updates concurrently; returns the file names that failed.
#[tauri::command]
pub async fn instance_mods_apply_updates(
    state: State<'_, AppState>,
    instance_id: String,
    updates: Vec<ModUpdate>,
) -> AppResult<Vec<String>> {
    if state.running.lock().contains_key(&instance_id) {
        return Err(AppError::Instance("ferme le jeu avant de mettre à jour ses mods".to_string()));
    }
    let instance = instances::get(&state.paths, &instance_id)?;
    let semaphore = tokio::sync::Semaphore::new(6);
    let results = futures_util::future::join_all(updates.iter().map(|update| async {
        let _permit = semaphore.acquire().await;
        content::apply_update(&state.downloader, &instance, update)
            .await
            .map_err(|e| format!("{}: {e}", update.file_name))
    }))
    .await;
    Ok(results.into_iter().filter_map(Result::err).collect())
}
