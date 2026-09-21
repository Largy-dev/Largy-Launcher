//! Microsoft → Xbox Live → XSTS → Minecraft Services auth chain. The device
//! code flow (`ms_oauth`) needs a public-client Azure AD application id,
//! supplied by the user via Settings (`GlobalSettings::azure_client_id`) —
//! this project ships without one baked in.

pub mod mc_auth;
pub mod ms_oauth;
pub mod token_store;
pub mod xbox;

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use ms_oauth::{DeviceCodeInfo, PollOutcome};
use token_store::TokenStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSession {
    pub profile: MinecraftProfile,
    pub minecraft_access_token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ActiveAccountMeta {
    id: String,
    name: String,
}

pub async fn begin_login(client: &reqwest::Client, azure_client_id: &str) -> AppResult<DeviceCodeInfo> {
    if azure_client_id.trim().is_empty() {
        return Err(AppError::Auth(
            "Azure Client ID manquant. Ouvre Paramètres et renseigne l'identifiant de ton \
             application Azure AD (client public, flux « device code » autorisé) pour activer \
             la connexion Microsoft."
                .to_string(),
        ));
    }
    ms_oauth::request_device_code(client, azure_client_id).await
}

async fn finish_chain(client: &reqwest::Client, ms_access_token: &str) -> AppResult<AccountSession> {
    let xbl = xbox::authenticate_xbl(client, ms_access_token).await?;
    let xsts = xbox::authorize_xsts(client, &xbl.token).await?;
    let (mc_token, expires_at) = mc_auth::login_with_xbox(client, &xsts.uhs, &xsts.token).await?;
    let profile = mc_auth::fetch_profile(client, &mc_token).await?;
    Ok(AccountSession {
        profile,
        minecraft_access_token: mc_token,
        expires_at,
    })
}

/// Polls the token endpoint until the user finishes the browser step, then
/// runs the rest of the chain and persists the refresh token + active
/// account pointer. Runs for as long as `device.expires_in` allows.
pub async fn complete_login(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
    device: &DeviceCodeInfo,
) -> AppResult<AccountSession> {
    let deadline = Instant::now() + Duration::from_secs(device.expires_in);
    let interval = Duration::from_secs(device.interval.max(1));

    loop {
        tokio::time::sleep(interval).await;
        if Instant::now() > deadline {
            return Err(AppError::Auth("Code expiré, réessaie.".to_string()));
        }

        match ms_oauth::poll_device_token(client, azure_client_id, &device.device_code).await? {
            PollOutcome::Pending => continue,
            PollOutcome::Expired => return Err(AppError::Auth("Code expiré, réessaie.".to_string())),
            PollOutcome::Success(tokens) => {
                let session = finish_chain(client, &tokens.access_token).await?;
                TokenStore::save_refresh_token(&session.profile.id, &tokens.refresh_token)?;
                save_active_account(paths, &session.profile)?;
                return Ok(session);
            }
        }
    }
}

/// Called on startup: silently re-authenticates the last active account
/// from its stored refresh token, or returns `None` if there isn't one.
pub async fn try_silent_login(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
) -> AppResult<Option<AccountSession>> {
    if azure_client_id.trim().is_empty() {
        return Ok(None);
    }

    let Some(meta) = load_active_account(paths)? else {
        return Ok(None);
    };
    let Some(refresh) = TokenStore::load_refresh_token(&meta.id)? else {
        return Ok(None);
    };

    let tokens = ms_oauth::refresh_token(client, azure_client_id, &refresh).await?;
    TokenStore::save_refresh_token(&meta.id, &tokens.refresh_token)?;
    let session = finish_chain(client, &tokens.access_token).await?;
    save_active_account(paths, &session.profile)?;
    Ok(Some(session))
}

pub fn logout(paths: &AppPaths) -> AppResult<()> {
    if let Some(meta) = load_active_account(paths)? {
        TokenStore::delete_refresh_token(&meta.id)?;
    }
    let path = paths.accounts_file();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn save_active_account(paths: &AppPaths, profile: &MinecraftProfile) -> AppResult<()> {
    let meta = ActiveAccountMeta {
        id: profile.id.clone(),
        name: profile.name.clone(),
    };
    std::fs::create_dir_all(paths.root())?;
    std::fs::write(paths.accounts_file(), serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

fn load_active_account(paths: &AppPaths) -> AppResult<Option<ActiveAccountMeta>> {
    let path = paths.accounts_file();
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    Ok(Some(serde_json::from_slice(&bytes)?))
}

/// The same deterministic UUID a vanilla server derives for an offline-mode
/// player: MD5 of `OfflinePlayer:<username>` (UTF-8), with the version (3)
/// and variant (RFC 4122) bits forced — Java's `UUID.nameUUIDFromBytes`.
/// Deterministic per username so saves/whitelists stay consistent across
/// launches, matching what any offline-mode server would compute itself.
fn offline_uuid(username: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize().into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).simple().to_string()
}

/// Builds a local, network-free session for [`GlobalSettings::offline_mode`].
/// Only valid on singleplayer or servers explicitly running in offline mode.
pub fn offline_session(username: &str) -> AppResult<AccountSession> {
    let name = username.trim();
    if name.is_empty() {
        return Err(AppError::Auth(
            "Renseigne un pseudo hors-ligne dans Paramètres avant de lancer le jeu.".to_string(),
        ));
    }
    if name.len() > 16 {
        return Err(AppError::Auth(
            "Le pseudo hors-ligne doit faire 16 caractères maximum.".to_string(),
        ));
    }

    Ok(AccountSession {
        profile: MinecraftProfile { id: offline_uuid(name), name: name.to_string() },
        minecraft_access_token: "-".to_string(),
        expires_at: i64::MAX,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_deterministic_and_matches_java_algorithm() {
        // Cross-checked against a standalone Python implementation of the
        // same bit-twiddling over MD5(b"OfflinePlayer:Player") — the exact
        // value every offline-mode Minecraft server derives for "Player".
        assert_eq!(offline_uuid("Player"), "a01e3843e5213998958af459800e4d11");
    }

    #[test]
    fn offline_uuid_differs_per_username_but_is_stable() {
        let a1 = offline_uuid("Alice");
        let a2 = offline_uuid("Alice");
        let b = offline_uuid("Bob");
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
    }

    #[test]
    fn offline_session_rejects_blank_or_too_long_username() {
        assert!(offline_session("").is_err());
        assert!(offline_session("   ").is_err());
        assert!(offline_session("ThisNameIsWayTooLong").is_err());
    }

    #[test]
    fn offline_session_trims_and_builds_legacy_session() {
        let session = offline_session("  Steve  ").unwrap();
        assert_eq!(session.profile.name, "Steve");
        assert_eq!(session.profile.id.len(), 32);
        assert_eq!(session.minecraft_access_token, "-");
    }
}
