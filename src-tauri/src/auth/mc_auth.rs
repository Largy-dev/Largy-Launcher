//! Final hop of the auth chain: exchange the XSTS token for a Minecraft
//! Services access token, then fetch the player profile (id + username).

use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppResult};

use super::MinecraftProfile;

pub async fn login_with_xbox(
    client: &reqwest::Client,
    uhs: &str,
    xsts_token: &str,
) -> AppResult<(String, i64)> {
    let body = json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") });

    let response: serde_json::Value = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AppError::Auth(format!("connexion Minecraft Services échouée: {e}")))?
        .json()
        .await?;

    let access_token = response
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Auth("réponse Minecraft Services invalide".to_string()))?
        .to_string();

    let expires_in = response.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(86400);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    Ok((access_token, now + expires_in))
}

pub async fn fetch_profile(client: &reqwest::Client, mc_access_token: &str) -> AppResult<MinecraftProfile> {
    let response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_access_token)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::Auth(
            "Ce compte Microsoft ne possède pas Minecraft: Java Edition.".to_string(),
        ));
    }

    let body: serde_json::Value = response
        .error_for_status()
        .map_err(|e| AppError::Auth(format!("récupération du profil échouée: {e}")))?
        .json()
        .await?;

    Ok(MinecraftProfile {
        id: body
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Auth("profil Minecraft invalide".to_string()))?
            .to_string(),
        name: body
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Auth("profil Minecraft invalide".to_string()))?
            .to_string(),
    })
}
