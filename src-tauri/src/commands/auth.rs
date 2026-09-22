use tauri::State;

use crate::auth::{self, ms_oauth::DeviceCodeInfo, AccountSession};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn auth_begin_login(state: State<'_, AppState>) -> AppResult<DeviceCodeInfo> {
    let client_id = state.settings.read().azure_client_id.clone();
    auth::begin_login(&state.client, &client_id).await
}

#[tauri::command]
pub async fn auth_complete_login(state: State<'_, AppState>, device: DeviceCodeInfo) -> AppResult<AccountSession> {
    let client_id = state.settings.read().azure_client_id.clone();
    let session = auth::complete_login(&state.paths, &state.client, &client_id, &device).await?;
    *state.active_account.write() = Some(session.clone());
    Ok(session)
}

#[tauri::command]
pub async fn auth_try_silent_login(state: State<'_, AppState>) -> AppResult<Option<AccountSession>> {
    let client_id = state.settings.read().azure_client_id.clone();
    let session = auth::try_silent_login(&state.paths, &state.client, &client_id).await?;
    if let Some(session) = &session {
        *state.active_account.write() = Some(session.clone());
    }
    Ok(session)
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> AppResult<()> {
    auth::logout(&state.paths)?;
    *state.active_account.write() = None;
    Ok(())
}

#[tauri::command]
pub fn auth_get_active_account(state: State<'_, AppState>) -> Option<AccountSession> {
    state.active_account.read().clone()
}
