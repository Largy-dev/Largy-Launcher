//! Restore points of an instance — see [`crate::instances::snapshots`].

use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::instances::snapshots::{self, Snapshot};
use crate::instances::{self, Instance};
use crate::state::AppState;

use super::instances::{ensure_not_running, instances_changed, spawn_blocking};

#[tauri::command]
pub fn instance_snapshots_list(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<Snapshot>> {
    instances::validate_id(&instance_id)?;
    Ok(snapshots::list(&state.paths, &instance_id))
}

#[tauri::command]
pub async fn instance_snapshot_create(state: State<'_, AppState>, instance_id: String) -> AppResult<Snapshot> {
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        snapshots::create(&paths, &instance, "Point de restauration manuel")
    })
    .await
}

#[tauri::command]
pub async fn instance_snapshot_restore(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    snapshot_id: String,
) -> AppResult<Instance> {
    ensure_not_running(&state, &instance_id)?;
    let _guard = state.begin_install(&instance_id)?;
    let paths = state.paths.clone();
    let instance = spawn_blocking(move || {
        let instance = instances::get(&paths, &instance_id)?;
        snapshots::restore(&paths, &instance, &snapshot_id)
    })
    .await?;
    instances_changed(&app);
    Ok(instance)
}

#[tauri::command]
pub fn instance_snapshot_delete(state: State<'_, AppState>, instance_id: String, snapshot_id: String) -> AppResult<()> {
    instances::validate_id(&instance_id)?;
    snapshots::delete(&state.paths, &instance_id, &snapshot_id)
}
