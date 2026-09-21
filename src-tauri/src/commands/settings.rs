use tauri::State;

use crate::error::AppResult;
use crate::settings::GlobalSettings;
use crate::state::AppState;

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> GlobalSettings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
pub fn settings_update(state: State<'_, AppState>, settings: GlobalSettings) -> AppResult<()> {
    settings.save(&state.paths)?;
    *state.curseforge_api_key.write().unwrap() = settings.curseforge_api_key.clone();
    *state.settings.write().unwrap() = settings;
    Ok(())
}
