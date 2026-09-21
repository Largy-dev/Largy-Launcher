//! Microsoft identity platform device-code flow (the "enter this code at
//! microsoft.com/link" screen) against the `consumers` tenant, so this never
//! needs a client secret — only a public-client Azure AD app registration
//! with "Allow public client flows" enabled.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: String,
}

pub enum PollOutcome {
    Success(MsTokens),
    Pending,
    Expired,
}

pub async fn request_device_code(client: &reqwest::Client, client_id: &str) -> AppResult<DeviceCodeInfo> {
    let body: serde_json::Value = client
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", client_id), ("scope", SCOPE)])
        .send()
        .await?
        .json()
        .await?;

    Ok(DeviceCodeInfo {
        device_code: field_str(&body, "device_code")?,
        user_code: field_str(&body, "user_code")?,
        verification_uri: body
            .get("verification_uri")
            .or_else(|| body.get("verification_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("https://microsoft.com/link")
            .to_string(),
        expires_in: body.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(900),
        interval: body.get("interval").and_then(|v| v.as_u64()).unwrap_or(5),
    })
}

pub async fn poll_device_token(
    client: &reqwest::Client,
    client_id: &str,
    device_code: &str,
) -> AppResult<PollOutcome> {
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", device_code),
        ])
        .send()
        .await?;

    let status = response.status();
    let body: serde_json::Value = response.json().await?;

    if status.is_success() {
        return Ok(PollOutcome::Success(MsTokens {
            access_token: field_str(&body, "access_token")?,
            refresh_token: field_str(&body, "refresh_token")?,
        }));
    }

    match body.get("error").and_then(|v| v.as_str()) {
        Some("authorization_pending") | Some("slow_down") => Ok(PollOutcome::Pending),
        Some("expired_token") | Some("code_expired") => Ok(PollOutcome::Expired),
        _ => Err(AppError::Auth(
            body.get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("échec de la connexion Microsoft")
                .to_string(),
        )),
    }
}

pub async fn refresh_token(
    client: &reqwest::Client,
    client_id: &str,
    refresh_token: &str,
) -> AppResult<MsTokens> {
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await?;

    let status = response.status();
    let body: serde_json::Value = response.json().await?;

    if !status.is_success() {
        return Err(AppError::Auth(
            body.get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("session expirée, reconnecte-toi")
                .to_string(),
        ));
    }

    Ok(MsTokens {
        access_token: field_str(&body, "access_token")?,
        refresh_token: field_str(&body, "refresh_token")?,
    })
}

fn field_str(body: &serde_json::Value, key: &str) -> AppResult<String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::Auth(format!("réponse Microsoft invalide (champ {key} manquant)")))
}
