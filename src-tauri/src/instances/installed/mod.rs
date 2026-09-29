//! What's installed in an instance's `mods/`, `resourcepacks/` and
//! `shaderpacks/` folders: listing with each file's own metadata (name,
//! version, authors, icon, dependencies), enable/disable (a `.disabled`
//! suffix — the convention every popular launcher uses, and the game ignores
//! such files), delete, add from disk, and identification of the Modrinth or
//! CurseForge project each file comes from. Browsing and installing from the
//! catalogues lives in [`super::content`].

pub mod cache;
pub mod metadata;

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use sha1::{Digest, Sha1};

use super::content::ContentKind;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::providers::curseforge::{self, CurseForgeProvider};
use crate::providers::LoaderKind;
use crate::providers::modrinth::ModrinthApi;
use crate::util::fs::validate_file_name;
use cache::{CacheEntry, RemoteProject, RemoteProvider};
use metadata::LocalMeta;

pub const DISABLED_SUFFIX: &str = ".disabled";
/// Icons bigger than this aren't worth caching.
const MAX_ICON_BYTES: usize = 512 * 1024;
/// A lookup that found nothing is retried after this long.
const REMOTE_RECHECK_SECS: i64 = 3 * 24 * 3600;

/// One installed mod, resource pack or shader pack.
#[derive(Debug, Clone, Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct InstalledItem {
    /// File name without the `.disabled` suffix.
    pub file_name: String,
    pub enabled: bool,
    /// An unpacked pack (folder) rather than a file — can't be disabled.
    pub is_dir: bool,
    pub size: u64,
    /// Unix seconds.
    pub modified: i64,
    pub sha1: Option<String>,
    /// CurseForge fingerprint, to compare with a project's files.
    pub fingerprint: Option<u32>,
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub loaders: Vec<String>,
    /// For the instance's loader: mod ids this file provides, requires, and
    /// declares itself incompatible with.
    pub provides: Vec<String>,
    pub depends: Vec<String>,
    pub breaks: Vec<metadata::BreakRule>,
    /// Absolute path of the extracted icon (served by the asset protocol).
    pub icon_path: Option<String>,
    pub remote: Option<RemoteProject>,
}

struct Candidate {
    file_name: String,
    enabled: bool,
    is_dir: bool,
    size: u64,
    modified: i64,
    path: PathBuf,
}

impl Candidate {
    fn key(&self) -> String {
        cache::key(&self.file_name, self.size, self.modified)
    }
}

fn accepts(kind: ContentKind, name: &str, is_dir: bool, path: &Path) -> bool {
    let lower = name.to_ascii_lowercase();
    match kind {
        ContentKind::Mod => !is_dir && lower.ends_with(".jar"),
        ContentKind::ResourcePack => {
            if is_dir {
                path.join("pack.mcmeta").is_file()
            } else {
                lower.ends_with(".zip")
            }
        }
        ContentKind::Shader => is_dir || lower.ends_with(".zip"),
    }
}

fn modified_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
}

fn candidates(dir: &Path, kind: ContentKind) -> AppResult<Vec<Candidate>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let raw = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = entry.metadata() else { continue };
        let is_dir = meta.is_dir();
        let (file_name, enabled) = match raw.strip_suffix(DISABLED_SUFFIX) {
            Some(base) if !is_dir => (base.to_string(), false),
            _ => (raw.clone(), true),
        };
        if file_name.starts_with('.') || !accepts(kind, &file_name, is_dir, &entry.path()) {
            continue;
        }
        let size = if is_dir { crate::util::fs::dir_size(&entry.path()) } else { meta.len() };
        out.push(Candidate { file_name, enabled, is_dir, size, modified: modified_secs(&meta), path: entry.path() });
    }
    out.sort_by_key(|c| c.file_name.to_lowercase());
    Ok(out)
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

