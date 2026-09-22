//! Final hop of the auth chain: exchange the XSTS token for a Minecraft
//! Services access token, then fetch the player profile (id + username).

use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppResult};

use super::MinecraftProfile;

/// Extracts the access token + absolute expiry timestamp from a
/// `login_with_xbox` response body. Split out from [`login_with_xbox`] so the
/// missing-field and `expires_in`-defaulting logic is testable without a
/// network round-trip.
fn parse_login_response(response: &serde_json::Value, now: i64) -> AppResult<(String, i64)> {
    let access_token = response
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Auth("réponse Minecraft Services invalide".to_string()))?
        .to_string();

    let expires_in = response.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(86400);

    Ok((access_token, now + expires_in))
}

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

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    parse_login_response(&response, now)
}

/// Extracts the player profile from a `/minecraft/profile` response body.
/// Split out from [`fetch_profile`] so missing-field validation is testable
/// without a network round-trip.
fn parse_profile_response(body: &serde_json::Value) -> AppResult<MinecraftProfile> {
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

    parse_profile_response(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_login_response_extracts_token_and_computes_absolute_expiry() {
        let response = json!({ "access_token": "abc123", "expires_in": 3600 });
        let (token, expires_at) = parse_login_response(&response, 1_000_000).unwrap();
        assert_eq!(token, "abc123");
        assert_eq!(expires_at, 1_003_600);
    }

    #[test]
    fn parse_login_response_defaults_expires_in_when_absent() {
        let response = json!({ "access_token": "abc123" });
        let (_, expires_at) = parse_login_response(&response, 1_000_000).unwrap();
        assert_eq!(expires_at, 1_000_000 + 86400);
    }

    #[test]
    fn parse_login_response_errors_when_access_token_missing() {
        let response = json!({ "expires_in": 3600 });
        assert!(matches!(parse_login_response(&response, 0), Err(AppError::Auth(_))));
    }

    #[test]
    fn parse_profile_response_extracts_id_and_name() {
        let body = json!({ "id": "uuid-1", "name": "Steve" });
        let profile = parse_profile_response(&body).unwrap();
        assert_eq!(profile.id, "uuid-1");
        assert_eq!(profile.name, "Steve");
    }

    #[test]
    fn parse_profile_response_errors_when_name_missing() {
        let body = json!({ "id": "uuid-1" });
        assert!(matches!(parse_profile_response(&body), Err(AppError::Auth(_))));
    }

    #[test]
    fn parse_profile_response_errors_when_id_missing() {
        let body = json!({ "name": "Steve" });
        assert!(matches!(parse_profile_response(&body), Err(AppError::Auth(_))));
    }
}
