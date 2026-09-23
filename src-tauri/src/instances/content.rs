//! Modrinth content for one instance: browsing compatible mods / resource
//! packs / shaders, installing them (with required dependencies), and
//! checking installed mods for updates by file hash.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::download::{sha1_of_file, DownloadItem, DownloadManager};
use crate::error::{AppError, AppResult};
use crate::providers::modrinth::api::{SearchHit, Version};
use crate::providers::modrinth::ModrinthApi;
use crate::util::fs::{is_plain_file_name, validate_file_name};

use super::Instance;

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

fn loader_filter(instance: &Instance, kind: ContentKind) -> Vec<&'static str> {
    match kind {
        ContentKind::Mod => instance.loader.modrinth_name().into_iter().collect(),
        _ => Vec::new(),
    }
}

pub async fn search(
    api: &ModrinthApi,
    instance: &Instance,
    kind: ContentKind,
    query: &str,
    offset: u32,
) -> AppResult<Vec<SearchHit>> {
    let mut facets = vec![
        vec![format!("project_type:{}", kind.project_type())],
        vec![format!("versions:{}", instance.minecraft_version)],
    ];
    if let Some(loader) = loader_filter(instance, kind).first() {
        facets.push(vec![format!("categories:{loader}")]);
    }
    Ok(api.search(query, facets, offset, 30).await?)
}

async fn compatible_version(api: &ModrinthApi, instance: &Instance, kind: ContentKind, project_id: &str) -> AppResult<Version> {
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

async fn hash_folder(dir: &Path) -> AppResult<HashMap<String, String>> {
    let mut out = HashMap::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Ok(out);
    };
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".part") {
            continue;
        }
        out.insert(sha1_of_file(&entry.path()).await?, name);
    }
    Ok(out)
}

/// Installs a project and, for mods, every required dependency not already
/// present. Returns the file names written.
pub async fn install(
    api: &ModrinthApi,
    downloader: &DownloadManager,
    instance: &Instance,
    project_id: &str,
    kind: ContentKind,
) -> AppResult<Vec<String>> {
    let root = compatible_version(api, instance, kind, project_id).await?;

    let mut installed_projects: HashSet<String> = HashSet::new();
    if kind == ContentKind::Mod && root.dependencies.iter().any(|d| d.dependency_type == "required") {
        let hashes: Vec<String> = hash_folder(&instance.directory.join("mods")).await?.into_keys().collect();
        if let Ok(known) = api.versions_by_hash(&hashes).await {
            installed_projects.extend(known.into_values().map(|v| v.project_id));
        }
    }

    let mut queue = VecDeque::from([root]);
    let mut seen: HashSet<String> = HashSet::from([project_id.to_string()]);
    let mut written = Vec::new();
    while let Some(version) = queue.pop_front() {
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
        written.push(file.filename.clone());

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModUpdate {
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

/// Every mod in `mods/` Modrinth knows a newer compatible version of.
pub async fn check_updates(api: &ModrinthApi, instance: &Instance) -> AppResult<Vec<ModUpdate>> {
    let by_hash = hash_folder(&instance.directory.join("mods")).await?;
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
    fn enabled_name_strips_the_disabled_suffix() {
        assert_eq!(enabled_name("a.jar.disabled"), "a.jar");
        assert_eq!(enabled_name("a.jar"), "a.jar");
    }

    #[tokio::test]
    async fn hash_folder_indexes_files_by_sha1() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jar"), b"abc").unwrap();
        std::fs::write(dir.path().join("b.jar.part"), b"partial").unwrap();
        let hashes = hash_folder(dir.path()).await.unwrap();
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes["a9993e364706816aba3e25717850c26c9cd0d89d"], "a.jar");
    }
}
