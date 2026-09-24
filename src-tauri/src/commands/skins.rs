//! Skin and cape of the active Microsoft account, and the local skin library.

use tauri::State;

use crate::auth::AccountSession;
use crate::error::{AppError, AppResult};
use crate::skins::library::{self, LibrarySkin, LibrarySkinView};
use crate::skins::{self, SkinProfile, SkinVariant};
use crate::state::AppState;

use super::instances::spawn_blocking;

/// The active account with a token Minecraft Services will accept,
/// refreshed first when it's about to expire.
async fn online_session(state: &AppState) -> AppResult<AccountSession> {
    let current = state
        .active_account
        .read()
        .clone()
        .ok_or_else(|| AppError::Auth("Connecte-toi avec un compte Microsoft pour gérer ton skin.".to_string()))?;
    let session = if crate::auth::needs_refresh(&current, crate::auth::now_unix()) {
        let client_id = state.settings.read().azure_client_id.clone();
        let refreshed = crate::auth::refresh_or_keep(&state.paths, &state.client, &client_id, current).await?;
        *state.active_account.write() = Some(refreshed.clone());
        refreshed
    } else {
        current
    };
    if session.view().offline {
        return Err(AppError::Auth(
            "Les serveurs Minecraft sont injoignables : réessaie une fois connecté à internet.".to_string(),
        ));
    }
    Ok(session)
}

fn read_skin_file(path: &str) -> AppResult<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    skins::validate_skin(&bytes)?;
    Ok(bytes)
}

#[tauri::command]
pub async fn skins_get_profile(state: State<'_, AppState>) -> AppResult<SkinProfile> {
    let session = online_session(&state).await?;
    skins::get_profile(&state.client, &session.minecraft_access_token).await
}

/// Uploads a PNG from disk and keeps a copy in the library.
#[tauri::command]
pub async fn skins_upload(
    state: State<'_, AppState>,
    path: String,
    variant: SkinVariant,
    name: String,
) -> AppResult<SkinProfile> {
    let png = read_skin_file(&path)?;
    let session = online_session(&state).await?;
    let profile = skins::upload_skin(&state.client, &session.minecraft_access_token, png.clone(), variant).await?;
    if let Err(e) = library::add(&state.paths.skins_dir(), &name, variant, &png, crate::auth::now_unix()) {
        tracing::warn!("could not save the skin to the library: {e}");
    }
    Ok(profile)
}

#[tauri::command]
pub async fn skins_reset(state: State<'_, AppState>) -> AppResult<SkinProfile> {
    let session = online_session(&state).await?;
    skins::reset_skin(&state.client, &session.minecraft_access_token).await
}

#[tauri::command]
pub async fn skins_set_cape(state: State<'_, AppState>, cape_id: Option<String>) -> AppResult<SkinProfile> {
    let session = online_session(&state).await?;
    skins::set_cape(&state.client, &session.minecraft_access_token, cape_id.as_deref()).await
}

/// A skin file from disk as a data URL, for the preview before upload.
#[tauri::command]
pub fn skins_read_file(path: String) -> AppResult<String> {
    Ok(skins::data_url(&read_skin_file(&path)?))
}

#[tauri::command]
pub async fn skins_library_list(state: State<'_, AppState>) -> AppResult<Vec<LibrarySkinView>> {
    let dir = state.paths.skins_dir();
    spawn_blocking(move || Ok(library::list(&dir))).await
}

#[tauri::command]
pub fn skins_library_add(
    state: State<'_, AppState>,
    path: String,
    variant: SkinVariant,
    name: String,
) -> AppResult<LibrarySkin> {
    let png = read_skin_file(&path)?;
    library::add(&state.paths.skins_dir(), &name, variant, &png, crate::auth::now_unix())
}

#[tauri::command]
pub fn skins_library_remove(state: State<'_, AppState>, id: String) -> AppResult<()> {
    library::remove(&state.paths.skins_dir(), &id)
}

/// Wears a library skin.
#[tauri::command]
pub async fn skins_library_apply(state: State<'_, AppState>, id: String) -> AppResult<SkinProfile> {
    let (skin, png) = library::get(&state.paths.skins_dir(), &id)?;
    let session = online_session(&state).await?;
    skins::upload_skin(&state.client, &session.minecraft_access_token, png, skin.variant).await
}
