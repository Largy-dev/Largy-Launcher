//! Secure storage for the long-lived Microsoft refresh token, backed by the
//! Windows Credential Manager via the `keyring` crate. Non-secret account
//! metadata (profile id/name, active account pointer) is kept separately as
//! plain JSON under the app data dir — only the refresh token needs a vault.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.largylauncher.app";

pub struct TokenStore;

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
        let entry = keyring::Entry::new(SERVICE, account_key)
            .map_err(|e| AppError::TokenStore(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::TokenStore(e.to_string())),
        }
    }
}
