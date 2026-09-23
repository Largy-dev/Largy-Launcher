use serde::Serialize;

/// Unified application error. Every Tauri command returns `Result<T, AppError>`
/// so the frontend gets a stable, serializable error shape instead of ad-hoc strings.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("(de)serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("secure token storage error: {0}")]
    TokenStore(String),

    #[error("modpack provider error: {0}")]
    Provider(String),

    #[error("mod loader install error: {0}")]
    Loader(String),

    #[error("instance error: {0}")]
    Instance(String),

    #[error("java error: {0}")]
    Java(String),

    #[error("download error: {0}")]
    Download(String),

    #[error("launch error: {0}")]
    Launch(String),

    #[error("opération annulée")]
    Cancelled,

    /// A service answered 429: retrying immediately would only make it worse.
    #[error("{0}")]
    RateLimited(String),

    #[error("{0}")]
    Other(String),
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::Other(format!("zip error: {err}"))
    }
}

/// Serialized to the frontend as `{ "kind": "...", "message": "..." }` via
/// `invoke()` rejections, so React can branch on `kind` without string matching.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let kind = match self {
            AppError::Network(_) => "network",
            AppError::Io(_) => "io",
            AppError::Serde(_) => "serde",
            AppError::Auth(_) => "auth",
            AppError::TokenStore(_) => "token_store",
            AppError::Provider(_) => "provider",
            AppError::Loader(_) => "loader",
            AppError::Instance(_) => "instance",
            AppError::Java(_) => "java",
            AppError::Download(_) => "download",
            AppError::Launch(_) => "launch",
            AppError::Cancelled => "cancelled",
            AppError::RateLimited(_) => "rate_limited",
            AppError::Other(_) => "other",
        };
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("kind", kind)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_kind_and_message() {
        let err = AppError::Auth("échec de connexion".to_string());
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "auth");
        assert_eq!(value["message"], "authentication failed: échec de connexion");
    }

    #[test]
    fn every_variant_reports_its_own_kind() {
        assert_eq!(serde_json::to_value(AppError::Loader("x".into())).unwrap()["kind"], "loader");
        assert_eq!(serde_json::to_value(AppError::Launch("x".into())).unwrap()["kind"], "launch");
        assert_eq!(serde_json::to_value(AppError::Other("x".into())).unwrap()["kind"], "other");
    }
}
