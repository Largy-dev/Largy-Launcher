//! The server list of an instance (its `servers.dat`) and status pings.

use tauri::State;

use crate::error::AppResult;
use crate::instances;
use crate::servers::{self, ping::ServerStatus, ServerEntry};
use crate::state::AppState;

use super::instances::ensure_not_running;

#[tauri::command]
pub fn instance_servers_list(state: State<'_, AppState>, instance_id: String) -> AppResult<Vec<ServerEntry>> {
    let instance = instances::get(&state.paths, &instance_id)?;
    servers::list(&instance.directory)
}

/// Editing is refused while the game runs: it rewrites `servers.dat` itself.
#[tauri::command]
pub fn instance_servers_add(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
    address: String,
) -> AppResult<()> {
    ensure_not_running(&state, &instance_id)?;
    let instance = instances::get(&state.paths, &instance_id)?;
    servers::add(&instance.directory, &name, &address)
}

#[tauri::command]
pub fn instance_servers_update(
    state: State<'_, AppState>,
    instance_id: String,
    index: usize,
    name: String,
    address: String,
) -> AppResult<()> {
    ensure_not_running(&state, &instance_id)?;
    let instance = instances::get(&state.paths, &instance_id)?;
    servers::update(&instance.directory, index, &name, &address)
}

#[tauri::command]
pub fn instance_servers_remove(state: State<'_, AppState>, instance_id: String, index: usize) -> AppResult<()> {
    ensure_not_running(&state, &instance_id)?;
    let instance = instances::get(&state.paths, &instance_id)?;
    servers::remove(&instance.directory, index)
}

#[tauri::command]
pub async fn server_ping(address: String) -> AppResult<ServerStatus> {
    servers::ping::ping(&address).await
}
