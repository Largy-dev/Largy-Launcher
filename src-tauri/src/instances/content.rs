//! Modrinth content for one instance: browsing compatible mods / resource
//! packs / shaders, installing them (with required dependencies), and
//! checking installed mods for updates by file hash.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::download::{DownloadItem, DownloadManager};
use crate::error::{AppError, AppResult};
use crate::instances::installed::cache::RemoteProvider;
use crate::providers::modrinth::api::{SearchHit, Version};
use crate::providers::LoaderKind;
use crate::providers::modrinth::ModrinthApi;
use crate::util::fs::{is_plain_file_name, validate_file_name};

use super::Instance;

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Mod,
    ResourcePack,
    Shader,
}

impl ContentKind {
    fn project_type(self) -> &'static str {
        match self {
            ContentKind::Mod => "mod",
            ContentKind::ResourcePack => "resourcepack",
            ContentKind::Shader => "shader",
        }
    }

    pub fn folder(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::ResourcePack => "resourcepacks",
            ContentKind::Shader => "shaderpacks",
        }
    }
}

/// Modrinth loaders whose mods run on the instance: Quilt also loads Fabric
/// mods, and NeoForge for 1.20.1 is still Forge-compatible.
pub fn compatible_loaders(instance: &Instance) -> Vec<&'static str> {
    match instance.loader {
        LoaderKind::Quilt => vec!["quilt", "fabric"],
        LoaderKind::NeoForge if instance.minecraft_version == "1.20.1" => vec!["neoforge", "forge"],
        other => other.modrinth_name().into_iter().collect(),
    }
}

fn loader_filter(instance: &Instance, kind: ContentKind) -> Vec<&'static str> {
    match kind {
        ContentKind::Mod => compatible_loaders(instance),
        _ => Vec::new(),
    }
}

/// A catalogue project, whichever provider it comes from.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ContentHit {
    pub provider: RemoteProvider,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    /// Project page on the provider's website.
    pub url: String,
}

impl From<SearchHit> for ContentHit {
    fn from(hit: SearchHit) -> Self {
        let slug = if hit.slug.is_empty() { hit.project_id.clone() } else { hit.slug.clone() };
        ContentHit {
            provider: RemoteProvider::Modrinth,
            url: crate::instances::installed::modrinth_url(&hit.project_type, &slug),
            project_id: hit.project_id,
            slug,
            title: hit.title,
            description: hit.description,
            author: hit.author,
            icon_url: hit.icon_url,
            downloads: hit.downloads,
        }
    }
}

pub async fn search(
    api: &ModrinthApi,
    instance: &Instance,
    kind: ContentKind,
    query: &str,
    offset: u32,
) -> AppResult<Vec<ContentHit>> {
    let mut facets = vec![
        vec![format!("project_type:{}", kind.project_type())],
        vec![format!("versions:{}", instance.minecraft_version)],
    ];
    let loaders = loader_filter(instance, kind);
    if !loaders.is_empty() {
        facets.push(loaders.iter().map(|l| format!("categories:{l}")).collect());
    }
    Ok(api.search(query, facets, offset, 30).await?.into_iter().map(ContentHit::from).collect())
}

pub(crate) async fn compatible_version(api: &ModrinthApi, instance: &Instance, kind: ContentKind, project_id: &str) -> AppResult<Version> {
    let loaders = loader_filter(instance, kind);
    let versions = api.project_versions(project_id, &loaders, &[instance.minecraft_version.as_str()]).await?;
    versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first())
        .cloned()
        .ok_or_else(|| {
            AppError::Provider(format!(
                "aucune version compatible avec Minecraft {} ({:?})",
                instance.minecraft_version, instance.loader
            ))
        })
}

fn enabled_name(name: &str) -> &str {
    name.strip_suffix(DISABLED_SUFFIX).unwrap_or(name)
}

/// Downloads the primary file of exactly `version`, without its dependencies.
/// Returns the file name written.
pub async fn install_exact(
    downloader: &DownloadManager,
    instance: &Instance,
    version: &Version,
    kind: ContentKind,
) -> AppResult<String> {
    let file = version
        .primary_file()
        .ok_or_else(|| AppError::Provider(format!("la version {} n'a pas de fichier", version.name)))?;
    if !is_plain_file_name(&file.filename) {
        return Err(AppError::Provider(format!("nom de fichier refusé: {}", file.filename)));
    }
    let target = instance.directory.join(kind.folder()).join(&file.filename);
    downloader
        .ensure_file(&DownloadItem {
            url: file.url.clone(),
            dest: target,
            sha1: Some(file.hashes.sha1.clone()),
            size: (file.size > 0).then_some(file.size),
        })
        .await?;
    Ok(file.filename.clone())
}

