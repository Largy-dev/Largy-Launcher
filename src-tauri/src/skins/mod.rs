//! Skin and cape of the active Microsoft account, through Minecraft
//! Services (`/minecraft/profile`), plus a local library of skins to switch
//! between ([`library`]). Textures reach the webview as data URLs so the 3D
//! preview never depends on the texture server's CORS headers.

pub mod library;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::auth::check_status;
use crate::error::{AppError, AppResult};

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const TEXTURE_HOST: &str = "textures.minecraft.net";
/// Mojang refuses bigger uploads anyway; keeps a wrong file from being read whole.
const MAX_SKIN_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinVariant {
    Classic,
    Slim,
}

impl SkinVariant {
    fn from_api(value: &str) -> Self {
        if value.eq_ignore_ascii_case("slim") {
            SkinVariant::Slim
        } else {
            SkinVariant::Classic
        }
    }

    fn api_name(self) -> &'static str {
        match self {
            SkinVariant::Classic => "classic",
            SkinVariant::Slim => "slim",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CapeView {
    pub id: String,
    pub alias: String,
    pub active: bool,
    pub texture: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkinProfile {
    pub name: String,
    pub variant: SkinVariant,
    /// Data URL of the active skin; `None` for the default skin.
    pub skin: Option<String>,
    pub capes: Vec<CapeView>,
}

#[derive(Deserialize)]
struct ApiSkin {
    #[serde(default)]
    state: String,
    url: String,
    #[serde(default)]
    variant: String,
}

#[derive(Deserialize)]
struct ApiCape {
    id: String,
    #[serde(default)]
    state: String,
    url: String,
    #[serde(default)]
    alias: String,
}

#[derive(Deserialize)]
struct ApiProfile {
    name: String,
    #[serde(default)]
    skins: Vec<ApiSkin>,
    #[serde(default)]
    capes: Vec<ApiCape>,
}

pub fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

/// Width and height of a PNG, read from its IHDR chunk.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || !bytes.starts_with(SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let read = |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Some((read(16), read(20)))
}

/// A skin file the game accepts: a 64×64 (or legacy 64×32) PNG.
pub fn validate_skin(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() > MAX_SKIN_BYTES {
        return Err(AppError::Other("ce fichier est trop lourd pour être un skin".to_string()));
    }
    match png_size(bytes) {
        Some((64, 64 | 32)) => Ok(()),
        Some((w, h)) => Err(AppError::Other(format!("un skin fait 64×64 pixels (celui-ci fait {w}×{h})"))),
        None => Err(AppError::Other("un skin doit être une image PNG".to_string())),
    }
}

async fn texture(client: &reqwest::Client, url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.host_str() != Some(TEXTURE_HOST) {
        return None;
    }
    let secure = format!("https://{TEXTURE_HOST}{}", parsed.path());
    let bytes = client.get(secure).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    png_size(&bytes).is_some().then(|| data_url(&bytes))
}

pub async fn get_profile(client: &reqwest::Client, token: &str) -> AppResult<SkinProfile> {
    let response = client.get(PROFILE_URL).bearer_auth(token).send().await?;
    let profile: ApiProfile = check_status(response, "Lecture du profil")?.json().await?;

    let active_skin = profile.skins.iter().find(|s| s.state.eq_ignore_ascii_case("active"));
    let skin = match active_skin {
        Some(s) => texture(client, &s.url).await,
        None => None,
    };
    let capes = futures_util::future::join_all(profile.capes.iter().map(|cape| async {
        CapeView {
            id: cape.id.clone(),
            alias: cape.alias.clone(),
            active: cape.state.eq_ignore_ascii_case("active"),
            texture: texture(client, &cape.url).await,
        }
    }))
    .await;

    Ok(SkinProfile {
        name: profile.name,
        variant: active_skin.map_or(SkinVariant::Classic, |s| SkinVariant::from_api(&s.variant)),
        skin,
        capes,
    })
}

/// Uploads `png` as the account's skin.
pub async fn upload_skin(
    client: &reqwest::Client,
    token: &str,
    png: Vec<u8>,
    variant: SkinVariant,
) -> AppResult<SkinProfile> {
    validate_skin(&png)?;
    let part = reqwest::multipart::Part::bytes(png).file_name("skin.png").mime_str("image/png")?;
    let form = reqwest::multipart::Form::new().text("variant", variant.api_name()).part("file", part);
    let response = client.post(format!("{PROFILE_URL}/skins")).bearer_auth(token).multipart(form).send().await?;
    check_status(response, "Mise à jour du skin")?;
    get_profile(client, token).await
}

/// Back to the default skin (Steve/Alex…).
pub async fn reset_skin(client: &reqwest::Client, token: &str) -> AppResult<SkinProfile> {
    let response = client.delete(format!("{PROFILE_URL}/skins/active")).bearer_auth(token).send().await?;
    check_status(response, "Réinitialisation du skin")?;
    get_profile(client, token).await
}

/// Shows the cape `cape_id`, or hides every cape with `None`.
pub async fn set_cape(client: &reqwest::Client, token: &str, cape_id: Option<&str>) -> AppResult<SkinProfile> {
    let url = format!("{PROFILE_URL}/capes/active");
    let request = match cape_id {
        Some(id) => client.put(url).json(&serde_json::json!({ "capeId": id })),
        None => client.delete(url),
    };
    check_status(request.bearer_auth(token).send().await?, "Mise à jour de la cape")?;
    get_profile(client, token).await
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn fake_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn reads_png_dimensions() {
        assert_eq!(png_size(&fake_png(64, 32)), Some((64, 32)));
        assert_eq!(png_size(b"GIF89a....................."), None);
    }

    #[test]
    fn accepts_only_skin_sized_pngs() {
        assert!(validate_skin(&fake_png(64, 64)).is_ok());
        assert!(validate_skin(&fake_png(64, 32)).is_ok());
        assert!(validate_skin(&fake_png(128, 128)).is_err());
        assert!(validate_skin(b"not a png").is_err());
    }

    #[test]
    fn variant_names_follow_the_api() {
        assert_eq!(SkinVariant::from_api("SLIM"), SkinVariant::Slim);
        assert_eq!(SkinVariant::from_api("CLASSIC"), SkinVariant::Classic);
        assert_eq!(SkinVariant::Slim.api_name(), "slim");
    }
}
