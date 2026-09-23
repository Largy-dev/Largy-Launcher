//! Modpack install, update, import (.mrpack / CurseForge zip / Prism),
//! export (.mrpack) and world backups.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::download::DownloadItem;
use crate::error::{AppError, AppResult};
use crate::instances::{self, backup, export, import, CreateInstanceInput, Instance, ModpackRef};
use crate::providers::curseforge::CurseForgeProvider;
use crate::providers::modrinth::{self, ModrinthApi};
use crate::providers::{FileDownloadInfo, InstallWarning, ModpackProvider, ResolvedModpackVersion};
use crate::state::{cancellable, AppState};
use crate::util::fs::copy_dir_recursive;

use super::instances::{ensure_not_running, instances_changed, spawn_blocking};

/// Result of an instance install/update: the instance itself, plus any
/// per-file problems that didn't stop the install but left it incomplete.
#[derive(Debug, Serialize)]
pub struct InstanceInstallResult {
    pub instance: Instance,
    pub warnings: Vec<InstallWarning>,
}

/// Files a player customises in-game: a modpack *update* never overwrites
/// them once they exist (a fresh install still gets the pack's defaults).
const USER_FILES: &[&str] = &["options.txt", "optionsof.txt", "optionsshaders.txt", "servers.dat", "servers.dat_old"];

/// Downloads every file of a resolved modpack version into `instance_dir`,
/// then copies its override folders. Returns every placed file's
/// instance-relative path and per-file warnings. Individual failures don't
/// abort the install — they become warnings.
async fn download_and_track_files(
    app: &AppHandle,
    state: &AppState,
    provider: Option<&dyn ModpackProvider>,
    resolved: &ResolvedModpackVersion,
    instance_id: &str,
    instance_dir: &Path,
    preserve_user_files: bool,
) -> AppResult<(Vec<PathBuf>, Vec<InstallWarning>)> {
    let (items, mut warnings) = resolve_download_items(provider, resolved, instance_dir).await;
    let failures = state
        .downloader
        .run_batch_lenient(app, instance_id, "Fichiers du modpack", items.clone(), 8)
        .await;
    let failed: HashSet<PathBuf> = failures.iter().map(|(item, _)| item.dest.clone()).collect();
    for (item, error) in failures {
        warnings.push(InstallWarning {
            file_name: item.dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            message: format!("Téléchargement impossible : {error}"),
            browser_url: None,
        });
    }
    let mut installed_files: Vec<PathBuf> = items
        .iter()
        .filter(|i| !failed.contains(&i.dest))
        .filter_map(|i| i.dest.strip_prefix(instance_dir).ok().map(Path::to_path_buf))
        .collect();

    let overrides = resolved.overrides_dirs.clone();
    let dest = instance_dir.to_path_buf();
    let copied = spawn_blocking(move || {
        let mut all = Vec::new();
        for dir in overrides {
            all.extend(copy_dir_recursive(&dir, &dest, |rel| {
                preserve_user_files && rel.to_str().is_some_and(|r| USER_FILES.contains(&r))
            })?);
        }
        Ok(all)
    })
    .await?;
    installed_files.extend(copied);
    installed_files.sort();
    installed_files.dedup();

    Ok((installed_files, warnings))
}

async fn resolve_download_items(
    provider: Option<&dyn ModpackProvider>,
    resolved: &ResolvedModpackVersion,
    instance_dir: &Path,
) -> (Vec<DownloadItem>, Vec<InstallWarning>) {
    let mut items = Vec::new();
    let mut warnings = resolved.warnings.clone();
    for file in &resolved.files {
        let Some(dest) = crate::util::fs::safe_join(instance_dir, &file.path) else {
            continue;
        };
        let info = match provider {
            Some(p) => p.resolve_file_download(file).await,
            None => match &file.direct_url {
                Some(url) => Ok(FileDownloadInfo::Direct { url: url.clone() }),
                None => Err(crate::providers::ProviderError::Other("URL manquante".to_string())),
            },
        };
        match info {
            Ok(FileDownloadInfo::Direct { url }) => items.push(DownloadItem {
                url,
                dest,
                sha1: file.sha1.clone(),
                size: (file.size > 0).then_some(file.size),
            }),
            Ok(FileDownloadInfo::ManualRequired { browser_url, expected_filename }) => warnings.push(InstallWarning {
                file_name: expected_filename,
                message: "L'auteur a désactivé le téléchargement automatique pour ce fichier.".to_string(),
                browser_url: Some(browser_url),
            }),
            Err(e) => warnings.push(InstallWarning {
                file_name: file.path.display().to_string(),
                message: e.to_string(),
                browser_url: None,
            }),
        }
    }
    (items, warnings)
}