/// Reads a candidate once: hashes, metadata, and its icon saved to `icons`.
fn analyse(candidate: &Candidate, icons: &Path) -> CacheEntry {
    let mut entry = CacheEntry::default();
    let icon_bytes;
    if candidate.is_dir {
        entry.meta = metadata::read_folder_pack(&candidate.path);
        icon_bytes = entry.meta.icon_entry.as_ref().and_then(|rel| {
            crate::util::fs::safe_join(&candidate.path, rel).and_then(|p| std::fs::read(p).ok())
        });
    } else {
        let Ok(bytes) = std::fs::read(&candidate.path) else { return entry };
        entry.sha1 = Some(hex::encode(Sha1::digest(&bytes)));
        entry.fingerprint = Some(curseforge::fingerprint(&bytes));
        match zip::ZipArchive::new(std::io::Cursor::new(&bytes)) {
            Ok(mut zip) => {
                entry.meta = metadata::read_archive_from(&mut zip);
                icon_bytes = entry.meta.icon_entry.clone().and_then(|name| {
                    metadata::read_entry(&mut zip, name.trim_start_matches('/'))
                });
            }
            Err(_) => icon_bytes = None,
        }
    }
    if let Some(bytes) = icon_bytes.filter(|b| is_png(b) && b.len() <= MAX_ICON_BYTES) {
        let name = format!("{}.png", hex::encode(Sha1::digest(&bytes)));
        let path = icons.join(&name);
        if path.exists() || std::fs::create_dir_all(icons).and_then(|_| std::fs::write(&path, &bytes)).is_ok() {
            entry.icon = Some(name);
        }
    }
    entry
}

/// Analyses `todo` on every core; returns entries by candidate index.
fn analyse_all(todo: &[&Candidate], icons: &Path) -> Vec<CacheEntry> {
    let next = AtomicUsize::new(0);
    let results = parking_lot::Mutex::new(vec![CacheEntry::default(); todo.len()]);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8).min(todo.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(candidate) = todo.get(i) else { break };
                let entry = analyse(candidate, icons);
                results.lock()[i] = entry;
            });
        }
    });
    results.into_inner()
}

/// How much content an instance holds — counts only, nothing read or hashed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ContentSummary {
    pub mods: u32,
    pub mods_enabled: u32,
    pub resource_packs: u32,
    pub shaders: u32,
    pub worlds: u32,
}

pub fn summary(instance_dir: &Path) -> ContentSummary {
    let count = |kind: ContentKind| -> (u32, u32) {
        let Ok(entries) = std::fs::read_dir(instance_dir.join(kind.folder())) else { return (0, 0) };
        entries.flatten().fold((0, 0), |(total, enabled), entry| {
            let raw = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            let (name, on) = match raw.strip_suffix(DISABLED_SUFFIX) {
                Some(base) if !is_dir => (base, false),
                _ => (raw.as_str(), true),
            };
            if name.starts_with('.') || !accepts(kind, name, is_dir, &entry.path()) {
                return (total, enabled);
            }
            (total + 1, enabled + u32::from(on))
        })
    };
    let (mods, mods_enabled) = count(ContentKind::Mod);
    let worlds = std::fs::read_dir(instance_dir.join("saves"))
        .map(|entries| entries.flatten().filter(|e| e.path().join("level.dat").is_file()).count() as u32)
        .unwrap_or(0);
    ContentSummary {
        mods,
        mods_enabled,
        resource_packs: count(ContentKind::ResourcePack).0,
        shaders: count(ContentKind::Shader).0,
        worlds,
    }
}

