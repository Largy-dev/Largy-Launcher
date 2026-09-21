//! Thin `#[tauri::command]` handlers, one module per screen/domain. Each
//! handler just validates input and delegates into the matching top-level
//! module (`auth`, `instances`, `providers`, ...) — no business logic here.

pub mod auth;
pub mod instances;
pub mod launch;
pub mod minecraft;
pub mod providers;
pub mod settings;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::providers::LoaderKind;
use crate::state::AppState;

#[tauri::command]
pub fn app_version() -> AppResult<String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub async fn loaders_list_versions(
    state: State<'_, AppState>,
    loader: LoaderKind,
    minecraft_version: String,
) -> AppResult<Vec<String>> {
    let installer = state
        .loaders
        .get(loader)
        .ok_or_else(|| AppError::Loader("mod loader non supporté".to_string()))?;
    Ok(installer.list_versions(&minecraft_version).await.map_err(AppError::from)?)
}
