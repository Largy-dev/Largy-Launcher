//! Xbox Live user token + XSTS authorization — the two hops between a
//! Microsoft OAuth access token and something `api.minecraftservices.com`
//! will accept.

use serde_json::json;

use crate::error::{AppError, AppResult};

pub struct XblToken {
    pub token: String,
    pub uhs: String,
}

pub async fn authenticate_xbl(client: &reqwest::Client, ms_access_token: &str) -> AppResult<XblToken> {
    let body = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    });

    let response: serde_json::Value = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AppError::Auth(format!("authentification Xbox Live échouée: {e}")))?
        .json()
        .await?;

    parse_token_response(&response)
}

pub async fn authorize_xsts(client: &reqwest::Client, xbl_token: &str) -> AppResult<XblToken> {
    let body = json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token],
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });

    let response = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let json_body: serde_json::Value = response.json().await?;

    if !status.is_success() {
        let message = match json_body.get("XErr").and_then(|v| v.as_u64()) {
            Some(2148916233) => {
                "Ce compte Microsoft n'a pas de compte Xbox associé. Crée-en un sur xbox.com puis réessaie."
            }
            Some(2148916235) => "Xbox Live n'est pas disponible dans ta région.",
            Some(2148916236) | Some(2148916237) => {
                "Ce compte Xbox nécessite une vérification adulte (Xbox pour adultes)."
            }
            Some(2148916238) => {
                "Ce compte est un compte enfant : ajoute-le à une famille Microsoft avant de te connecter."
            }
            _ => "Autorisation Xbox Live (XSTS) échouée.",
        };
        return Err(AppError::Auth(message.to_string()));
    }

    parse_token_response(&json_body)
}

fn parse_token_response(body: &serde_json::Value) -> AppResult<XblToken> {
    let token = body
        .get("Token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Auth("réponse Xbox Live invalide".to_string()))?
        .to_string();

    let uhs = body
        .get("DisplayClaims")
        .and_then(|v| v.get("xui"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("uhs"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Auth("réponse Xbox Live invalide (uhs manquant)".to_string()))?
        .to_string();

    Ok(XblToken { token, uhs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_token_response_extracts_token_and_uhs() {
        let body = json!({
            "Token": "abc123",
            "DisplayClaims": { "xui": [{ "uhs": "deadbeef" }] }
        });
        let token = parse_token_response(&body).unwrap();
        assert_eq!(token.token, "abc123");
        assert_eq!(token.uhs, "deadbeef");
    }

    #[test]
    fn parse_token_response_errors_when_uhs_missing() {
        let body = json!({ "Token": "abc123", "DisplayClaims": { "xui": [] } });
        assert!(parse_token_response(&body).is_err());
    }

    #[test]
    fn parse_token_response_errors_when_token_missing() {
        let body = json!({ "DisplayClaims": { "xui": [{ "uhs": "deadbeef" }] } });
        assert!(parse_token_response(&body).is_err());
    }
}
