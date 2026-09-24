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

#[tauri::command]
pub async fn providers_get_changelog(
    state: State<'_, AppState>,
    provider: String,
    pack_id: String,
    version_id: String,
) -> AppResult<Option<String>> {
    Ok(get_provider(&state, &provider)?.get_changelog(&pack_id, &version_id).await?)
}

/// Whether this build ships the launcher's own CurseForge API key — the
/// CurseForge tab then works without the player entering one.
#[tauri::command]
pub fn providers_curseforge_builtin_key() -> bool {
    crate::providers::curseforge::builtin_key().is_some()
}

/// The FTB-published copy of a CurseForge pack, if there is one. Best
/// effort: a failed lookup just means no suggestion.
#[tauri::command]
pub async fn providers_ftb_equivalent(
    state: State<'_, AppState>,
    curseforge_id: u64,
    name: String,
) -> AppResult<Option<ModpackSummary>> {
    let Some(ftb) = state.providers.get("ftb") else {
        return Ok(None);
    };
    Ok(ftb.find_curseforge_equivalent(curseforge_id, &name).await.unwrap_or_else(|e| {
        tracing::debug!("FTB equivalent lookup failed: {e}");
        None
    }))
}
