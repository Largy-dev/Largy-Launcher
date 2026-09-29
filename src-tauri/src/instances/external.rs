//! Instances living in other launchers — Mojang's official launcher, Prism /
//! MultiMC / PolyMC, the CurseForge app, the Modrinth App — found where
//! those launchers keep them, and copied into Largy with their mods,
//! configs, packs, options and (optionally) worlds.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{CreateInstanceInput, Instance};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::providers::LoaderKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ExternalSource {
    Official,
    Prism,
    Curseforge,
    Modrinth,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ExternalInstance {
    /// `<source>:<game dir>` — stable, and what [`import`] takes.
    pub id: String,
    pub source: ExternalSource,
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    pub game_dir: String,
    pub mods: u32,
    pub worlds: u32,
    /// Unix seconds.
    pub last_played: Option<i64>,
    /// An instance was already imported from it.
    pub already_imported: bool,
}

/// Folders and files copied into the new instance (`saves` on request).
const FOLDERS: &[&str] = &["mods", "config", "defaultconfigs", "kubejs", "scripts", "resourcepacks", "shaderpacks"];
const FILES: &[&str] = &["options.txt", "optionsof.txt", "optionsshaders.txt", "servers.dat"];

/// Where other launchers keep their data on this machine.
pub struct Roots {
    pub official: Vec<PathBuf>,
    pub prism: Vec<PathBuf>,
    pub curseforge: Vec<PathBuf>,
    pub modrinth: Vec<PathBuf>,
}

impl Roots {
    pub fn system() -> Self {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let home = env(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
        let data = if cfg!(windows) {
            env("APPDATA")
        } else if cfg!(target_os = "macos") {
            home.as_ref().map(|h| h.join("Library/Application Support"))
        } else {
            env("XDG_DATA_HOME").or_else(|| home.as_ref().map(|h| h.join(".local/share")))
        };
        let join = |base: &Option<PathBuf>, rel: &str| base.as_ref().map(|b| b.join(rel));
        Roots {
            official: [
                join(&data, if cfg!(target_os = "macos") { "minecraft" } else { ".minecraft" }),
                join(&home, ".minecraft").filter(|_| cfg!(target_os = "linux")),
            ]
            .into_iter()
            .flatten()
            .collect(),
            prism: ["PrismLauncher/instances", "PolyMC/instances", "multimc/instances"]
                .iter()
                .filter_map(|rel| join(&data, rel))
                .collect(),
            curseforge: join(&home, "curseforge/minecraft/Instances").into_iter().collect(),
            modrinth: ["ModrinthApp/profiles", "com.modrinth.theseus/profiles"]
                .iter()
                .filter_map(|rel| join(&data, rel))
                .collect(),
        }
    }
}

fn count_mods(game_dir: &Path) -> u32 {
    std::fs::read_dir(game_dir.join("mods"))
        .map(|e| e.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".jar")).count() as u32)
        .unwrap_or(0)
}

fn count_worlds(game_dir: &Path) -> u32 {
    std::fs::read_dir(game_dir.join("saves"))
        .map(|e| e.flatten().filter(|e| e.path().join("level.dat").is_file()).count() as u32)
        .unwrap_or(0)
}

fn parse_date(value: Option<&str>) -> Option<i64> {
    value.and_then(crate::providers::parse_date).filter(|&t| t > 0)
}

/// Loader and loader version from a version id as the official launcher
/// (and the installers) name them.
pub fn loader_from_version_id(id: &str, minecraft: &str) -> (LoaderKind, Option<String>) {
    let strip_mc = |v: &str| v.strip_suffix(&format!("-{minecraft}")).unwrap_or(v).to_string();
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        return (LoaderKind::Fabric, Some(strip_mc(rest)));
    }
    if let Some(rest) = id.strip_prefix("quilt-loader-") {
        return (LoaderKind::Quilt, Some(strip_mc(rest)));
    }
    if let Some(rest) = id.strip_prefix("neoforge-") {
        return (LoaderKind::NeoForge, Some(rest.to_string()));
    }
    if let Some((_, rest)) = id.split_once("-forge-") {
        return (LoaderKind::Forge, Some(rest.to_string()));
    }
    // 1.7.10-Forge10.13.4.1614-1.7.10 — the Maven layout of old Forge.
    if let Some((_, rest)) = id.split_once("-Forge") {
        return (LoaderKind::Forge, Some(rest.to_string()));
    }
    (LoaderKind::Vanilla, None)
}

#[derive(Deserialize)]
struct LauncherProfiles {
    #[serde(default)]
    profiles: std::collections::HashMap<String, OfficialProfile>,
}

#[derive(Deserialize)]
struct OfficialProfile {
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(rename = "lastVersionId", default)]
    last_version_id: String,
    #[serde(rename = "gameDir", default)]
    game_dir: Option<String>,
    #[serde(rename = "lastUsed", default)]
    last_used: Option<String>,
}

