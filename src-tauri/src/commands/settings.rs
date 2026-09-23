use tauri::State;

use crate::error::AppResult;
use crate::settings::GlobalSettings;
use crate::state::AppState;

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> GlobalSettings {
    state.settings.read().clone()
}

#[tauri::command]
pub fn settings_update(state: State<'_, AppState>, settings: GlobalSettings) -> AppResult<GlobalSettings> {
    let settings = settings.sanitized();
    settings.save(&state.paths)?;
    *state.curseforge_api_key.write() = settings.curseforge_api_key.clone();
    *state.settings.write() = settings.clone();
    Ok(settings)
}