/// Everything installed in `kind`'s folder, with metadata (dependencies as
/// `loader` sees them).
pub fn list(paths: &AppPaths, instance_dir: &Path, loader: LoaderKind, kind: ContentKind) -> AppResult<Vec<InstalledItem>> {
    let found = candidates(&instance_dir.join(kind.folder()), kind)?;
    let keys: Vec<String> = found.iter().map(Candidate::key).collect();
    let mut known = cache::get_many(paths, &keys);

    let todo: Vec<&Candidate> = found.iter().filter(|c| !known.contains_key(&c.key())).collect();
    let icons = cache::icons_dir(paths);
    let fresh: Vec<(String, CacheEntry)> =
        todo.iter().map(|c| c.key()).zip(analyse_all(&todo, &icons)).collect();

    let now = crate::auth::now_unix();
    cache::update(paths, |entries| {
        for (key, entry) in &fresh {
            entries.insert(key.clone(), CacheEntry { last_seen: now, ..entry.clone() });
        }
        for key in &keys {
            if let Some(e) = entries.get_mut(key) {
                e.last_seen = now;
            }
        }
    });
    known.extend(fresh);

    Ok(found
        .into_iter()
        .map(|c| {
            let entry = known.remove(&c.key()).unwrap_or_default();
            to_item(c, entry, loader, &icons)
        })
        .collect())
}

fn to_item(c: Candidate, entry: CacheEntry, loader: LoaderKind, icons: &Path) -> InstalledItem {
    let deps = entry.meta.deps_for(loader).cloned().unwrap_or_default();
    let LocalMeta { mod_id, name, version, description, authors, loaders, .. } = entry.meta;
    InstalledItem {
        file_name: c.file_name,
        enabled: c.enabled,
        is_dir: c.is_dir,
        size: c.size,
        modified: c.modified,
        sha1: entry.sha1,
        fingerprint: entry.fingerprint,
        mod_id,
        name,
        version,
        description,
        authors,
        loaders,
        provides: deps.provides,
        depends: deps.depends,
        breaks: deps.breaks,
        icon_path: entry.icon.map(|i| icons.join(i).display().to_string()),
        remote: entry.remote,
    }
}

pub fn modrinth_url(project_type: &str, slug: &str) -> String {
    let kind = match project_type {
        "resourcepack" | "shader" | "modpack" | "datapack" | "plugin" => project_type,
        _ => "mod",
    };
    format!("https://modrinth.com/{kind}/{slug}")
}

/// Looks up the Modrinth (by SHA-1) or CurseForge (by fingerprint) project
/// of every file not identified recently; returns how many were found.
pub async fn identify(
    paths: &AppPaths,
    modrinth: &ModrinthApi,
    curseforge: &CurseForgeProvider,
    instance_dir: &Path,
    loader: LoaderKind,
    kind: ContentKind,
) -> AppResult<usize> {
    let items = {
        let (paths, dir) = (paths.clone(), instance_dir.to_path_buf());
        tokio::task::spawn_blocking(move || list(&paths, &dir, loader, kind))
            .await
            .map_err(|e| AppError::Other(format!("tâche de fond interrompue: {e}")))??
    };
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

    let mut found: HashMap<String, RemoteProject> = HashMap::new();
    let by_sha1: HashMap<String, String> =
        pending.iter().filter_map(|(k, e)| e.sha1.clone().map(|h| (h, k.clone()))).collect();
    let hashes: Vec<String> = by_sha1.keys().cloned().collect();
    let mut versions = HashMap::new();
    for chunk in hashes.chunks(500) {
        versions.extend(modrinth.versions_by_hash(chunk).await?);
    }
    let mut project_ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect();
    project_ids.sort();
    project_ids.dedup();
    let mut projects = HashMap::new();
    for chunk in project_ids.chunks(100) {
        projects.extend(modrinth.projects(chunk).await?.into_iter().map(|p| (p.id.clone(), p)));
    }
    for (hash, version) in &versions {
        if let (Some(key), Some(p)) = (by_sha1.get(hash), projects.get(&version.project_id)) {
            let slug = if p.slug.is_empty() { &p.id } else { &p.slug };
            found.insert(
                key.clone(),
                RemoteProject {
                    provider: RemoteProvider::Modrinth,
                    project_id: p.id.clone(),
                    title: p.title.clone(),
                    description: p.description.clone(),
                    icon_url: p.icon_url.clone(),
                    url: modrinth_url(&p.project_type, slug),
                },
            );
        }
    }

    if curseforge.has_key() {
        let by_fingerprint: HashMap<u32, String> = pending
            .iter()
            .filter(|(k, _)| !found.contains_key(*k))
            .filter_map(|(k, e)| e.fingerprint.map(|f| (f, k.clone())))
            .collect();
        let fingerprints: Vec<u32> = by_fingerprint.keys().copied().collect();
        if !fingerprints.is_empty() {
            match curseforge.identify(&fingerprints).await {
                Ok(matches) => {
                    for (fp, p) in matches {
                        if let Some(key) = by_fingerprint.get(&fp) {
                            found.insert(
                                key.clone(),
                                RemoteProject {
                                    provider: RemoteProvider::Curseforge,
                                    project_id: p.id.to_string(),
                                    title: p.name,
                                    description: p.summary,
                                    icon_url: p.icon_url,
                                    url: p.website_url.unwrap_or_else(|| {
                                        format!("https://www.curseforge.com/projects/{}", p.id)
                                    }),
                                },
                            );
                        }
                    }
                }
                Err(e) => tracing::warn!("CurseForge fingerprint lookup failed: {e}"),
            }
        }
    }

    let identified = found.len();
    cache::update(paths, |entries| {
        for key in pending.keys() {
            if let Some(entry) = entries.get_mut(key) {
                entry.remote_checked_at = now;
                entry.remote = found.remove(key);
            }
        }
    });
    Ok(identified)
}

