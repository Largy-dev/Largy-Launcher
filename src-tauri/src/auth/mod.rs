//! Microsoft → Xbox Live → XSTS → Minecraft Services auth chain, with
//! several remembered accounts (one active). Refresh tokens live in the OS
//! credential vault; only non-secret metadata (id, name, which is active)
//! is kept in `accounts.json`. The Minecraft access token itself never
//! leaves the backend.

pub mod mc_auth;
pub mod ms_oauth;
pub mod token_store;
pub mod xbox;

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::util::fs::write_atomic;
use ms_oauth::{DeviceCodeInfo, PollOutcome};
use token_store::{CachedMinecraftToken, TokenStore};

/// Turns an HTTP error status into an [`AppError`]: 429 becomes
/// [`AppError::RateLimited`] (callers fall back to a cached session).
pub(crate) fn check_status(response: reqwest::Response, what: &str) -> AppResult<reqwest::Response> {
    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AppError::RateLimited(format!(
            "{what} : trop de connexions en peu de temps, réessaie dans quelques minutes."
        )));
    }
    response.error_for_status().map_err(|e| AppError::Auth(format!("{what} échouée : {e}")))
}

/// Errors after which the cached session is still the best thing to use.
fn is_transient(err: &AppError) -> bool {
    matches!(err, AppError::Network(_) | AppError::RateLimited(_))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
}

/// Backend-only: holds the access token handed to the game.
#[derive(Clone)]
pub struct AccountSession {
    pub profile: MinecraftProfile,
    pub minecraft_access_token: String,
    pub expires_at: i64,
    pub xuid: Option<String>,
}

/// What the frontend gets to see of the active session.
#[derive(Debug, Clone, Serialize)]
pub struct AccountView {
    pub profile: MinecraftProfile,
    /// The session couldn't be verified online (no network at startup):
    /// singleplayer works, multiplayer servers will refuse it.
    pub offline: bool,
}

