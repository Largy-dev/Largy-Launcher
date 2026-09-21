//! Checks GitHub Releases for a version newer than the running build.
//! Informational only — no auto-download/install (see README's "à faire"
//! section); the user follows `release_url` to grab the new installer.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/Largy-dev/Largy-Launcher/releases/latest";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
}

/// Parses a `MAJOR.MINOR.PATCH` version string (an optional leading `v` is
/// stripped) into a tuple for ordering. Missing or non-numeric segments
/// parse as `0` rather than erroring, so a malformed remote tag never
/// breaks the check — it just compares as very old.
fn parse_version(raw: &str) -> (u64, u64, u64) {
    let raw = raw.trim();
    let raw = raw.strip_prefix('v').unwrap_or(raw);
    let mut parts = raw.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

pub async fn fetch_latest(client: &reqwest::Client) -> AppResult<UpdateCheck> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let release: GithubRelease = client
        .get(RELEASES_LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AppError::Other(format!("vérification de mise à jour échouée: {e}")))?
        .json()
        .await?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let update_available = parse_version(&latest_version) > parse_version(&current_version);

    Ok(UpdateCheck {
        current_version,
        latest_version,
        update_available,
        release_url: release.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_handles_leading_v_and_missing_segments() {
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("1.2"), (1, 2, 0));
        assert_eq!(parse_version("garbage"), (0, 0, 0));
    }

    #[test]
    fn parse_version_orders_correctly_including_double_digit_segments() {
        assert!(parse_version("0.10.0") > parse_version("0.9.0"));
        assert!(parse_version("1.0.0") > parse_version("0.99.99"));
    }

    #[test]
    fn parse_version_treats_equal_versions_as_equal() {
        assert_eq!(parse_version("v0.1.2"), parse_version("0.1.2"));
    }
}
