use std::sync::Arc;

use tauri::State;
use tokio::sync::Notify;

use crate::auth::{self, interactive, ms_oauth::DeviceCodeInfo, AccountView, StoredAccount};
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

/// Whether signing in inside a launcher window works for this app
/// registration (else the device-code flow is used).
#[tauri::command]
pub async fn auth_window_login_available(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(interactive::is_available(&state.client, &client_id(&state)).await)
}

const LOGIN_WINDOW: &str = "ms-login";

/// Opens the Microsoft sign-in page in a window and waits for its redirect.
/// Closing the window (or [`auth_cancel_login`]) cancels.
#[tauri::command]
pub async fn auth_window_login(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<AccountView> {
    use tauri::Manager;

    let client_id = client_id(&state);
    let pkce = interactive::Pkce::new();
    let expected_state = uuid::Uuid::new_v4().simple().to_string();
    let url = interactive::authorize_url(&client_id, &pkce, &expected_state);
    let url: tauri::Url = url.parse().map_err(|e| crate::error::AppError::Auth(format!("adresse invalide : {e}")))?;

    if let Some(previous) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = previous.destroy();
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    let tx = Arc::new(parking_lot::Mutex::new(Some(tx)));
    let on_redirect = tx.clone();
    let window = tauri::WebviewWindowBuilder::new(&app, LOGIN_WINDOW, tauri::WebviewUrl::External(url))
        .title("Connexion Microsoft — Largy Launcher")
        .inner_size(500.0, 700.0)
        // Its own WebView2 profile: isolated from the launcher's page, and
        // never at odds with the main window's environment options.
        .data_directory(state.paths.cache_dir().join("login-webview"))
        .resizable(true)
        .center()
        .on_navigation(move |target| {
            if interactive::is_redirect(target.as_str()) {
                if let Some(tx) = on_redirect.lock().take() {
                    let _ = tx.send(Some(target.to_string()));
                }
                return false;
            }
            true
        })
        .build()
        .map_err(|e| crate::error::AppError::Auth(format!("fenêtre de connexion impossible à ouvrir : {e}")))?;
    let on_close = tx.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            if let Some(tx) = on_close.lock().take() {
                let _ = tx.send(None);
            }
        }
    });

    let cancel = Arc::new(Notify::new());
    if let Some(previous) = state.login_cancel.lock().replace(cancel.clone()) {
        previous.notify_one();
    }
    let redirect = tokio::select! {
        result = rx => result.ok().flatten(),
        _ = cancel.notified() => None,
        _ = tokio::time::sleep(std::time::Duration::from_secs(15 * 60)) => None,
    };
    {
        let mut slot = state.login_cancel.lock();
        if slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cancel)) {
            *slot = None;
        }
    }
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = window.destroy();
    }
    let redirect = redirect.ok_or(crate::error::AppError::Cancelled)?;
    let code = interactive::code_from_redirect(&redirect, &expected_state)?;
    let tokens = interactive::exchange_code(&state.client, &client_id, &code, &pkce).await?;
    let session = auth::login_with_tokens(&state.paths, &state.client, &tokens)
        .await
        .inspect_err(|e| tracing::warn!("login failed: {e}"))?;
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