impl AccountSession {
    pub fn view(&self) -> AccountView {
        AccountView {
            profile: self.profile.clone(),
            offline: self.minecraft_access_token == CACHED_TOKEN || self.expires_at <= now_unix(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredAccount {
    pub id: String,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
struct AccountMeta {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AccountsFile {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    accounts: Vec<AccountMeta>,
}

impl AccountsFile {
    fn upsert(&mut self, profile: &MinecraftProfile) {
        let meta = AccountMeta { id: profile.id.clone(), name: profile.name.clone() };
        match self.accounts.iter_mut().find(|a| a.id == meta.id) {
            Some(existing) => *existing = meta,
            None => self.accounts.push(meta),
        }
        self.active = Some(profile.id.clone());
    }

    fn get(&self, id: &str) -> Option<&AccountMeta> {
        self.accounts.iter().find(|a| a.id == id)
    }
}

/// Placeholder token of a session restored without network.
const CACHED_TOKEN: &str = "-";

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
    Ok(AccountSession { profile, minecraft_access_token: mc_token, expires_at, xuid: xsts.xid })
}

/// Polls the token endpoint until the user finishes the browser step (or
/// `cancel` fires), then runs the rest of the chain and remembers the
/// account as the active one.
pub async fn complete_login(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
    device: &DeviceCodeInfo,
    cancel: &Notify,
) -> AppResult<AccountSession> {
    let deadline = Instant::now() + Duration::from_secs(device.expires_in);
    let mut interval = Duration::from_secs(device.interval.max(1));

    loop {
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = cancel.notified() => return Err(AppError::Cancelled),
        }
        if Instant::now() > deadline {
            return Err(AppError::Auth("Code expiré, réessaie.".to_string()));
        }

        match ms_oauth::poll_device_token(client, azure_client_id, &device.device_code).await? {
            PollOutcome::Pending => continue,
            // RFC 8628 §3.5: back off by 5 seconds on every `slow_down`.
            PollOutcome::SlowDown => interval += Duration::from_secs(5),
            PollOutcome::Expired => return Err(AppError::Auth("Code expiré, réessaie.".to_string())),
            PollOutcome::Success(tokens) => {
                let session = finish_chain(client, &tokens.access_token).await?;
                TokenStore::save_refresh_token(&session.profile.id, &tokens.refresh_token)?;
                remember_minecraft_token(&session);
                let mut file = load_accounts(paths)?;
                file.upsert(&session.profile);
                save_accounts(paths, &file)?;
                return Ok(session);
            }
        }
    }
}

/// Fresh session for a remembered account, from its stored refresh token.
pub async fn refresh_account(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
    account_id: &str,
) -> AppResult<AccountSession> {
    let refresh = TokenStore::load_refresh_token(account_id)?
        .ok_or_else(|| AppError::Auth("Session expirée — reconnecte ce compte.".to_string()))?;
    let tokens = ms_oauth::refresh_token(client, azure_client_id, &refresh).await?;
    TokenStore::save_refresh_token(account_id, &tokens.refresh_token)?;
    let session = finish_chain(client, &tokens.access_token).await?;
    remember_minecraft_token(&session);
    let mut file = load_accounts(paths)?;
    file.upsert(&session.profile);
    save_accounts(paths, &file)?;
    Ok(session)
}

fn remember_minecraft_token(session: &AccountSession) {
    let cached = CachedMinecraftToken {
        token: session.minecraft_access_token.clone(),
        expires_at: session.expires_at,
        xuid: session.xuid.clone(),
    };
    if let Err(e) = TokenStore::save_minecraft_token(&session.profile.id, &cached) {
        tracing::warn!("could not cache the Minecraft token: {e}");
    }
}

/// The session stored for `meta`: its cached Minecraft token when there is
/// one (possibly expired), else an offline placeholder.
fn cached_session(meta: &AccountMeta) -> AccountSession {
    let profile = MinecraftProfile { id: meta.id.clone(), name: meta.name.clone() };
    match TokenStore::load_minecraft_token(&meta.id) {
        Some(cached) => AccountSession {
            profile,
            minecraft_access_token: cached.token,
            expires_at: cached.expires_at,
            xuid: cached.xuid,
        },
        None => AccountSession { profile, minecraft_access_token: CACHED_TOKEN.to_string(), expires_at: 0, xuid: None },
    }
}

/// Uses the cached Minecraft token while it's valid (no network at all);
/// otherwise refreshes, falling back to the cached session when Microsoft
/// or Minecraft Services can't be reached or rate-limit us.
async fn login_as(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
    meta: &AccountMeta,
) -> AppResult<AccountSession> {
    let cached = cached_session(meta);
    if cached.minecraft_access_token != CACHED_TOKEN && !needs_refresh(&cached, now_unix()) {
        return Ok(cached);
    }
    match refresh_account(paths, client, azure_client_id, &meta.id).await {
        Err(e) if is_transient(&e) => {
            tracing::warn!("using cached session for {} ({e})", meta.name);
            Ok(cached)
        }
        other => other,
    }
}

/// Refreshes an account whose token is about to expire, keeping the current
/// session when the network is down or the service rate-limits us.
pub async fn refresh_or_keep(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
    current: AccountSession,
) -> AppResult<AccountSession> {
    match refresh_account(paths, client, azure_client_id, &current.profile.id).await {
        Err(e) if is_transient(&e) => {
            tracing::warn!("token refresh failed ({e}); launching with the cached session");
            Ok(current)
        }
        other => other,
    }
}

/// Called on startup: silently re-authenticates the active account, or
/// returns `None` if there isn't one.
pub async fn try_silent_login(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
) -> AppResult<Option<AccountSession>> {
    if azure_client_id.trim().is_empty() {
        return Ok(None);
    }
    let file = load_accounts(paths)?;
    let Some(meta) = file.active.as_deref().and_then(|id| file.get(id)).cloned() else {
        return Ok(None);
    };
    login_as(paths, client, azure_client_id, &meta).await.map(Some)
}

pub async fn switch_account(
    paths: &AppPaths,
    client: &reqwest::Client,
    azure_client_id: &str,
    account_id: &str,
) -> AppResult<AccountSession> {
    let meta = load_accounts(paths)?
        .get(account_id)
        .cloned()
        .ok_or_else(|| AppError::Auth("compte inconnu".to_string()))?;
    let session = login_as(paths, client, azure_client_id, &meta).await?;
    let mut file = load_accounts(paths)?;
    file.active = Some(account_id.to_string());
    save_accounts(paths, &file)?;
    Ok(session)
}

pub fn list_accounts(paths: &AppPaths) -> AppResult<Vec<StoredAccount>> {
    let file = load_accounts(paths)?;
    Ok(file
        .accounts
        .iter()
        .map(|a| StoredAccount { id: a.id.clone(), name: a.name.clone(), active: file.active.as_deref() == Some(&a.id) })
        .collect())
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

/// True when `session`'s Minecraft access token has already expired or will
/// within the next few minutes — a stale token makes the game refuse the
/// session with "invalid session, restart the game/launcher".
pub fn needs_refresh(session: &AccountSession, now: i64) -> bool {
    const REFRESH_BUFFER_SECS: i64 = 300;
    session.expires_at - now <= REFRESH_BUFFER_SECS
}

/// Forgets `account_id` (default: the active account). Returns whether it
/// was the active one.
pub fn logout(paths: &AppPaths, account_id: Option<&str>) -> AppResult<bool> {
    let mut file = load_accounts(paths)?;
    let Some(id) = account_id.map(str::to_string).or_else(|| file.active.clone()) else {
        return Ok(false);
    };
    TokenStore::delete_refresh_token(&id)?;
    file.accounts.retain(|a| a.id != id);
    let was_active = file.active.as_deref() == Some(&id);
    if was_active {
        file.active = None;
    }
    save_accounts(paths, &file)?;
    Ok(was_active)
}

fn save_accounts(paths: &AppPaths, file: &AccountsFile) -> AppResult<()> {
    write_atomic(&paths.accounts_file(), serde_json::to_string_pretty(file)?.as_bytes())?;
    Ok(())
}

/// Reads `accounts.json`, migrating the single-account format
/// (`{"id","name"}`) of earlier versions.
fn load_accounts(paths: &AppPaths) -> AppResult<AccountsFile> {
    let path = paths.accounts_file();
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(AccountsFile::default());
    };
    if let Ok(file) = serde_json::from_slice::<AccountsFile>(&bytes) {
        if file.active.is_some() || !file.accounts.is_empty() {
            return Ok(file);
        }
    }
    match serde_json::from_slice::<AccountMeta>(&bytes) {
        Ok(legacy) if !legacy.id.is_empty() => {
            Ok(AccountsFile { active: Some(legacy.id.clone()), accounts: vec![legacy] })
        }
        _ => Ok(AccountsFile::default()),
    }
}

/// The same deterministic UUID a vanilla server derives for an offline-mode
/// player: MD5 of `OfflinePlayer:<username>` with version 3 / RFC 4122 bits —
/// Java's `UUID.nameUUIDFromBytes`.
fn offline_uuid(username: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize().into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).simple().to_string()
}

/// At most 16 characters and no whitespace — what the game accepts.
fn valid_username(name: &str) -> bool {
    (1..=16).contains(&name.chars().count()) && !name.chars().any(char::is_whitespace)
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
    if !valid_username(name) {
        return Err(AppError::Auth(
            "Le pseudo hors-ligne doit faire 16 caractères maximum, sans espace.".to_string(),
        ));
    }

    Ok(AccountSession {
        profile: MinecraftProfile { id: offline_uuid(name), name: name.to_string() },
        minecraft_access_token: CACHED_TOKEN.to_string(),
        expires_at: i64::MAX,
        xuid: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_deterministic_and_matches_java_algorithm() {
        assert_eq!(offline_uuid("Player"), "a01e3843e5213998958af459800e4d11");
    }

    #[test]
    fn offline_uuid_differs_per_username_but_is_stable() {
        assert_eq!(offline_uuid("Alice"), offline_uuid("Alice"));
        assert_ne!(offline_uuid("Alice"), offline_uuid("Bob"));
    }

    #[test]
    fn offline_session_rejects_invalid_usernames() {
        for bad in ["", "   ", "ThisNameIsWayTooLong", "with space"] {
            assert!(offline_session(bad).is_err(), "{bad:?}");
        }
    }

    fn fixture_session(expires_at: i64) -> AccountSession {
        AccountSession {
            profile: MinecraftProfile { id: "uuid".to_string(), name: "Steve".to_string() },
            minecraft_access_token: "token".to_string(),
            expires_at,
            xuid: None,
        }
    }

    #[test]
    fn needs_refresh_follows_the_buffer_window() {
        assert!(!needs_refresh(&fixture_session(10_000), 1_000));
        assert!(needs_refresh(&fixture_session(1_200), 1_000));
        assert!(needs_refresh(&fixture_session(500), 1_000));
        assert!(!needs_refresh(&offline_session("Steve").unwrap(), now_unix()));
    }

    #[test]
    fn offline_session_trims_and_builds_legacy_session() {
        let session = offline_session("  Steve  ").unwrap();
        assert_eq!(session.profile.name, "Steve");
        assert_eq!(session.profile.id.len(), 32);
        assert_eq!(session.minecraft_access_token, "-");
    }

    #[test]
    fn legacy_single_account_file_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        std::fs::write(paths.accounts_file(), r#"{"id":"abc","name":"Steve"}"#).unwrap();

        let file = load_accounts(&paths).unwrap();
        assert_eq!(file.active.as_deref(), Some("abc"));
        assert_eq!(file.accounts, vec![AccountMeta { id: "abc".into(), name: "Steve".into() }]);
    }

    #[test]
    fn upsert_adds_or_renames_and_activates() {
        let mut file = AccountsFile::default();
        file.upsert(&MinecraftProfile { id: "a".into(), name: "Old".into() });
        file.upsert(&MinecraftProfile { id: "b".into(), name: "Bob".into() });
        file.upsert(&MinecraftProfile { id: "a".into(), name: "New".into() });
        assert_eq!(file.accounts.len(), 2);
        assert_eq!(file.get("a").unwrap().name, "New");
        assert_eq!(file.active.as_deref(), Some("a"));
    }

    #[test]
    fn cached_session_is_reported_as_offline() {
        let session = cached_session(&AccountMeta { id: "a".into(), name: "Steve".into() });
        assert!(session.view().offline);
        assert!(!fixture_session(now_unix() + 3600).view().offline);
        assert!(fixture_session(0).view().offline, "an expired token is offline-only");
    }
}
