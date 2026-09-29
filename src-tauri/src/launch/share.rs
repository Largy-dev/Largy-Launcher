//! "Partager le log": uploads a game log, crash report or the launcher's
//! own log to mclo.gs — the paste site the Minecraft community reads — and
//! returns the link, so asking for help on Discord is one click. Personal
//! details are scrubbed first: the Windows user folder (which carries the
//! account name) and anything shaped like an access token.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const API: &str = "https://api.mclo.gs/1/log";
/// mclo.gs keeps at most 25 000 lines / 10 MiB; the end of a log is what matters.
const MAX_LINES: usize = 25_000;
const MAX_BYTES: usize = 9 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LogSource {
    /// The instance's `logs/latest.log`.
    Game,
    /// A crash report (the given one, else the instance's newest).
    Crash,
    /// Largy Launcher's own log.
    Launcher,
}

#[derive(Deserialize)]
struct UploadResponse {
    success: bool,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// The last `MAX_LINES` lines, within `MAX_BYTES`.
pub fn tail(text: &str) -> &str {
    let mut start = 0;
    if let Some((index, _)) = text.rmatch_indices('\n').nth(MAX_LINES) {
        start = index + 1;
    }
    if text.len() - start > MAX_BYTES {
        start = text.len() - MAX_BYTES;
        while !text.is_char_boundary(start) {
            start += 1;
        }
    }
    &text[start..]
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
}

/// Replaces JWT-looking runs (`eyJ…`, 100+ chars) with `<jeton masqué>`.
fn scrub_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("eyJ") {
        out.push_str(&rest[..pos]);
        let candidate = &rest[pos..];
        let len = candidate.find(|c: char| !is_token_char(c)).unwrap_or(candidate.len());
        if len >= 100 && candidate[..len].matches('.').count() >= 2 {
            out.push_str("<jeton masqué>");
        } else {
            out.push_str(&candidate[..len]);
        }
        rest = &candidate[len..];
    }
    out.push_str(rest);
    out
}

/// Scrubs the user's home folder (both slash styles) and access tokens.
pub fn sanitize(text: &str, home: Option<&Path>) -> String {
    let mut text = scrub_tokens(text);
    if let Some(home) = home.map(|h| h.display().to_string()).filter(|h| h.len() > 3) {
        let replacement = if home.contains('\\') { "C:\\Users\\<joueur>" } else { "/home/<joueur>" };
        for variant in [home.clone(), home.replace('\\', "/"), home.replace('\\', "\\\\")] {
            text = text.replace(&variant, replacement);
        }
    }
    text
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Newest crash report of an instance.
pub fn newest_crash_report(instance_dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(instance_dir.join("crash-reports"))
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// Uploads `path`'s content (scrubbed, tail-trimmed); returns the mclo.gs link.
pub async fn upload_file(client: &reqwest::Client, path: &Path) -> AppResult<String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| AppError::Other("aucun log à partager pour l'instant".to_string()))?;
    let text = String::from_utf8_lossy(&bytes);
    let content = sanitize(tail(&text), home_dir().as_deref());
    if content.trim().is_empty() {
        return Err(AppError::Other("le log est vide".to_string()));
    }
    let response: UploadResponse =
        client.post(API).form(&[("content", content.as_str())]).send().await?.error_for_status()?.json().await?;
    match (response.success, response.url) {
        (true, Some(url)) => Ok(url),
        _ => Err(AppError::Other(format!(
            "mclo.gs a refusé le log : {}",
            response.error.unwrap_or_else(|| "raison inconnue".to_string())
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_folder_and_tokens_are_scrubbed() {
        let token = format!("eyJ{}.{}.{}", "a".repeat(60), "b".repeat(40), "c".repeat(20));
        let log = format!(
            "Loading C:\\Users\\larar\\AppData\\x.jar\nat C:/Users/larar/mods\n--accessToken {token} rest\neyJshort stays"
        );
        let clean = sanitize(&log, Some(Path::new("C:\\Users\\larar")));
        assert!(!clean.contains("larar"), "{clean}");
        assert!(clean.contains("C:\\Users\\<joueur>\\AppData"));
        assert!(clean.contains("--accessToken <jeton masqué> rest"));
        assert!(clean.contains("eyJshort stays"));
    }

    #[test]
    fn only_the_end_of_huge_logs_is_kept() {
        let log: String = (0..30_000).map(|i| format!("line {i}\n")).collect();
        let kept = tail(&log);
        assert_eq!(kept.lines().count(), MAX_LINES);
        assert!(kept.ends_with("line 29999\n"));
        assert_eq!(tail("short"), "short");
    }

    #[test]
    fn the_newest_crash_report_is_picked() {
        let dir = tempfile::tempdir().unwrap();
        assert!(newest_crash_report(dir.path()).is_none());
        let reports = dir.path().join("crash-reports");
        std::fs::create_dir_all(&reports).unwrap();
        std::fs::write(reports.join("crash-old.txt"), "a").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(reports.join("crash-new.txt"), "b").unwrap();
        std::fs::write(reports.join("notes.md"), "c").unwrap();
        assert!(newest_crash_report(dir.path()).unwrap().ends_with("crash-new.txt"));
    }
}
