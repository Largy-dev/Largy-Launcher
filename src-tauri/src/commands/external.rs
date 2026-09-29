//! Import from other launchers — see [`crate::instances::external`].

use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::instances::external::{self, ExternalInstance, Roots};
use crate::instances::{self, Instance};
use crate::minecraft::manifest;
use crate::state::AppState;

use super::instances::{instances_changed, spawn_blocking};

async fn detect_all(state: &AppState) -> AppResult<Vec<ExternalInstance>> {
    // "Latest release" profiles need the current release; offline they're skipped.
    let latest = manifest::fetch_version_manifest(&state.meta).await.ok().map(|m| m.latest.release);
    let paths = state.paths.clone();
    spawn_blocking(move || {
        let imported: Vec<String> =
            instances::list(&paths)?.into_iter().filter_map(|i| i.imported_from).collect();
        Ok(external::detect(&Roots::system(), latest.as_deref(), &imported))
    })
    .await
}

/// Instances found in the official launcher, Prism / MultiMC, the CurseForge
/// app and the Modrinth App.
#[tauri::command]
pub async fn external_instances_detect(state: State<'_, AppState>) -> AppResult<Vec<ExternalInstance>> {
    detect_all(&state).await
}

/// Copies one of them into a new instance (only ids [`external_instances_detect`] reports).
#[tauri::command]
pub async fn external_instance_import(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    include_worlds: bool,
) -> AppResult<Instance> {
    let found = detect_all(&state)
        .await?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::Instance("instance introuvable dans les autres launchers".to_string()))?;
    let paths = state.paths.clone();
    let instance = spawn_blocking(move || external::import(&paths, &found, include_worlds)).await?;
    instances_changed(&app);
    Ok(instance)
}
