//! Signing in inside a launcher window (authorization code + PKCE): the
//! Microsoft page opens in a small window and the launcher picks the code up
//! from the redirect — no code to copy by hand. It needs the app's Azure
//! registration to allow [`REDIRECT_URI`] ("Mobile and desktop
//! applications" platform); [`is_available`] checks that first so the
//! launcher falls back to the device-code flow otherwise.

use base64::Engine;
use sha2::{Digest, Sha256};

use super::ms_oauth::MsTokens;
use crate::error::{AppError, AppResult};

pub const REDIRECT_URI: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";
const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";

/// PKCE verifier and its S256 challenge.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Self {
        let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Pkce { verifier, challenge }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn authorize_url(client_id: &str, pkce: &Pkce, state: &str) -> String {
    format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}\
         &code_challenge_method=S256&state={}&prompt=select_account",
        encode(client_id),
        encode(REDIRECT_URI),
        encode(SCOPE),
        pkce.challenge,
        encode(state)
    )
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                        continue;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `url` is the redirect this flow waits for.
pub fn is_redirect(url: &str) -> bool {
    url.starts_with(REDIRECT_URI)
}

/// The authorization code from the redirect URL, checking `state`.
pub fn code_from_redirect(url: &str, expected_state: &str) -> AppResult<String> {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let query = query.split('#').next().unwrap_or_default();
    let param = |name: &str| {
        query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == name).then(|| decode(v))
        })
    };
    if let Some(error) = param("error") {
        if error == "access_denied" {
            return Err(AppError::Cancelled);
        }
        return Err(AppError::Auth(param("error_description").unwrap_or(error)));
    }
    if param("state").as_deref() != Some(expected_state) {
        return Err(AppError::Auth("réponse de connexion inattendue, réessaie".to_string()));
    }
    param("code").filter(|c| !c.is_empty()).ok_or_else(|| AppError::Auth("code de connexion absent".to_string()))
}

/// Asks Microsoft whether this app accepts [`REDIRECT_URI`]: an unregistered
/// redirect is refused before any page the player would see.
pub async fn is_available(client: &reqwest::Client, client_id: &str) -> bool {
    let url = authorize_url(client_id, &Pkce::new(), "probe");
    match client.get(url).send().await {
        Ok(response) => match response.text().await {
            Ok(body) => !body.contains("AADSTS50011") && !body.contains("redirect_uri' is not valid")
                && !body.contains("redirect_uri&#39; is not valid"),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

pub async fn exchange_code(client: &reqwest::Client, client_id: &str, code: &str, pkce: &Pkce) -> AppResult<MsTokens> {
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", pkce.verifier.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    let field = |key: &str| body.get(key).and_then(|v| v.as_str()).map(str::to_string);
    if !status.is_success() {
        return Err(AppError::Auth(field("error_description").unwrap_or_else(|| "échec de la connexion Microsoft".into())));
    }
    match (field("access_token"), field("refresh_token")) {
        (Some(access_token), Some(refresh_token)) => Ok(MsTokens { access_token, refresh_token }),
        _ => Err(AppError::Auth("réponse Microsoft invalide".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_the_s256_of_the_verifier() {
        let pkce = Pkce::new();
        assert!((43..=128).contains(&pkce.verifier.len()));
        let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier.as_bytes()));
        assert_eq!(pkce.challenge, expected);
        assert_ne!(Pkce::new().verifier, pkce.verifier);
    }

    #[test]
    fn authorize_url_carries_every_parameter_encoded() {
        let pkce = Pkce { verifier: "v".into(), challenge: "abc".into() };
        let url = authorize_url("id-1", &pkce, "s t");
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("redirect_uri=https%3A%2F%2Flogin.microsoftonline.com%2Fcommon%2Foauth2%2Fnativeclient"));
        assert!(url.contains("scope=XboxLive.signin%20offline_access"));
        assert!(url.contains("code_challenge=abc&code_challenge_method=S256"));
        assert!(url.contains("state=s%20t"));
    }

    #[test]
    fn the_code_is_read_from_the_redirect_and_the_state_checked() {
        let ok = format!("{REDIRECT_URI}?code=M.C1_x%2Fy&state=abc");
        assert_eq!(code_from_redirect(&ok, "abc").unwrap(), "M.C1_x/y");
        assert!(code_from_redirect(&ok, "other").is_err());
        let denied = format!("{REDIRECT_URI}?error=access_denied&state=abc");
        assert!(matches!(code_from_redirect(&denied, "abc"), Err(AppError::Cancelled)));
        let failed = format!("{REDIRECT_URI}?error=server_error&error_description=Oups+!&state=abc");
        assert!(matches!(code_from_redirect(&failed, "abc"), Err(AppError::Auth(m)) if m == "Oups !"));
        assert!(is_redirect(&ok) && !is_redirect("https://login.live.com/"));
    }
}
