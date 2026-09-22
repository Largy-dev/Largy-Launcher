use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::download::DownloadItem;
use crate::error::{AppError, AppResult};
use crate::instances::{self, CreateInstanceInput, Instance, ModpackRef};
use crate::providers::{FileDownloadInfo, LoaderKind, ModpackFileRef, ModpackProvider};
use crate::state::AppState;

/// Result of an instance install/update: the instance itself, plus any
/// per-file problems that didn't stop the install but left it incomplete
/// (manual-download-required mods, files whose download URL couldn't be
/// resolved) — surfaced to the frontend instead of only logged.
#[derive(Debug, Serialize)]
pub struct InstanceInstallResult {
    pub instance: Instance,
    pub warnings: Vec<String>,
}

/// Runs a blocking, `'static`-owned closure on Tokio's blocking thread pool
/// and flattens the `JoinError` into an `AppError` — used for filesystem
/// work (recursive copies, zip reads) that would otherwise stall the async
/// runtime for as long as it takes to walk a modpack's override tree.
async fn spawn_blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("tâche de fond interrompue: {e}")))?
}

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
) -> AppResult<InstanceInstallResult> {
    let provider_ref = state
        .providers
        .get(&provider)
        .ok_or_else(|| AppError::Provider(format!("provider inconnu: {provider}")))?;

    let resolved = provider_ref.resolve_version(&pack_id, &version_id).await?;

    let paths = state.paths.clone();
    let create_input = CreateInstanceInput {
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
    };
    let instance = spawn_blocking(move || instances::create(&paths, create_input)).await?;

    let (items, warnings) = resolve_download_items(provider_ref, &resolved.files, &instance.directory).await;

    // task_id is the instance id itself, so the frontend can match a
    // download-progress event back to the specific instance card that's
    // currently installing (several installs could otherwise share the
    // same generic task name and be indistinguishable in the UI).
    state
        .downloader
        .run_batch(&app, &instance.id, "Fichiers du modpack", items, 8)
        .await
        .map_err(|e| AppError::Download(format!("instance \"{}\": {e}", instance.name)))?;

    if let Some(overrides_dir) = resolved.overrides_dir.clone() {
        let dest = instance.directory.clone();
        spawn_blocking(move || copy_dir_recursive(&overrides_dir, &dest)).await?;
    }

    Ok(InstanceInstallResult { instance, warnings })
}

/// Resolves every modpack file's real download URL against `provider`,
/// splitting the results into ready-to-download items and human-readable
/// warnings for files that need manual download or failed to resolve. Split
/// out from [`instances_install_modpack`] so the warnings-aggregation logic
/// is testable against a fake provider instead of a real network call.
async fn resolve_download_items(
    provider: &dyn ModpackProvider,
    files: &[ModpackFileRef],
    instance_dir: &Path,
) -> (Vec<DownloadItem>, Vec<String>) {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for file in files {
        match provider.resolve_file_download(file).await {
            Ok(FileDownloadInfo::Direct { url }) => items.push(DownloadItem {
                url,
                dest: instance_dir.join(&file.path),
                sha1: file.sha1.clone(),
                size: (file.size > 0).then_some(file.size),
            }),
            Ok(FileDownloadInfo::ManualRequired { browser_url, expected_filename }) => {
                let message = format!(
                    "{}: téléchargement manuel requis ({browser_url}, attendu: {expected_filename})",
                    file.path.display()
                );
                tracing::warn!("{message}");
                warnings.push(message);
            }
            Err(e) => {
                let message = format!("échec de résolution du fichier {}: {e}", file.path.display());
                tracing::warn!("{message}");
                warnings.push(message);
            }
        }
    }
    (items, warnings)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ModpackDetails, ModpackSummary, ModpackVersionSummary, ProviderError, ResolvedModpackVersion, SearchQuery};
    use async_trait::async_trait;
    use std::collections::HashMap;

    #[derive(Clone)]
    enum MockOutcome {
        Direct(String),
        ManualRequired { browser_url: String, expected_filename: String },
        Failed(String),
    }

    struct MockProvider {
        outcomes: HashMap<String, MockOutcome>,
    }

    #[async_trait]
    impl ModpackProvider for MockProvider {
        fn id(&self) -> &'static str {
            "mock"
        }
        fn display_name(&self) -> &'static str {
            "Mock"
        }
        async fn search(&self, _query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError> {
            unimplemented!("not exercised by these tests")
        }
        async fn get_modpack(&self, _pack_id: &str) -> Result<ModpackDetails, ProviderError> {
            unimplemented!("not exercised by these tests")
        }
        async fn get_versions(&self, _pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
            unimplemented!("not exercised by these tests")
        }
        async fn resolve_version(
            &self,
            _pack_id: &str,
            _version_id: &str,
        ) -> Result<ResolvedModpackVersion, ProviderError> {
            unimplemented!("not exercised by these tests")
        }
        async fn resolve_file_download(&self, file: &ModpackFileRef) -> Result<FileDownloadInfo, ProviderError> {
            match self.outcomes.get(&file.file_id) {
                Some(MockOutcome::Direct(url)) => Ok(FileDownloadInfo::Direct { url: url.clone() }),
                Some(MockOutcome::ManualRequired { browser_url, expected_filename }) => {
                    Ok(FileDownloadInfo::ManualRequired {
                        browser_url: browser_url.clone(),
                        expected_filename: expected_filename.clone(),
                    })
                }
                Some(MockOutcome::Failed(message)) => Err(ProviderError::Other(message.clone())),
                None => panic!("unexpected file id in test: {}", file.file_id),
            }
        }
    }

    fn file_ref(file_id: &str, path: &str) -> ModpackFileRef {
        ModpackFileRef {
            project_id: "proj".to_string(),
            file_id: file_id.to_string(),
            path: std::path::PathBuf::from(path),
            sha1: None,
            size: 0,
            direct_url: None,
        }
    }

    #[tokio::test]
    async fn resolve_download_items_collects_direct_downloads_and_skips_the_rest() {
        let provider = MockProvider {
            outcomes: HashMap::from([
                ("ok".to_string(), MockOutcome::Direct("https://example.com/a.jar".to_string())),
                (
                    "manual".to_string(),
                    MockOutcome::ManualRequired {
                        browser_url: "https://example.com/b".to_string(),
                        expected_filename: "b.jar".to_string(),
                    },
                ),
                ("broken".to_string(), MockOutcome::Failed("boom".to_string())),
            ]),
        };
        let files = vec![file_ref("ok", "mods/a.jar"), file_ref("manual", "mods/b.jar"), file_ref("broken", "mods/c.jar")];

        let (items, warnings) = resolve_download_items(&provider, &files, Path::new("/instance")).await;

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://example.com/a.jar");
        assert!(items[0].dest.ends_with("mods/a.jar"));

        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("mods/b.jar") && warnings[0].contains("téléchargement manuel"));
        assert!(warnings[1].contains("mods/c.jar") && warnings[1].contains("boom"));
    }

    #[tokio::test]
    async fn resolve_download_items_returns_no_warnings_when_everything_resolves() {
        let provider = MockProvider {
            outcomes: HashMap::from([("ok".to_string(), MockOutcome::Direct("https://example.com/a.jar".to_string()))]),
        };
        let files = vec![file_ref("ok", "mods/a.jar")];

        let (items, warnings) = resolve_download_items(&provider, &files, Path::new("/instance")).await;

        assert_eq!(items.len(), 1);
        assert!(warnings.is_empty());
    }
}
