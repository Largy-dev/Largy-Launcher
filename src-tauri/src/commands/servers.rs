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

/// The "Serveurs" catalog (online copy, or the one built into the app).
#[tauri::command]
pub async fn featured_servers(state: State<'_, AppState>) -> AppResult<servers::featured::Catalog> {
    Ok(servers::featured::load(&state.client).await)
}

/// Creates a Fabric instance ready for one server: mods, shaders, server list.
#[tauri::command]
pub async fn servers_prepare_instance(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    spec: servers::prepare::PrepareSpec,
) -> AppResult<servers::prepare::PrepareResult> {
    let result = servers::prepare::prepare(&state, spec).await?;
    super::instances::instances_changed(&app);
    Ok(result)
}

/// Links a freshly installed modpack instance to its catalog server: the
/// server joins the multiplayer list and is joined on launch.
#[tauri::command]
pub fn servers_attach_instance(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    featured_id: String,
    address: String,
) -> AppResult<instances::Instance> {
    servers::validate_address(&address)?;
    if !servers::featured::is_slug(&featured_id) {
        return Err(crate::error::AppError::Other("serveur inconnu".to_string()));
    }
    let mut instance = instances::get(&state.paths, &instance_id)?;
    let address = address.trim().to_string();
    if !servers::list(&instance.directory)?.iter().any(|s| s.address == address) {
        servers::add(&instance.directory, &instance.name, &address)?;
    }
    instance.auto_join_server = Some(address);
    instance.featured_server = Some(featured_id);
    instances::save(&instance)?;
    super::instances::instances_changed(&app);
    Ok(instance)
}
