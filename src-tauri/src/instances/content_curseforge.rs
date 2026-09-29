//! CurseForge content for one instance — the counterpart of the Modrinth
//! code in [`super::content`]: search, install (with required
//! dependencies) and updates of mods identified as CurseForge projects.
//! Files whose author forbids third-party downloads are never fetched
//! behind their back: the error points to the project page instead.

use std::collections::{HashSet, VecDeque};

use super::content::{ContentHit, ContentKind, ModUpdate};
use super::installed::cache::RemoteProvider;
use super::installed::InstalledItem;
use super::Instance;
use crate::download::{DownloadItem, DownloadManager};
use crate::error::{AppError, AppResult};
use crate::providers::curseforge::content::{
    loader_type, pick_install_file, CfContentFile, CLASS_MODS, CLASS_RESOURCE_PACKS, CLASS_SHADERS,
};
use crate::providers::curseforge::CurseForgeProvider;
use crate::util::fs::is_plain_file_name;

fn class_id(kind: ContentKind) -> u32 {
    match kind {
        ContentKind::Mod => CLASS_MODS,
        ContentKind::ResourcePack => CLASS_RESOURCE_PACKS,
        ContentKind::Shader => CLASS_SHADERS,
    }
}

/// Loader filter: only mods depend on the instance's loader.
fn loader_filter(instance: &Instance, kind: ContentKind) -> Option<u32> {
    (kind == ContentKind::Mod).then(|| loader_type(instance.loader)).flatten()
}

fn project_url(slug: &str, id: u32, website: Option<String>, kind: ContentKind) -> String {
    website.unwrap_or_else(|| {
        let section = match kind {
            ContentKind::Mod => "mc-mods",
            ContentKind::ResourcePack => "texture-packs",
            ContentKind::Shader => "shaders",
        };
        if slug.is_empty() {
            format!("https://www.curseforge.com/projects/{id}")
        } else {
            format!("https://www.curseforge.com/minecraft/{section}/{slug}")
        }
    })
}

pub async fn search(
    cf: &CurseForgeProvider,
    instance: &Instance,
    kind: ContentKind,
    query: &str,
    offset: u32,
) -> AppResult<Vec<ContentHit>> {
    let hits = cf
        .search_content(class_id(kind), &instance.minecraft_version, loader_filter(instance, kind), query, offset)
        .await?;
    Ok(hits
        .into_iter()
        .map(|h| ContentHit {
            provider: RemoteProvider::Curseforge,
            url: project_url(&h.slug, h.id, h.website_url, kind),
            project_id: h.id.to_string(),
            slug: h.slug,
            title: h.name,
            description: h.summary,
            author: h.author,
            icon_url: h.icon_url,
            downloads: h.downloads,
        })
        .collect())
}

fn parse_id(project_id: &str) -> AppResult<u32> {
    project_id.parse().map_err(|_| AppError::Provider(format!("identifiant CurseForge invalide : {project_id}")))
}

async fn download(downloader: &DownloadManager, instance: &Instance, kind: ContentKind, file: &CfContentFile) -> AppResult<String> {
    let url = file.download_url.clone().ok_or_else(|| {
        AppError::Provider(format!(
            "l'auteur de « {} » interdit le téléchargement par les launchers : télécharge-le depuis sa page CurseForge",
            file.display_name
        ))
    })?;
    if !is_plain_file_name(&file.file_name) {
        return Err(AppError::Provider(format!("nom de fichier refusé: {}", file.file_name)));
    }
    downloader
        .ensure_file(&DownloadItem {
            url,
            dest: instance.directory.join(kind.folder()).join(&file.file_name),
            sha1: file.sha1.clone(),
            size: (file.size > 0).then_some(file.size),
        })
        .await?;
    Ok(file.file_name.clone())
}

