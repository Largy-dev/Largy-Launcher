use std::sync::Arc;

use tauri::State;
use tokio::sync::Notify;

use crate::auth::{self, ms_oauth::DeviceCodeInfo, AccountView, StoredAccount};
use crate::error::AppResult;
use crate::state::AppState;

fn client_id(state: &AppState) -> String {
    state.settings.read().azure_client_id.clone()
}

#[tauri::command]
pub async fn auth_begin_login(state: State<'_, AppState>) -> AppResult<DeviceCodeInfo> {
    auth::begin_login(&state.client, &client_id(&state)).await
}

#[tauri::command]
pub async fn auth_complete_login(state: State<'_, AppState>, device: DeviceCodeInfo) -> AppResult<AccountView> {
    let cancel = Arc::new(Notify::new());
    if let Some(previous) = state.login_cancel.lock().replace(cancel.clone()) {
        previous.notify_one();
    }
    let result = auth::complete_login(&state.paths, &state.client, &client_id(&state), &device, &cancel)
        .await
        .inspect_err(|e| tracing::warn!("login failed: {e}"));
    {
        let mut slot = state.login_cancel.lock();
        if slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cancel)) {
            *slot = None;
        }
    }
    let session = result?;
    let view = session.view();
    *state.active_account.write() = Some(session);
    Ok(view)
}

#[tauri::command]
pub fn auth_cancel_login(state: State<'_, AppState>) {
    if let Some(cancel) = state.login_cancel.lock().take() {
        cancel.notify_one();
    }
}

#[tauri::command]
pub async fn auth_try_silent_login(state: State<'_, AppState>) -> AppResult<Option<AccountView>> {
    let session = auth::try_silent_login(&state.paths, &state.client, &client_id(&state))
        .await
        .inspect_err(|e| tracing::warn!("silent login failed: {e}"))?;
    let view = session.as_ref().map(auth::AccountSession::view);
    if let Some(session) = session {
        *state.active_account.write() = Some(session);
    }
    Ok(view)
}

#[tauri::command]
pub async fn auth_switch_account(state: State<'_, AppState>, account_id: String) -> AppResult<AccountView> {
    let session = auth::switch_account(&state.paths, &state.client, &client_id(&state), &account_id)
        .await
        .inspect_err(|e| tracing::warn!("account switch failed: {e}"))?;
    let view = session.view();
    *state.active_account.write() = Some(session);
    Ok(view)
}

#[tauri::command]
pub fn auth_list_accounts(state: State<'_, AppState>) -> AppResult<Vec<StoredAccount>> {
    auth::list_accounts(&state.paths)
}

/// Forgets `account_id` (or the active account). Logging out of the active
/// account clears the session.
#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>, account_id: Option<String>) -> AppResult<()> {
    let was_active = auth::logout(&state.paths, account_id.as_deref())?;
    if was_active || account_id.is_none() {
        *state.active_account.write() = None;
    }
    Ok(())
}

#[tauri::command]
pub fn auth_get_active_account(state: State<'_, AppState>) -> Option<AccountView> {
    state.active_account.read().as_ref().map(auth::AccountSession::view)
}
