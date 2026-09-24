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
    state.discord.set_enabled(settings.discord_rich_presence);
    *state.settings.write() = settings.clone();
    Ok(settings)
}

/// Size in bytes of the downloaded Forge/NeoForge installer jars — safe to
/// clear since they're redownloaded on demand, never read after install.
#[tauri::command]
pub fn settings_installer_cache_size(state: State<'_, AppState>) -> u64 {
    crate::util::fs::dir_size(&state.paths.installers_dir())
}

#[tauri::command]
pub fn settings_clear_installer_cache(state: State<'_, AppState>) -> AppResult<u64> {
    let dir = state.paths.installers_dir();
    let freed = crate::util::fs::dir_size(&dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(freed)
}
