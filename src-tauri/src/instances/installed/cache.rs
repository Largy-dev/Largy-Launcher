//! What the launcher learned about each content file, kept across runs in
//! `cache/content-meta.json` so opening the mods list of a 300-mod pack only
//! costs a directory listing: metadata, hashes and the extracted icon are
//! computed once per file (keyed by name, size and modification time), and
//! the Modrinth / CurseForge project it belongs to is looked up at most
//! every few days.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::metadata::LocalMeta;
use crate::paths::AppPaths;
use crate::util::fs::write_atomic;

/// Bumped when [`LocalMeta`] parsing changes, so every file is read again.
const SCHEMA: u32 = 3;
/// Entries of files not seen for this long are forgotten.
const FORGET_AFTER_SECS: i64 = 90 * 24 * 3600;

/// Serialises load-merge-save cycles between concurrent listings.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(Default::default);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RemoteProvider {
    Modrinth,
    Curseforge,
}

/// The catalogue project an installed file comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoteProject {
    pub provider: RemoteProvider,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    /// Project page on the provider's website.
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheEntry {
    pub meta: LocalMeta,
    pub sha1: Option<String>,
    /// CurseForge fingerprint (see `providers::curseforge::fingerprint`).
    pub fingerprint: Option<u32>,
    /// File name of the extracted icon in [`icons_dir`].
    pub icon: Option<String>,
    pub remote: Option<RemoteProject>,
    /// Unix seconds of the last Modrinth / CurseForge lookup (0 = never).
    pub remote_checked_at: i64,
    pub last_seen: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    schema: u32,
    entries: HashMap<String, CacheEntry>,
}

fn cache_file(paths: &AppPaths) -> PathBuf {
    paths.cache_dir().join("content-meta.json")
}

pub fn icons_dir(paths: &AppPaths) -> PathBuf {
    paths.cache_dir().join("content-icons")
}

/// Identity of a file's current bytes without reading them: renaming it to
/// `.disabled` and back keeps the key, replacing it changes size or mtime.
pub fn key(enabled_name: &str, size: u64, modified: i64) -> String {
    format!("{size}:{modified}:{enabled_name}")
}

fn load(path: &Path) -> CacheFile {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<CacheFile>(&bytes).ok())
        .filter(|file| file.schema == SCHEMA)
        .unwrap_or_else(|| CacheFile { schema: SCHEMA, entries: HashMap::new() })
}

/// Cached entries for `keys` (those already known).
pub fn get_many(paths: &AppPaths, keys: &[String]) -> HashMap<String, CacheEntry> {
    let _guard = LOCK.lock();
    let file = load(&cache_file(paths));
    keys.iter().filter_map(|k| file.entries.get(k).map(|e| (k.clone(), e.clone()))).collect()
}

/// Applies `change` to the cache and saves it, dropping long-unseen entries.
pub fn update(paths: &AppPaths, change: impl FnOnce(&mut HashMap<String, CacheEntry>)) {
    let _guard = LOCK.lock();
    let path = cache_file(paths);
    let mut file = load(&path);
    change(&mut file.entries);
    let now = crate::auth::now_unix();
    file.entries.retain(|_, e| now - e.last_seen < FORGET_AFTER_SECS);
    match serde_json::to_vec(&file) {
        Ok(bytes) => {
            if let Err(e) = write_atomic(&path, &bytes) {
                tracing::warn!("content metadata cache not saved: {e}");
            }
        }
        Err(e) => tracing::warn!("content metadata cache not serialised: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_and_stale_ones_are_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(dir.path().to_path_buf());
        let now = crate::auth::now_unix();
        update(&paths, |entries| {
            entries.insert("fresh".into(), CacheEntry { last_seen: now, sha1: Some("aa".into()), ..Default::default() });
            entries.insert("old".into(), CacheEntry { last_seen: now - FORGET_AFTER_SECS - 1, ..Default::default() });
        });
        let got = get_many(&paths, &["fresh".to_string(), "old".to_string()]);
        assert_eq!(got.len(), 1);
        assert_eq!(got["fresh"].sha1.as_deref(), Some("aa"));
    }

    #[test]
    fn keys_ignore_the_disabled_suffix_by_design() {
        assert_eq!(key("a.jar", 10, 5), "10:5:a.jar");
    }
}
