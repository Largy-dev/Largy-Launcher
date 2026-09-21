use tauri::{AppHandle, State};

use crate::download::DownloadItem;
use crate::error::{AppError, AppResult};
use crate::instances::{self, CreateInstanceInput, Instance, ModpackRef};
use crate::providers::{FileDownloadInfo, LoaderKind};
use crate::state::AppState;

#[tauri::command]
pub fn instances_list(state: State<'_, AppState>) -> AppResult<Vec<Instance>> {
    instances::list(&state.paths)
}

#[tauri::command]
pub fn instances_get(state: State<'_, AppState>, id: String) -> AppResult<Instance> {
    instances::get(&state.paths, &id)
}

#[tauri::command]
pub fn instances_create(
    state: State<'_, AppState>,
    name: String,
    minecraft_version: String,
    loader: LoaderKind,
    loader_version: Option<String>,
) -> AppResult<Instance> {
    instances::create(
        &state.paths,
        CreateInstanceInput {
            name,
            minecraft_version,
            loader,
            loader_version,
            modpack: None,
            icon_url: None,
        },
    )
}

#[tauri::command]
pub fn instances_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    instances::delete(&state.paths, &id)
}

#[tauri::command]
pub fn instances_update_settings(
    state: State<'_, AppState>,
    id: String,
    min_memory_mb: Option<u32>,
    max_memory_mb: Option<u32>,
    extra_jvm_args: Vec<String>,
) -> AppResult<Instance> {
    let mut instance = instances::get(&state.paths, &id)?;
    instance.min_memory_mb = min_memory_mb;
    instance.max_memory_mb = max_memory_mb;
    instance.extra_jvm_args = extra_jvm_args;
    instances::save(&instance)?;
    Ok(instance)
}

#[tauri::command]
pub fn instances_open_folder(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let instance = instances::get(&state.paths, &id)?;
    std::fs::create_dir_all(&instance.directory)?;

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(&instance.directory).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&instance.directory).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&instance.directory).spawn();
    }

    Ok(())
}

/// Resolves a provider's modpack version, creates a fresh instance for it,
/// downloads every mod file, and copies any bundled config/scripts overrides.
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
) -> AppResult<Instance> {
    let provider_ref = state
        .providers
        .get(&provider)
        .ok_or_else(|| AppError::Provider(format!("provider inconnu: {provider}")))?;

    let resolved = provider_ref.resolve_version(&pack_id, &version_id).await?;

    let instance = instances::create(
        &state.paths,
        CreateInstanceInput {
            name: instance_name,
            minecraft_version: resolved.minecraft_version.clone(),
            loader: resolved.loader,
            loader_version: (!resolved.loader_version.is_empty()).then(|| resolved.loader_version.clone()),
            modpack: Some(ModpackRef {
                provider: provider.clone(),
                pack_id: pack_id.clone(),
                version_id: version_id.clone(),
                pack_name,
            }),
            icon_url: pack_icon_url,
        },
    )?;

    let mut items = Vec::new();
    for file in &resolved.files {
        match provider_ref.resolve_file_download(file).await {
            Ok(FileDownloadInfo::Direct { url }) => items.push(DownloadItem {
                url,
                dest: instance.directory.join(&file.path),
                sha1: file.sha1.clone(),
                size: (file.size > 0).then_some(file.size),
            }),
            Ok(FileDownloadInfo::ManualRequired { browser_url, expected_filename }) => {
                tracing::warn!(
                    "{}: téléchargement manuel requis ({browser_url}, attendu: {expected_filename})",
                    file.path.display()
                );
            }
            Err(e) => tracing::warn!("échec de résolution du fichier {}: {e}", file.path.display()),
        }
    }

    // task_id is the instance id itself, so the frontend can match a
    // download-progress event back to the specific instance card that's
    // currently installing (several installs could otherwise share the
    // same generic task name and be indistinguishable in the UI).
    state
        .downloader
        .run_batch(&app, &instance.id, "Fichiers du modpack", items, 8)
        .await?;

    if let Some(overrides_dir) = &resolved.overrides_dir {
        copy_dir_recursive(overrides_dir, &instance.directory)?;
    }

    Ok(instance)
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> AppResult<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