fn diff_stale_files(old_installed: &[PathBuf], new_installed: &[PathBuf]) -> Vec<PathBuf> {
    let new_set: HashSet<&PathBuf> = new_installed.iter().collect();
    old_installed.iter().filter(|p| !new_set.contains(p)).cloned().collect()
}

/// Creates a fresh instance for `resolved`, downloads everything into it,
/// and removes it again if the install fails or is cancelled.
async fn install_into_new_instance(
    app: &AppHandle,
    state: &AppState,
    provider: Option<&dyn ModpackProvider>,
    resolved: ResolvedModpackVersion,
    input: CreateInstanceInput,
    extra_dir: Option<PathBuf>,
) -> AppResult<InstanceInstallResult> {
    let paths = state.paths.clone();
    let mut instance = spawn_blocking(move || instances::create(&paths, input)).await?;
    instances_changed(app);

    let result = async {
        let guard = state.begin_install(&instance.id)?;
        let (installed_files, warnings) = cancellable(
            &guard.cancel,
            download_and_track_files(app, state, provider, &resolved, &instance.id, &instance.directory, false),
        )
        .await?;
        if let Some(extra) = extra_dir {
            let dest = instance.directory.clone();
            spawn_blocking(move || copy_dir_recursive(&extra, &dest, |_| false).map(|_| ())).await?;
        }
        Ok::<_, AppError>((installed_files, warnings))
    }
    .await;

    match result {
        Ok((installed_files, warnings)) => {
            if let Some(modpack) = &mut instance.modpack {
                modpack.installed_files = installed_files;
            }
            let to_save = instance.clone();
            spawn_blocking(move || instances::save(&to_save)).await?;
            instances_changed(app);
            Ok(InstanceInstallResult { instance, warnings })
        }
        Err(e) => {
            let dir = instance.directory.clone();
            let _ = spawn_blocking(move || std::fs::remove_dir_all(dir).map_err(AppError::from)).await;
            instances_changed(app);
            Err(e)
        }
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn instances_install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    pack_id: String,
    version_id: String,
    pack_name: String,
    pack_icon_url: Option<String>,
    instance_name: String,
) -> AppResult<InstanceInstallResult> {
    let name = instances::validate_name(&instance_name)?;
    let provider_ref = state
        .providers
        .get(&provider)
        .ok_or_else(|| AppError::Provider(format!("provider inconnu: {provider}")))?;
    let resolved = provider_ref.resolve_version(&pack_id, &version_id).await?;

    let input = CreateInstanceInput {
        name,
        minecraft_version: resolved.minecraft_version.clone(),
        loader: resolved.loader,
        loader_version: (!resolved.loader_version.is_empty()).then(|| resolved.loader_version.clone()),
        modpack: Some(ModpackRef { provider, pack_id, version_id, pack_name, installed_files: Vec::new() }),
        icon_url: pack_icon_url,
    };
    install_into_new_instance(&app, &state, Some(provider_ref), resolved, input, None).await
}

/// Cancels a modpack install/update in progress for `id`.
#[tauri::command]
pub fn instances_cancel_install(state: State<'_, AppState>, id: String) {
    if let Some(cancel) = state.installs.lock().get(&id) {
        cancel.notify_one();
    }
}

/// Installs another version of a modpack into an *existing* instance:
/// worlds are backed up first, identity/settings are kept, player-edited
/// files (options, server list) are preserved, and files the previous
/// version installed but the new one doesn't ship are removed.
#[tauri::command]
pub async fn instances_update_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    version_id: String,
) -> AppResult<InstanceInstallResult> {
    ensure_not_running(&state, &instance_id)?;
    let guard = state.begin_install(&instance_id)?;
    let paths = state.paths.clone();
    let id = instance_id.clone();
    let mut instance = spawn_blocking(move || instances::get(&paths, &id)).await?;

    let modpack = instance
        .modpack
        .clone()
        .ok_or_else(|| AppError::Instance("cette instance n'est pas un modpack installé".to_string()))?;
    let provider_ref = state
        .providers
        .get(&modpack.provider)
        .ok_or_else(|| AppError::Provider(format!("provider inconnu: {}", modpack.provider)))?;

    let resolved = cancellable(&guard.cancel, async {
        Ok(provider_ref.resolve_version(&modpack.pack_id, &version_id).await?)
    })
    .await?;

    let (paths, id, dir) = (state.paths.clone(), instance.id.clone(), instance.directory.clone());
    spawn_blocking(move || backup::backup_saves(&paths, &id, &dir)).await?;

    let (new_installed_files, warnings) = cancellable(
        &guard.cancel,
        download_and_track_files(&app, &state, Some(provider_ref), &resolved, &instance.id, &instance.directory, true),
    )
    .await?;

    let stale = diff_stale_files(&modpack.installed_files, &new_installed_files);
    let instance_dir = instance.directory.clone();
    spawn_blocking(move || {
        for rel in &stale {
            if let Some(path) = crate::util::fs::safe_join(&instance_dir, rel) {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    })
    .await?;

    instance.minecraft_version = resolved.minecraft_version.clone();
    instance.loader = resolved.loader;
    instance.loader_version = (!resolved.loader_version.is_empty()).then(|| resolved.loader_version.clone());
    instance.modpack = Some(ModpackRef { version_id, installed_files: new_installed_files, ..modpack });

    let to_save = instance.clone();
    spawn_blocking(move || instances::save(&to_save)).await?;
    instances_changed(&app);
    Ok(InstanceInstallResult { instance, warnings })
}

/// Creates an instance from a local `.mrpack`, CurseForge zip or Prism /
/// MultiMC export.
#[tauri::command]
pub async fn instances_import(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<InstanceInstallResult> {
    let source = PathBuf::from(&path);
    let kind = {
        let source = source.clone();
        spawn_blocking(move || import::detect(&source)).await?
    };
    let file_stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let extract_dir = state.paths.cache_dir().join("imports").join(uuid::Uuid::new_v4().simple().to_string());

    let result = match kind {
        import::ImportKind::Mrpack => {
            let resolved = modrinth::resolve_mrpack(&source, &extract_dir).await?;
            import_resolved(&app, &state, resolved, &file_stem).await
        }
        import::ImportKind::CurseForge => {
            let provider = CurseForgeProvider::new(
                state.client.clone(),
                state.curseforge_api_key.clone(),
                state.paths.cache_dir().join("curseforge"),
            );
            let resolved = provider.resolve_zip(&source, &extract_dir).await?;
            let name = resolved.pack_name.clone().unwrap_or_else(|| file_stem.clone());
            let input = new_instance_input(&name, &resolved)?;
            install_into_new_instance(&app, &state, Some(&provider), resolved, input, None).await
        }
        import::ImportKind::Prism { root } => {
            let parsed = {
                let source = source.clone();
                spawn_blocking(move || import::parse_prism(&source, &root)).await?
            };
            let game_dir = {
                let (source, prefix, dest) = (source.clone(), parsed.game_dir_prefix.clone(), extract_dir.clone());
                spawn_blocking(move || import::extract_prism_game_dir(&source, &prefix, &dest)).await?
            };
            let resolved = ResolvedModpackVersion {
                minecraft_version: parsed.minecraft_version,
                loader: parsed.loader,
                loader_version: parsed.loader_version.unwrap_or_default(),
                files: Vec::new(),
                overrides_dirs: Vec::new(),
                warnings: Vec::new(),
                pack_name: parsed.name,
            };
            let name = resolved.pack_name.clone().unwrap_or_else(|| file_stem.clone());
            let mut input = new_instance_input(&name, &resolved)?;
            input.modpack = None;
            let mut result = install_into_new_instance(&app, &state, None, resolved, input, game_dir).await?;
            if parsed.max_memory_mb.is_some() {
                result.instance.min_memory_mb = parsed.min_memory_mb;
                result.instance.max_memory_mb = parsed.max_memory_mb;
                instances::save(&result.instance)?;
            }
            Ok(result)
        }
    };
    let _ = std::fs::remove_dir_all(&extract_dir);
    result
}

fn new_instance_input(name: &str, resolved: &ResolvedModpackVersion) -> AppResult<CreateInstanceInput> {
    let name: String = name.chars().take(instances::MAX_NAME_LEN).collect();
    Ok(CreateInstanceInput {
        name: instances::validate_name(&name).unwrap_or_else(|_| "Instance importée".to_string()),
        minecraft_version: resolved.minecraft_version.clone(),
        loader: resolved.loader,
        loader_version: (!resolved.loader_version.is_empty()).then(|| resolved.loader_version.clone()),
        modpack: None,
        icon_url: None,
    })
}

async fn import_resolved(
    app: &AppHandle,
    state: &AppState,
    resolved: ResolvedModpackVersion,
    fallback_name: &str,
) -> AppResult<InstanceInstallResult> {
    let name = resolved.pack_name.clone().unwrap_or_else(|| fallback_name.to_string());
    let input = new_instance_input(&name, &resolved)?;
    install_into_new_instance(app, state, None, resolved, input, None).await
}

#[tauri::command]
pub async fn instances_export(
    state: State<'_, AppState>,
    id: String,
    dest: String,
    include_saves: bool,
) -> AppResult<export::ExportSummary> {
    let instance = instances::get(&state.paths, &id)?;
    let mut dest = PathBuf::from(dest);
    if dest.extension().is_none_or(|e| e != "mrpack") {
        dest.set_extension("mrpack");
    }
    export::export_mrpack(&ModrinthApi::new(state.client.clone()), &instance, &dest, include_saves).await
}

#[tauri::command]
pub async fn instances_backup_worlds(state: State<'_, AppState>, id: String) -> AppResult<Option<String>> {
    let instance = instances::get(&state.paths, &id)?;
    let paths = state.paths.clone();
    let backup = spawn_blocking(move || backup::backup_saves(&paths, &instance.id, &instance.directory)).await?;
    Ok(backup.map(|p| p.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{LoaderKind, ModpackFileRef};

    fn file_ref(path: &str, url: Option<&str>) -> ModpackFileRef {
        ModpackFileRef {
            project_id: "p".to_string(),
            file_id: path.to_string(),
            path: PathBuf::from(path),
            sha1: None,
            size: 0,
            direct_url: url.map(str::to_string),
            browser_url: url.is_none().then(|| "https://example.com/manual".to_string()),
        }
    }

    fn resolved(files: Vec<ModpackFileRef>) -> ResolvedModpackVersion {
        ResolvedModpackVersion {
            minecraft_version: "1.20.1".to_string(),
            loader: LoaderKind::Vanilla,
            loader_version: String::new(),
            files,
            overrides_dirs: Vec::new(),
            warnings: vec![InstallWarning { file_name: "x".into(), message: "m".into(), browser_url: None }],
            pack_name: None,
        }
    }

    #[tokio::test]
    async fn resolve_download_items_splits_direct_and_manual_files_and_keeps_provider_warnings() {
        let r = resolved(vec![
            file_ref("mods/a.jar", Some("https://example.com/a.jar")),
            file_ref("mods/b.jar", None),
            file_ref("../escape.jar", Some("https://example.com/e.jar")),
        ]);
        let (items, warnings) = resolve_download_items(None, &r, Path::new("/instance")).await;

        assert_eq!(items.len(), 1);
        assert!(items[0].dest.ends_with("mods/a.jar"));
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].file_name, "x");
    }

    #[test]
    fn diff_stale_files_flags_only_files_dropped_from_the_new_version() {
        let old = vec![PathBuf::from("mods/a.jar"), PathBuf::from("mods/b.jar"), PathBuf::from("config/x.toml")];
        let new = vec![PathBuf::from("mods/a.jar"), PathBuf::from("config/x.toml")];
        assert_eq!(diff_stale_files(&old, &new), vec![PathBuf::from("mods/b.jar")]);
        assert!(diff_stale_files(&new, &old).is_empty());
    }
}
