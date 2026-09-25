//! The server list of an instance (its `servers.dat`) and status pings.

use tauri::State;

use crate::error::AppResult;
use crate::instances::{self, content::{self, ContentKind}};
use crate::providers::modrinth::ModrinthApi;
use crate::providers::InstallWarning;
use crate::servers::{self, ping::ServerStatus, ServerEntry};
use crate::state::AppState;

use super::instances::ensure_not_running;
use super::modpacks::InstanceInstallResult;

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
/// server joins the multiplayer list and is joined on launch, and the mods
/// the server adds on top of the pack are installed.
#[tauri::command]
pub async fn servers_attach_instance(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    featured_id: String,
    address: String,
) -> AppResult<InstanceInstallResult> {
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
    instance.featured_server = Some(featured_id.clone());
    instances::save(&instance)?;

    let catalog = servers::featured::load(&state.client).await;
    let extra_mods = catalog
        .servers
        .into_iter()
        .find(|s| s.id == featured_id)
        .and_then(|s| s.modpack)
        .map(|m| m.extra_mods)
        .unwrap_or_default();
    let api = ModrinthApi::new(state.client.clone());
    let mut warnings = Vec::new();
    for extra in &extra_mods {
        let installed = async {
            // Pinned by the catalog: the exact file, not the latest release.
            let version = api.version(&extra.version_id).await.map_err(crate::error::AppError::from)?;
            content::install_exact(&state.downloader, &instance, &version, ContentKind::Mod).await
        };
        if let Err(e) = installed.await {
            warnings.push(InstallWarning {
                file_name: extra.project.clone(),
                message: e.to_string(),
                browser_url: Some(format!("https://modrinth.com/mod/{}/version/{}", extra.project, extra.version_id)),
                ..Default::default()
            });
        }
    }

    super::instances::instances_changed(&app);
    Ok(InstanceInstallResult { instance, warnings })
}
