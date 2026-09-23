use tauri::State;

use crate::error::{AppError, AppResult};
use crate::providers::{ModpackDetails, ModpackProvider, ModpackSummary, ModpackVersionSummary, SearchQuery};
use crate::state::AppState;

fn get_provider<'a>(state: &'a State<'_, AppState>, provider: &str) -> AppResult<&'a dyn ModpackProvider> {
    state.providers.get(provider).ok_or_else(|| AppError::Provider(format!("provider inconnu: {provider}")))
}

#[tauri::command]
pub async fn providers_search(
    state: State<'_, AppState>,
    provider: String,
    text: String,
    offset: Option<u32>,
) -> AppResult<Vec<ModpackSummary>> {
    Ok(get_provider(&state, &provider)?.search(SearchQuery { text, offset: offset.unwrap_or(0) }).await?)
}

#[tauri::command]
pub async fn providers_get_modpack(
    state: State<'_, AppState>,
    provider: String,
    pack_id: String,
) -> AppResult<ModpackDetails> {
    Ok(get_provider(&state, &provider)?.get_modpack(&pack_id).await?)
}

#[tauri::command]
pub async fn providers_get_versions(
    state: State<'_, AppState>,
    provider: String,
    pack_id: String,
) -> AppResult<Vec<ModpackVersionSummary>> {
    Ok(get_provider(&state, &provider)?.get_versions(&pack_id).await?)
}
