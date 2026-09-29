//! Which Modrinth or CurseForge project each installed file belongs to:
//! Modrinth by SHA-1, then CurseForge by fingerprint for what Modrinth
//! doesn't know. One catalogue being down doesn't stop the other, and files
//! are only marked as "looked up" when the lookup actually happened.

use std::collections::HashMap;
use std::path::Path;

use super::cache::{self, CacheEntry, RemoteProject, RemoteProvider};
use super::{ensure_hashes, list, modrinth_url, ContentKind, REMOTE_RECHECK_SECS};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::providers::curseforge::CurseForgeProvider;
use crate::providers::modrinth::ModrinthApi;
use crate::providers::{LoaderKind, ProviderError};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("tâche de fond interrompue: {e}")))?
}

/// Cache key → project, for the SHA-1s Modrinth knows.
async fn from_modrinth(
    api: &ModrinthApi,
    by_sha1: &HashMap<String, String>,
) -> Result<HashMap<String, RemoteProject>, ProviderError> {
    let hashes: Vec<String> = by_sha1.keys().cloned().collect();
    let mut versions = HashMap::new();
    for chunk in hashes.chunks(500) {
        versions.extend(api.versions_by_hash(chunk).await?);
    }
    let mut project_ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect();
    project_ids.sort();
    project_ids.dedup();
    let mut projects = HashMap::new();
    for chunk in project_ids.chunks(100) {
        projects.extend(api.projects(chunk).await?.into_iter().map(|p| (p.id.clone(), p)));
    }
    Ok(versions
        .iter()
        .filter_map(|(hash, version)| {
            let (key, p) = (by_sha1.get(hash)?, projects.get(&version.project_id)?);
            let slug = if p.slug.is_empty() { &p.id } else { &p.slug };
            Some((
                key.clone(),
                RemoteProject {
                    provider: RemoteProvider::Modrinth,
                    project_id: p.id.clone(),
                    title: p.title.clone(),
                    description: p.description.clone(),
                    icon_url: p.icon_url.clone(),
                    url: modrinth_url(&p.project_type, slug),
                },
            ))
        })
        .collect())
}

/// Cache key → project, for the fingerprints CurseForge knows.
async fn from_curseforge(
    cf: &CurseForgeProvider,
    by_fingerprint: &HashMap<u32, String>,
) -> Result<HashMap<String, RemoteProject>, ProviderError> {
    let fingerprints: Vec<u32> = by_fingerprint.keys().copied().collect();
    if fingerprints.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(cf
        .identify(&fingerprints)
        .await?
        .into_iter()
        .filter_map(|(fp, p)| {
            let key = by_fingerprint.get(&fp)?.clone();
            let url = p.website_url.clone().unwrap_or_else(|| format!("https://www.curseforge.com/projects/{}", p.id));
            Some((
                key,
                RemoteProject {
                    provider: RemoteProvider::Curseforge,
                    project_id: p.id.to_string(),
                    title: p.name,
                    description: p.summary,
                    icon_url: p.icon_url,
                    url,
                },
            ))
        })
        .collect())
}

/// Looks up the project of every file not identified recently; returns how
/// many were found.
pub async fn identify(
    paths: &AppPaths,
    modrinth: &ModrinthApi,
    curseforge: &CurseForgeProvider,
    instance_dir: &Path,
    loader: LoaderKind,
    kind: ContentKind,
) -> AppResult<usize> {
    let (p, dir) = (paths.clone(), instance_dir.to_path_buf());
    let items = blocking(move || {
        ensure_hashes(&p, &dir, kind)?;
        list(&p, &dir, loader, kind)
    })
    .await?;

    let now = crate::auth::now_unix();
    let keys: Vec<String> = items
        .iter()
        .filter(|i| !i.is_dir && i.remote.is_none())
        .map(|i| cache::key(&i.file_name, i.size, i.modified))
        .collect();
    let pending: HashMap<String, CacheEntry> = cache::get_many(paths, &keys)
        .into_iter()
        .filter(|(_, e)| e.sha1.is_some() && now - e.remote_checked_at > REMOTE_RECHECK_SECS)
        .collect();
    if pending.is_empty() {
        return Ok(0);
    }

    let by_sha1: HashMap<String, String> =
        pending.iter().filter_map(|(k, e)| e.sha1.clone().map(|h| (h, k.clone()))).collect();
    let (mut found, modrinth_error) = match from_modrinth(modrinth, &by_sha1).await {
        Ok(found) => (found, None),
        Err(e) => {
            tracing::warn!("Modrinth identification failed: {e}");
            (HashMap::new(), Some(e))
        }
    };

    let mut curseforge_failed = false;
    if curseforge.has_key() {
        let by_fingerprint: HashMap<u32, String> = pending
            .iter()
            .filter(|(k, _)| !found.contains_key(*k))
            .filter_map(|(k, e)| e.fingerprint.map(|f| (f, k.clone())))
            .collect();
        match from_curseforge(curseforge, &by_fingerprint).await {
            Ok(matches) => found.extend(matches),
            Err(e) => {
                tracing::warn!("CurseForge identification failed: {e}");
                curseforge_failed = true;
            }
        }
    }

    // A file counts as looked up only if every catalogue could answer; else
    // it's retried on the next visit rather than in three days.
    let complete = modrinth_error.is_none() && !curseforge_failed;
    let identified = found.len();
    cache::update(paths, |entries| {
        for key in pending.keys() {
            let Some(entry) = entries.get_mut(key) else { continue };
            if let Some(project) = found.remove(key) {
                entry.remote = Some(project);
                entry.remote_checked_at = now;
            } else if complete {
                entry.remote_checked_at = now;
            }
        }
    });
    match modrinth_error {
        Some(e) if identified == 0 => Err(e.into()),
        _ => Ok(identified),
    }
}
