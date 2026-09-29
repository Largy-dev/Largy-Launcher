//! Instance content commands: what's installed in `mods/`, `resourcepacks/`
//! and `shaderpacks/` (list with metadata, identify, enable / disable /
//! delete / add, in bulk) plus Modrinth browsing, installation and updates.

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::instances::content::{self, ContentHit, ContentKind, ModUpdate};
use crate::instances::content_curseforge;
use crate::instances::installed::cache::RemoteProvider;
use crate::instances::installed::{self, ContentSummary, InstalledItem};
use crate::instances::{self, Instance};
use crate::providers::curseforge::CurseForgeProvider;
use crate::providers::modrinth::ModrinthApi;
use crate::state::AppState;

use super::instances::spawn_blocking;

/// A loaded jar is locked by the running game on Windows: renaming or
/// deleting it would fail with a cryptic "access denied".
fn ensure_game_closed(state: &AppState, instance_id: &str) -> AppResult<()> {
    if state.running.lock().contains_key(instance_id) {
        return Err(AppError::Instance("ferme le jeu avant de modifier le contenu de cette instance".to_string()));
    }
    Ok(())
}

fn curseforge(state: &AppState) -> CurseForgeProvider {
    CurseForgeProvider::new(
        state.client.clone(),
        state.curseforge_api_key.clone(),
        state.paths.cache_dir().join("curseforge"),
    )
}

#[tauri::command]
pub async fn instance_content_list(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
) -> AppResult<Vec<InstalledItem>> {
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        installed::list(&paths, &instance.directory, instance.loader, kind)
    })
    .await
}

#[tauri::command]
pub async fn instance_content_summary(state: State<'_, AppState>, instance_id: String) -> AppResult<ContentSummary> {
    let paths = state.paths.clone();
    spawn_blocking(move || Ok(installed::summary(&instances::get(&paths, &instance_id)?.directory))).await
}

/// Finds the Modrinth / CurseForge project of files not identified yet;
/// resolves to how many were found (the list is then worth refetching).
#[tauri::command]
pub async fn instance_content_identify(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
) -> AppResult<usize> {
    let instance = instances::get(&state.paths, &instance_id)?;
    let modrinth = ModrinthApi::new(state.client.clone());
    installed::identify(&state.paths, &modrinth, &curseforge(&state), &instance.directory, instance.loader, kind).await
}

/// Applies `action` to every file; resolves to `"name : error"` for each failure.
fn for_each_file(
    instance: &Instance,
    file_names: &[String],
    action: impl Fn(&std::path::Path, &str) -> AppResult<()>,
) -> Vec<String> {
    file_names
        .iter()
        .filter_map(|name| action(&instance.directory, name).err().map(|e| format!("{name} : {e}")))
        .collect()
}

#[tauri::command]
pub fn instance_content_set_enabled(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    file_names: Vec<String>,
    enabled: bool,
) -> AppResult<Vec<String>> {
    ensure_game_closed(&state, &instance_id)?;
    let instance = instances::get(&state.paths, &instance_id)?;
    Ok(for_each_file(&instance, &file_names, |dir, name| installed::set_enabled(dir, kind, name, enabled)))
}

#[tauri::command]
pub fn instance_content_delete(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    file_names: Vec<String>,
) -> AppResult<Vec<String>> {
    ensure_game_closed(&state, &instance_id)?;
    let instance = instances::get(&state.paths, &instance_id)?;
    Ok(for_each_file(&instance, &file_names, |dir, name| installed::delete(dir, kind, name)))
}

/// Copies files picked in a dialog (or dropped on the window) into the
/// content folder; resolves to the names added.
#[tauri::command]
pub async fn instance_content_add(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    source_paths: Vec<String>,
) -> AppResult<Vec<String>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    spawn_blocking(move || {
        source_paths
            .iter()
            .map(|source| installed::add_from_path(&instance.directory, kind, std::path::Path::new(source)))
            .collect()
    })
    .await
}

/// Whether CurseForge content can be browsed (a key is available).
#[tauri::command]
pub fn content_curseforge_available(state: State<'_, AppState>) -> bool {
    curseforge(&state).has_key()
}

#[tauri::command]
pub async fn content_search(
    state: State<'_, AppState>,
    instance_id: String,
    provider: RemoteProvider,
    kind: ContentKind,
    query: String,
    offset: u32,
) -> AppResult<Vec<ContentHit>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    match provider {
        RemoteProvider::Modrinth => {
            content::search(&ModrinthApi::new(state.client.clone()), &instance, kind, &query, offset).await
        }
        RemoteProvider::Curseforge => {
            content_curseforge::search(&curseforge(&state), &instance, kind, &query, offset).await
        }
    }
}

async fn installed_items(state: &AppState, instance: &Instance, kind: ContentKind) -> AppResult<Vec<InstalledItem>> {
    let (paths, dir, loader) = (state.paths.clone(), instance.directory.clone(), instance.loader);
    spawn_blocking(move || installed::list(&paths, &dir, loader, kind)).await
}

/// Installs a project (and its required dependencies); resolves to the
/// file names written.
#[tauri::command]
pub async fn content_install(
    state: State<'_, AppState>,
    instance_id: String,
    provider: RemoteProvider,
    project_id: String,
    kind: ContentKind,
) -> AppResult<Vec<String>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    match provider {
        RemoteProvider::Modrinth => {
            content::install(&ModrinthApi::new(state.client.clone()), &state.downloader, &instance, &project_id, kind)
                .await
        }
        RemoteProvider::Curseforge => {
            let present = installed_items(&state, &instance, kind).await?;
            content_curseforge::install(&curseforge(&state), &state.downloader, &instance, &project_id, kind, &present)
                .await
        }
    }
}

/// Newer versions of installed mods, from Modrinth (by hash) and CurseForge
/// (mods identified as CurseForge projects).
#[tauri::command]
pub async fn instance_mods_check_updates(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<ModUpdate>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    let mut updates = content::check_updates(&ModrinthApi::new(state.client.clone()), &instance).await?;
    let cf = curseforge(&state);
    if cf.has_key() {
        let items = installed_items(&state, &instance, ContentKind::Mod).await?;
        for update in content_curseforge::check_updates(&cf, &instance, &items).await? {
            if !updates.iter().any(|u| u.file_name == update.file_name) {
                updates.push(update);
            }
        }
    }
    updates.sort_by_key(|u| u.title.to_lowercase());
    Ok(updates)
}

/// Applies updates concurrently; returns the file names that failed.
#[tauri::command]
pub async fn instance_mods_apply_updates(
    state: State<'_, AppState>,
    instance_id: String,
    updates: Vec<ModUpdate>,
) -> AppResult<Vec<String>> {
    ensure_game_closed(&state, &instance_id)?;
    let instance = instances::get(&state.paths, &instance_id)?;
    if !updates.is_empty() {
        let (paths, before) = (state.paths.clone(), instance.clone());
        let reason = match updates.len() {
            1 => format!("Avant la mise à jour de {}", updates[0].title),
            n => format!("Avant la mise à jour de {n} mods"),
        };
        spawn_blocking(move || crate::instances::snapshots::create(&paths, &before, &reason)).await?;
    }
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