/// Installs a project and, for mods, every required dependency not already
/// present. Returns the file names written.
/// `installed_hashes`: SHA-1 of the mods already there (see
/// `installed::hashes`), so dependencies already present aren't added twice.
pub async fn install(
    api: &ModrinthApi,
    downloader: &DownloadManager,
    instance: &Instance,
    project_id: &str,
    kind: ContentKind,
    installed_hashes: &[String],
) -> AppResult<Vec<String>> {
    let root = compatible_version(api, instance, kind, project_id).await?;

    let mut installed_projects: HashSet<String> = HashSet::new();
    if kind == ContentKind::Mod && root.dependencies.iter().any(|d| d.dependency_type == "required") {
        if let Ok(known) = api.versions_by_hash(installed_hashes).await {
            installed_projects.extend(known.into_values().map(|v| v.project_id));
        }
    }

    let mut queue = VecDeque::from([root]);
    let mut seen: HashSet<String> = HashSet::from([project_id.to_string()]);
    let mut written = Vec::new();
    while let Some(version) = queue.pop_front() {
        written.push(install_exact(downloader, instance, &version, kind).await?);

        if kind != ContentKind::Mod {
            continue;
        }
        for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
            let dep_version = match (&dep.version_id, &dep.project_id) {
                (Some(vid), _) => api.version(vid).await.ok(),
                (None, Some(pid)) => compatible_version(api, instance, kind, pid).await.ok(),
                (None, None) => None,
            };
            let Some(dep_version) = dep_version else {
                continue;
            };
            if installed_projects.contains(&dep_version.project_id) || !seen.insert(dep_version.project_id.clone()) {
                continue;
            }
            queue.push_back(dep_version);
        }
    }
    Ok(written)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ModUpdate {
    pub provider: RemoteProvider,
    pub file_name: String,
    pub project_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub current_version: String,
    pub new_version: String,
    pub new_file_name: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

/// Every mod Modrinth knows a newer compatible version of. `by_hash`: SHA-1
/// → file name on disk of the instance's mods (see `installed::hashes`).
pub async fn check_updates(
    api: &ModrinthApi,
    instance: &Instance,
    by_hash: HashMap<String, String>,
) -> AppResult<Vec<ModUpdate>> {
    let by_hash: HashMap<String, String> =
        by_hash.into_iter().filter(|(_, name)| enabled_name(name).ends_with(".jar")).collect();
    if by_hash.is_empty() {
        return Ok(Vec::new());
    }
    let hashes: Vec<String> = by_hash.keys().cloned().collect();
    let loaders = loader_filter(instance, ContentKind::Mod);
    let game_versions = [instance.minecraft_version.as_str()];
    let (current, latest) = tokio::join!(
        api.versions_by_hash(&hashes),
        api.latest_by_hash(&hashes, &loaders, &game_versions)
    );
    let (current, latest) = (current?, latest?);

    let mut updates = Vec::new();
    for (hash, new) in latest {
        let Some(old) = current.get(&hash) else {
            continue;
        };
        if old.id == new.id {
            continue;
        }
        let Some(file) = new.primary_file() else {
            continue;
        };
        if file.hashes.sha1 == hash || !is_plain_file_name(&file.filename) {
            continue;
        }
        updates.push(ModUpdate {
            provider: RemoteProvider::Modrinth,
            file_name: by_hash.get(&hash).cloned().unwrap_or_default(),
            project_id: new.project_id.clone(),
            title: String::new(),
            icon_url: None,
            current_version: old.version_number.clone(),
            new_version: new.version_number.clone(),
            new_file_name: file.filename.clone(),
            url: file.url.clone(),
            sha1: file.hashes.sha1.clone(),
            size: file.size,
        });
    }

    let ids: Vec<String> = updates.iter().map(|u| u.project_id.clone()).collect();
    if let Ok(projects) = api.projects(&ids).await {
        let by_id: HashMap<_, _> = projects.into_iter().map(|p| (p.id.clone(), p)).collect();
        for update in &mut updates {
            if let Some(p) = by_id.get(&update.project_id) {
                update.title = p.title.clone();
                update.icon_url = p.icon_url.clone();
            }
        }
    }
    updates.sort_by_key(|u| u.title.to_lowercase());
    Ok(updates)
}

/// Downloads the new file, then removes the old one; a disabled mod stays
/// disabled.
pub async fn apply_update(downloader: &DownloadManager, instance: &Instance, update: &ModUpdate) -> AppResult<()> {
    validate_file_name(&update.file_name)?;
    validate_file_name(&update.new_file_name)?;
    let mods = instance.directory.join("mods");
    let disabled = update.file_name.ends_with(DISABLED_SUFFIX);
    let new_name =
        if disabled { format!("{}{DISABLED_SUFFIX}", update.new_file_name) } else { update.new_file_name.clone() };

    downloader
        .ensure_file(&DownloadItem {
            url: update.url.clone(),
            dest: mods.join(&new_name),
            sha1: Some(update.sha1.clone()),
            size: (update.size > 0).then_some(update.size),
        })
        .await?;
    if new_name != update.file_name {
        let _ = tokio::fs::remove_file(mods.join(&update.file_name)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_kinds_map_to_modrinth_types_and_folders() {
        assert_eq!(ContentKind::Mod.project_type(), "mod");
        assert_eq!(ContentKind::Shader.folder(), "shaderpacks");
    }

    #[test]
    fn quilt_and_neoforge_1_20_1_accept_their_parent_loaders_mods() {
        let instance = |loader: &str, mc: &str| -> Instance {
            serde_json::from_value(serde_json::json!({
                "id": "i", "name": "I", "minecraft_version": mc, "loader": loader,
                "loader_version": "1", "directory": "/i"
            }))
            .unwrap()
        };
        assert_eq!(compatible_loaders(&instance("quilt", "1.21.1")), vec!["quilt", "fabric"]);
        assert_eq!(compatible_loaders(&instance("neoforge", "1.20.1")), vec!["neoforge", "forge"]);
        assert_eq!(compatible_loaders(&instance("neoforge", "1.21.1")), vec!["neoforge"]);
        assert!(compatible_loaders(&instance("vanilla", "1.21.1")).is_empty());
    }

    #[test]
    fn enabled_name_strips_the_disabled_suffix() {
        assert_eq!(enabled_name("a.jar.disabled"), "a.jar");
        assert_eq!(enabled_name("a.jar"), "a.jar");
    }

}