fn paths_for(dir: &Path, file_name: &str) -> (PathBuf, PathBuf) {
    (dir.join(file_name), dir.join(format!("{file_name}{DISABLED_SUFFIX}")))
}

pub fn set_enabled(instance_dir: &Path, kind: ContentKind, file_name: &str, enabled: bool) -> AppResult<()> {
    validate_file_name(file_name)?;
    let dir = instance_dir.join(kind.folder());
    let (on, off) = paths_for(&dir, file_name);
    if on.is_dir() {
        return Err(AppError::Instance(format!("« {file_name} » est un dossier : il ne peut pas être désactivé")));
    }
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    if to.exists() && !from.exists() {
        return Ok(());
    }
    if !from.exists() {
        return Err(AppError::Instance(format!("fichier introuvable : {file_name}")));
    }
    std::fs::rename(from, to)?;
    Ok(())
}

pub fn delete(instance_dir: &Path, kind: ContentKind, file_name: &str) -> AppResult<()> {
    validate_file_name(file_name)?;
    let dir = instance_dir.join(kind.folder());
    let (on, off) = paths_for(&dir, file_name);
    if on.is_dir() {
        std::fs::remove_dir_all(on)?;
    } else if on.exists() {
        std::fs::remove_file(on)?;
    } else if off.exists() {
        std::fs::remove_file(off)?;
    } else {
        return Err(AppError::Instance(format!("fichier introuvable : {file_name}")));
    }
    Ok(())
}

/// Extensions a file must have to be added to `kind`'s folder.
pub fn accepted_extension(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Mod => "jar",
        ContentKind::ResourcePack | ContentKind::Shader => "zip",
    }
}

pub fn add_from_path(instance_dir: &Path, kind: ContentKind, source: &Path) -> AppResult<String> {
    let ext = accepted_extension(kind);
    if !source.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        return Err(AppError::Instance(format!("seuls les fichiers .{ext} peuvent être ajoutés ici")));
    }
    let file_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Instance("chemin de fichier invalide".to_string()))?;
    let mut header = [0u8; 4];
    std::fs::File::open(source)?.read_exact(&mut header).map_err(|_| not_an_archive(&file_name))?;
    // A local file header, or the end record of an empty archive.
    if &header != b"PK\x03\x04" && &header != b"PK\x05\x06" {
        return Err(not_an_archive(&file_name));
    }
    let dir = instance_dir.join(kind.folder());
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(source, dir.join(&file_name))?;
    Ok(file_name)
}

fn not_an_archive(name: &str) -> AppError {
    AppError::Instance(format!("« {name} » n'est pas une archive valide"))
}

#[cfg(test)]
mod tests;
