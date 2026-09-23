//! Secure storage for the long-lived Microsoft refresh token, backed by the
//! Windows Credential Manager via the `keyring` crate. Non-secret account
//! metadata (profile id/name, active account pointer) is kept separately as
//! plain JSON under the app data dir — only the refresh token needs a vault.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.largylauncher.app";

pub struct TokenStore;

/// A Minecraft access token (valid ~24 h), cached so a launcher restart
/// doesn't redo the whole Microsoft → Xbox → Minecraft chain — Minecraft
/// Services rate-limits `login_with_xbox` aggressively.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CachedMinecraftToken {
    pub token: String,
    pub expires_at: i64,
    #[serde(default)]
    pub xuid: Option<String>,
}

fn minecraft_key(account_key: &str) -> String {
    format!("{account_key}:minecraft")
}

impl TokenStore {
    /// `account_key` is the Microsoft account's stable id (e.g. the `oid` claim),
    /// so multiple accounts can each have their own stored refresh token.
    pub fn save_refresh_token(account_key: &str, refresh_token: &str) -> AppResult<()> {
        let entry = keyring::Entry::new(SERVICE, account_key)
            .map_err(|e| AppError::TokenStore(e.to_string()))?;
        entry
            .set_password(refresh_token)
            .map_err(|e| AppError::TokenStore(e.to_string()))
    }

    pub fn load_refresh_token(account_key: &str) -> AppResult<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, account_key)
            .map_err(|e| AppError::TokenStore(e.to_string()))?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::TokenStore(e.to_string())),
        }
    }

    pub fn delete_refresh_token(account_key: &str) -> AppResult<()> {
        for key in [account_key.to_string(), minecraft_key(account_key)] {
            let entry = keyring::Entry::new(SERVICE, &key).map_err(|e| AppError::TokenStore(e.to_string()))?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(AppError::TokenStore(e.to_string())),
            }
        }
        Ok(())
    }

    pub fn save_minecraft_token(account_key: &str, cached: &CachedMinecraftToken) -> AppResult<()> {
        let json = serde_json::to_string(cached)?;
        Self::save_refresh_token(&minecraft_key(account_key), &json)
    }

    pub fn load_minecraft_token(account_key: &str) -> Option<CachedMinecraftToken> {
        let json = Self::load_refresh_token(&minecraft_key(account_key)).ok()??;
        serde_json::from_str(&json).ok()
    }
}