#[derive(Deserialize)]
struct VersionJson {
    #[serde(rename = "inheritsFrom", default)]
    inherits_from: Option<String>,
}

fn official(root: &Path, latest_release: Option<&str>) -> Vec<ExternalInstance> {
    let Ok(bytes) = std::fs::read(root.join("launcher_profiles.json")) else { return Vec::new() };
    let Ok(file) = serde_json::from_slice::<LauncherProfiles>(&bytes) else { return Vec::new() };
    let mut out = Vec::new();
    for profile in file.profiles.into_values() {
        let version_id = match profile.kind.as_str() {
            "latest-release" => match latest_release {
                Some(v) => v.to_string(),
                None => continue,
            },
            "latest-snapshot" => continue,
            _ => profile.last_version_id.clone(),
        };
        if version_id.is_empty() {
            continue;
        }
        let json = std::fs::read(root.join("versions").join(&version_id).join(format!("{version_id}.json")))
            .ok()
            .and_then(|b| serde_json::from_slice::<VersionJson>(&b).ok());
        let minecraft = json.and_then(|j| j.inherits_from).unwrap_or_else(|| version_id.clone());
        let (loader, loader_version) = loader_from_version_id(&version_id, &minecraft);
        let game_dir = profile.game_dir.map(PathBuf::from).filter(|d| d.is_dir()).unwrap_or_else(|| root.to_path_buf());
        let name = if profile.name.trim().is_empty() {
            format!("Minecraft {minecraft}")
        } else {
            profile.name.trim().to_string()
        };
        out.push(entry(ExternalSource::Official, name, minecraft, loader, loader_version, &game_dir, parse_date(profile.last_used.as_deref())));
    }
    out
}

fn entry(
    source: ExternalSource,
    name: String,
    minecraft_version: String,
    loader: LoaderKind,
    loader_version: Option<String>,
    game_dir: &Path,
    last_played: Option<i64>,
) -> ExternalInstance {
    let source_key = serde_json::to_value(source).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    ExternalInstance {
        // One separator style, so the id stays the same however the path was built.
        id: format!("{source_key}:{}", game_dir.display().to_string().replace('\\', "/")),
        source,
        name,
        minecraft_version,
        loader,
        loader_version,
        game_dir: game_dir.display().to_string(),
        mods: count_mods(game_dir),
        worlds: count_worlds(game_dir),
        last_played,
        already_imported: false,
    }
}

fn subdirs(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root).map(|e| e.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
}

fn prism(root: &Path) -> Vec<ExternalInstance> {
    subdirs(root)
        .into_iter()
        .filter_map(|dir| {
            let pack = std::fs::read_to_string(dir.join("mmc-pack.json")).ok()?;
            let cfg = std::fs::read_to_string(dir.join("instance.cfg")).unwrap_or_default();
            let meta = super::import::parse_prism_meta(&pack, &cfg).ok()?;
            let game_dir = [".minecraft", "minecraft"].iter().map(|d| dir.join(d)).find(|d| d.is_dir())?;
            let name = meta.name.unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into_owned());
            Some(entry(
                ExternalSource::Prism,
                name,
                meta.minecraft_version,
                meta.loader,
                meta.loader_version,
                &game_dir,
                meta.last_launch_ms.map(|ms| ms / 1000),
            ))
        })
        .collect()
}

#[derive(Deserialize)]
struct CurseForgeInstance {
    #[serde(default)]
    name: String,
    #[serde(rename = "gameVersion", default)]
    game_version: String,
    #[serde(rename = "baseModLoader", default)]
    base_mod_loader: Option<CurseForgeLoader>,
    #[serde(rename = "lastPlayed", default)]
    last_played: Option<String>,
}

#[derive(Deserialize)]
struct CurseForgeLoader {
    #[serde(default)]
    name: String,
}

fn curseforge(root: &Path) -> Vec<ExternalInstance> {
    subdirs(root)
        .into_iter()
        .filter_map(|dir| {
            let bytes = std::fs::read(dir.join("minecraftinstance.json")).ok()?;
            let info: CurseForgeInstance = serde_json::from_slice(&bytes).ok()?;
            if info.game_version.is_empty() {
                return None;
            }
            let (loader, loader_version) = info
                .base_mod_loader
                .as_ref()
                .and_then(|l| l.name.split_once('-'))
                .and_then(|(kind, version)| {
                    let kind = LoaderKind::from_name(kind)?;
                    let version = version.strip_suffix(&format!("-{}", info.game_version)).unwrap_or(version);
                    Some((kind, Some(version.to_string())))
                })
                .unwrap_or((LoaderKind::Vanilla, None));
            let name = if info.name.is_empty() { dir.file_name()?.to_string_lossy().into_owned() } else { info.name };
            Some(entry(
                ExternalSource::Curseforge,
                name,
                info.game_version,
                loader,
                loader_version,
                &dir,
                parse_date(info.last_played.as_deref()),
            ))
        })
        .collect()
}