/// Installs a project and, for mods, every required dependency not already
/// installed (`installed` = CurseForge projects already in the instance).
/// Returns the file names written.
pub async fn install(
    cf: &CurseForgeProvider,
    downloader: &DownloadManager,
    instance: &Instance,
    project_id: &str,
    kind: ContentKind,
    installed: &[InstalledItem],
) -> AppResult<Vec<String>> {
    let root = parse_id(project_id)?;
    let loader = loader_filter(instance, kind);
    let mut present: HashSet<u32> = installed
        .iter()
        .filter_map(|i| i.remote.as_ref())
        .filter(|r| r.provider == RemoteProvider::Curseforge)
        .filter_map(|r| r.project_id.parse().ok())
        .collect();
    present.remove(&root);

    let mut queue = VecDeque::from([root]);
    let mut seen = HashSet::from([root]);
    let mut written = Vec::new();
    while let Some(id) = queue.pop_front() {
        let files = cf.content_files(id, &instance.minecraft_version, loader).await?;
        let Some(file) = pick_install_file(&files) else {
            if id == root {
                return Err(AppError::Provider(format!(
                    "aucune version compatible avec Minecraft {} ({:?})",
                    instance.minecraft_version, instance.loader
                )));
            }
            continue;
        };
        match download(downloader, instance, kind, file).await {
            Ok(name) => written.push(name),
            Err(e) if id == root => return Err(e),
            Err(e) => tracing::warn!("CurseForge dependency {id} not installed: {e}"),
        }
        if kind == ContentKind::Mod {
            for dep in &file.required_projects {
                if !present.contains(dep) && seen.insert(*dep) {
                    queue.push_back(*dep);
                }
            }
        }
    }
    Ok(written)
}

/// Newer files for the mods identified as CurseForge projects: offered only
/// when the installed file is one of the project's files for this version
/// and a more recent one exists — never a downgrade, never a guess.
pub async fn check_updates(
    cf: &CurseForgeProvider,
    instance: &Instance,
    installed: &[InstalledItem],
) -> AppResult<Vec<ModUpdate>> {
    let loader = loader_filter(instance, ContentKind::Mod);
    let mut updates = Vec::new();
    for item in installed {
        let (Some(remote), Some(fingerprint)) = (&item.remote, item.fingerprint) else { continue };
        if remote.provider != RemoteProvider::Curseforge {
            continue;
        }
        let Ok(id) = remote.project_id.parse::<u32>() else { continue };
        let files = match cf.content_files(id, &instance.minecraft_version, loader).await {
            Ok(files) => files,
            Err(e) => {
                tracing::warn!("CurseForge update check for {} failed: {e}", remote.title);
                continue;
            }
        };
        let (Some(newest), Some(current)) = (files.first(), files.iter().find(|f| f.fingerprint == fingerprint))
        else {
            continue;
        };
        if current.id >= newest.id {
            continue;
        }
        let (Some(url), Some(sha1)) = (newest.download_url.clone(), newest.sha1.clone()) else { continue };
        if !is_plain_file_name(&newest.file_name) {
            continue;
        }
        updates.push(ModUpdate {
            provider: RemoteProvider::Curseforge,
            file_name: if item.enabled {
                item.file_name.clone()
            } else {
                format!("{}{}", item.file_name, super::installed::DISABLED_SUFFIX)
            },
            project_id: remote.project_id.clone(),
            title: remote.title.clone(),
            icon_url: remote.icon_url.clone(),
            current_version: current.display_name.clone(),
            new_version: newest.display_name.clone(),
            new_file_name: newest.file_name.clone(),
            url,
            sha1,
            size: newest.size,
        });
    }
    Ok(updates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_urls_fall_back_to_the_section_page() {
        assert_eq!(project_url("jei", 1, None, ContentKind::Mod), "https://www.curseforge.com/minecraft/mc-mods/jei");
        assert_eq!(project_url("", 9, None, ContentKind::Shader), "https://www.curseforge.com/projects/9");
        assert_eq!(project_url("x", 1, Some("https://site".into()), ContentKind::Mod), "https://site");
    }

    #[test]
    fn only_mods_are_filtered_by_loader() {
        let instance: Instance = serde_json::from_value(serde_json::json!({
            "id": "i", "name": "I", "minecraft_version": "1.20.1", "loader": "fabric",
            "loader_version": "0.15", "directory": "/i"
        }))
        .unwrap();
        assert_eq!(loader_filter(&instance, ContentKind::Mod), Some(4));
        assert_eq!(loader_filter(&instance, ContentKind::Shader), None);
        assert_eq!(class_id(ContentKind::ResourcePack), 12);
    }
}
