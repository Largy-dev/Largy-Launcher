use tauri::State;

use crate::error::AppResult;
use crate::minecraft::manifest::{self, VersionManifestEntry};
use crate::state::AppState;

#[tauri::command]
pub async fn minecraft_list_versions(state: State<'_, AppState>) -> AppResult<Vec<VersionManifestEntry>> {
    let manifest = manifest::fetch_version_manifest(&state.client).await?;
    Ok(manifest.versions)
}
