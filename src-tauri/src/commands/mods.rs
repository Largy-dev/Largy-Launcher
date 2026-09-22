//! Local mod management commands (v1 scope: list/enable/disable/delete/add
//! jars already or newly placed in an instance's `mods/` folder). In-app
//! marketplace search (CurseForge/Modrinth) is a meaningfully larger feature
//! — per-loader/MC-version compatibility resolution, dependency handling —
//! and deliberately out of scope here.

use tauri::State;

use crate::error::AppResult;
use crate::instances::{self, mods};
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

#[tauri::command]
pub fn instance_mods_add(state: State<'_, AppState>, instance_id: String, source_path: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &instance_id)?;
    mods::add_from_path(&instance.directory, std::path::Path::new(&source_path))
}