#[derive(Deserialize)]
struct ModrinthProfile {
    metadata: ModrinthMetadata,
}

#[derive(Deserialize)]
struct ModrinthMetadata {
    #[serde(default)]
    name: String,
    game_version: String,
    #[serde(default)]
    loader: String,
    #[serde(default)]
    loader_version: Option<ModrinthLoaderVersion>,
    #[serde(default)]
    last_played: Option<String>,
}

#[derive(Deserialize)]
struct ModrinthLoaderVersion {
    id: String,
}

fn modrinth(root: &Path) -> Vec<ExternalInstance> {
    // Older versions wrote a profile.json in each profile folder…
    let mut found: Vec<ExternalInstance> = subdirs(root)
        .into_iter()
        .filter_map(|dir| {
            let bytes = std::fs::read(dir.join("profile.json")).ok()?;
            let profile: ModrinthProfile = serde_json::from_slice(&bytes).ok()?;
            let m = profile.metadata;
            let loader = LoaderKind::from_name(&m.loader).unwrap_or(LoaderKind::Vanilla);
            let loader_version = (loader != LoaderKind::Vanilla).then(|| m.loader_version.map(|v| v.id)).flatten();
            let name = if m.name.is_empty() { dir.file_name()?.to_string_lossy().into_owned() } else { m.name };
            Some(entry(ExternalSource::Modrinth, name, m.game_version, loader, loader_version, &dir, parse_date(m.last_played.as_deref())))
        })
        .collect();
    // …current ones keep everything in app.db, next to the profiles folder.
    let Some(db) = root.parent().map(|data| data.join("app.db")) else { return found };
    for profile in modrinth_app::read_profiles(&db) {
        let dir = root.join(&profile.path);
        if !dir.is_dir() || !crate::util::fs::is_plain_file_name(&profile.path) {
            continue;
        }
        let loader = LoaderKind::from_name(&profile.loader).unwrap_or(LoaderKind::Vanilla);
        let loader_version = profile.loader_version.filter(|_| loader != LoaderKind::Vanilla);
        let item = entry(ExternalSource::Modrinth, profile.name, profile.game_version, loader, loader_version, &dir, profile.last_played);
        if !found.iter().any(|f| f.id == item.id) {
            found.push(item);
        }
    }
    found
}

/// Every instance found in other launchers, most recently played first.
/// `imported` = [`Instance::imported_from`] of existing instances.
pub fn detect(roots: &Roots, latest_release: Option<&str>, imported: &[String]) -> Vec<ExternalInstance> {
    let mut found: Vec<ExternalInstance> = Vec::new();
    for root in &roots.official {
        found.extend(official(root, latest_release));
    }
    for root in &roots.prism {
        found.extend(prism(root));
    }
    for root in &roots.curseforge {
        found.extend(curseforge(root));
    }
    for root in &roots.modrinth {
        found.extend(modrinth(root));
    }
    // Several official profiles can share one game folder: keep one.
    found.sort_by_key(|e| std::cmp::Reverse(e.last_played.unwrap_or(0)));
    let mut seen = std::collections::HashSet::new();
    found.retain(|e| seen.insert(e.id.clone()));
    for e in &mut found {
        e.already_imported = imported.contains(&e.id);
    }
    found
}

fn copy_item(src: &Path, dest: &Path) -> AppResult<()> {
    if src.is_dir() {
        crate::util::fs::copy_tree(src, dest)
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dest)?;
        Ok(())
    }
}

/// Creates a Largy instance from `external`: same version and loader, its
/// mods, configs, packs and options — and worlds when `include_worlds`.
pub fn import(paths: &AppPaths, external: &ExternalInstance, include_worlds: bool) -> AppResult<Instance> {
    let source = PathBuf::from(&external.game_dir);
    if !source.is_dir() {
        return Err(AppError::Instance(format!("dossier introuvable : {}", external.game_dir)));
    }
    let name: String = external.name.chars().take(super::MAX_NAME_LEN).collect();
    let created = super::create(
        paths,
        CreateInstanceInput {
            name: super::validate_name(&name).unwrap_or_else(|_| "Instance importée".to_string()),
            minecraft_version: external.minecraft_version.clone(),
            loader: external.loader,
            loader_version: external.loader_version.clone(),
            modpack: None,
            icon_url: None,
        },
    )?;
    let result = (|| -> AppResult<()> {
        let folders = FOLDERS.iter().chain(include_worlds.then_some(&"saves"));
        for item in folders.chain(FILES.iter()) {
            let src = source.join(item);
            if src.exists() {
                copy_item(&src, &created.directory.join(item))?;
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&created.directory);
        return Err(e);
    }
    super::update(paths, &created.id, |i| {
        i.imported_from = Some(external.id.clone());
        Ok(())
    })
}

mod modrinth_app;

#[cfg(test)]
mod tests;
