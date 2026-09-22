use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::download::{DownloadItem, DownloadManager};
use crate::error::{AppError, AppResult};
use crate::instances::{self, CreateInstanceInput, Instance, ModpackRef};
use crate::providers::{FileDownloadInfo, LoaderKind, ModpackFileRef, ModpackProvider, ResolvedModpackVersion};
use crate::state::AppState;

/// One file that didn't make it into the instance automatically. Structured
/// (rather than a pre-formatted string) so the frontend can offer real
/// actions — open the download page, open the instance's mods folder —
/// instead of a one-shot toast the user loses once it's dismissed.
#[derive(Debug, Clone, Serialize)]
pub struct InstallWarning {
    pub file_name: String,
    pub message: String,
    pub browser_url: Option<String>,
}

/// Result of an instance install/update: the instance itself, plus any
/// per-file problems that didn't stop the install but left it incomplete
/// (manual-download-required mods, files whose download URL couldn't be
/// resolved) — surfaced to the frontend instead of only logged.
#[derive(Debug, Serialize)]
pub struct InstanceInstallResult {
    pub instance: Instance,
    pub warnings: Vec<InstallWarning>,
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

/// Downloads every file a resolved modpack version references (mods first,
/// then any `overrides/` tree) into `instance_dir`, returning every
/// successfully-placed file's instance-relative path plus any per-file
/// warnings. Shared by fresh installs and in-place updates so both track
/// "what this version put on disk" the same way.
async fn download_and_track_files(
    app: &AppHandle,
    downloader: &DownloadManager,
    provider: &dyn ModpackProvider,
    resolved: &ResolvedModpackVersion,
    instance_id: &str,
    instance_name: &str,
    instance_dir: &Path,
) -> AppResult<(Vec<PathBuf>, Vec<InstallWarning>)> {
    let (items, warnings) = resolve_download_items(provider, &resolved.files, instance_dir).await;
    let mut installed_files: Vec<PathBuf> =
        items.iter().filter_map(|i| i.dest.strip_prefix(instance_dir).ok().map(Path::to_path_buf)).collect();

    // task_id is the instance id itself, so the frontend can match a
    // download-progress event back to the specific instance card that's
    // currently installing (several installs could otherwise share the
    // same generic task name and be indistinguishable in the UI).
    downloader
        .run_batch(app, instance_id, "Fichiers du modpack", items, 8)
        .await
        .map_err(|e| AppError::Download(format!("instance \"{instance_name}\": {e}")))?;

    if let Some(overrides_dir) = resolved.overrides_dir.clone() {
        let dest = instance_dir.to_path_buf();
        let copied = spawn_blocking(move || copy_dir_recursive(&overrides_dir, &dest)).await?;
        installed_files.extend(copied);
    }

    Ok((installed_files, warnings))
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
            installed_files: Vec::new(),
        }),
        icon_url: pack_icon_url,
    };
    let mut instance = spawn_blocking(move || instances::create(&paths, create_input)).await?;

    let (installed_files, warnings) = download_and_track_files(
        &app,
        &state.downloader,
        provider_ref,
        &resolved,
        &instance.id,
        &instance.name,
        &instance.directory,
    )
    .await?;

    if let Some(modpack) = &mut instance.modpack {
        modpack.installed_files = installed_files;
    }
    let to_save = instance.clone();
    spawn_blocking(move || instances::save(&to_save)).await?;

    Ok(InstanceInstallResult { instance, warnings })
}

/// Installs a newer version of a modpack into an *existing* instance instead
/// of creating a new one: preserves the instance's identity, name, icon, and
/// per-instance RAM/JVM overrides, updates its Minecraft/loader version, and
/// removes any previously-tracked file the new version no longer ships (mods
/// and overrides alike). Files never tracked by a modpack install — saves,
/// manually-added mods — are never touched, since they were never in
/// `installed_files` to begin with.
#[tauri::command]
pub async fn instances_update_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    version_id: String,
) -> AppResult<InstanceInstallResult> {
    let paths = state.paths.clone();
    let id_for_get = instance_id.clone();
    let mut instance = spawn_blocking(move || instances::get(&paths, &id_for_get)).await?;

    let modpack = instance
        .modpack
        .clone()
        .ok_or_else(|| AppError::Instance("cette instance n'est pas un modpack installé".to_string()))?;

    let provider_ref = state
        .providers
        .get(&modpack.provider)
        .ok_or_else(|| AppError::Provider(format!("provider inconnu: {}", modpack.provider)))?;

    let resolved = provider_ref.resolve_version(&modpack.pack_id, &version_id).await?;

    let (new_installed_files, warnings) = download_and_track_files(
        &app,
        &state.downloader,
        provider_ref,
        &resolved,
        &instance.id,
        &instance.name,
        &instance.directory,
    )
    .await?;

    let stale = diff_stale_files(&modpack.installed_files, &new_installed_files);
    let instance_dir = instance.directory.clone();
    spawn_blocking(move || {
        for rel in &stale {
            let _ = std::fs::remove_file(instance_dir.join(rel));
        }
        Ok(())
    })
    .await?;

    instance.minecraft_version = resolved.minecraft_version.clone();
    instance.loader = resolved.loader;
    instance.loader_version = (!resolved.loader_version.is_empty()).then(|| resolved.loader_version.clone());
    instance.modpack = Some(ModpackRef {
        version_id,
        installed_files: new_installed_files,
        ..modpack
    });

    let to_save = instance.clone();
    spawn_blocking(move || instances::save(&to_save)).await?;

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
) -> (Vec<DownloadItem>, Vec<InstallWarning>) {
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
                tracing::warn!(
                    "{}: téléchargement manuel requis ({browser_url}, attendu: {expected_filename})",
                    file.path.display()
                );
                warnings.push(InstallWarning {
                    file_name: expected_filename,
                    message: "L'auteur a désactivé le téléchargement automatique pour ce fichier.".to_string(),
                    browser_url: Some(browser_url),
                });
            }
            Err(e) => {
                let message = e.to_string();
                tracing::warn!("échec de résolution du fichier {}: {message}", file.path.display());
                warnings.push(InstallWarning {
                    file_name: file.path.display().to_string(),
                    message,
                    browser_url: None,
                });
            }
        }
    }
    (items, warnings)
}

/// Files tracked by the *old* modpack version but absent from the *new*
/// one's tracked file list — safe to delete since they're either a stale mod
/// jar or a stale override/config file the new version no longer ships.
/// Anything never tracked in the first place (saves, manual additions) can
/// never appear here, since this only ever looks at the two tracked lists,
/// never the instance directory itself.
fn diff_stale_files(old_installed: &[PathBuf], new_installed: &[PathBuf]) -> Vec<PathBuf> {
    let new_set: HashSet<&PathBuf> = new_installed.iter().collect();
    old_installed.iter().filter(|p| !new_set.contains(p)).cloned().collect()
}

/// Copies `src`'s tree into `dest`, returning the destination-relative path
/// of every file copied (used to track which files a modpack version placed
/// on disk, for `instances_update_modpack`'s stale-file pruning).
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> AppResult<Vec<std::path::PathBuf>> {
    let mut copied = Vec::new();
    copy_dir_recursive_into(src, dest, dest, &mut copied)?;
    Ok(copied)
}

fn copy_dir_recursive_into(
    src: &std::path::Path,
    dest: &std::path::Path,
    root: &std::path::Path,
    copied: &mut Vec<std::path::PathBuf>,
) -> AppResult<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_dir_recursive_into(&entry.path(), &target, root, copied)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
            if let Ok(rel) = target.strip_prefix(root) {
                copied.push(rel.to_path_buf());
            }
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
        assert_eq!(warnings[0].file_name, "b.jar");
        assert_eq!(warnings[0].browser_url, Some("https://example.com/b".to_string()));
        assert_eq!(warnings[1].file_name, "mods/c.jar");
        assert!(warnings[1].message.contains("boom"));
        assert_eq!(warnings[1].browser_url, None);
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

    #[test]
    fn diff_stale_files_flags_files_dropped_from_the_new_version() {
        let old = vec![PathBuf::from("mods/a.jar"), PathBuf::from("mods/b.jar"), PathBuf::from("config/x.toml")];
        let new = vec![PathBuf::from("mods/a.jar"), PathBuf::from("config/x.toml")];

        let stale = diff_stale_files(&old, &new);

        assert_eq!(stale, vec![PathBuf::from("mods/b.jar")]);
    }

    #[test]
    fn diff_stale_files_never_flags_a_path_that_was_never_tracked() {
        // Nothing in `saves/` is ever added to `installed_files` in the first
        // place, so it structurally can never show up as "stale" here —
        // this is what keeps an update from ever touching player saves.
        let old = vec![PathBuf::from("mods/a.jar")];
        let new: Vec<PathBuf> = Vec::new();

        let stale = diff_stale_files(&old, &new);

        assert_eq!(stale, vec![PathBuf::from("mods/a.jar")]);
        assert!(!stale.contains(&PathBuf::from("saves/world1/level.dat")));
    }

    #[test]
    fn diff_stale_files_returns_nothing_when_everything_carries_over() {
        let old = vec![PathBuf::from("mods/a.jar")];
        let new = vec![PathBuf::from("mods/a.jar"), PathBuf::from("mods/b.jar")];

        assert!(diff_stale_files(&old, &new).is_empty());
    }

    #[test]
    fn copy_dir_recursive_returns_every_copied_files_relative_path() {
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src_dir.path().join("config")).unwrap();
        std::fs::write(src_dir.path().join("config/settings.toml"), b"x").unwrap();
        std::fs::write(src_dir.path().join("readme.txt"), b"x").unwrap();

        let mut copied = copy_dir_recursive(src_dir.path(), dest_dir.path()).unwrap();
        copied.sort();

        assert_eq!(copied, vec![PathBuf::from("config/settings.toml"), PathBuf::from("readme.txt")]);
        assert!(dest_dir.path().join("config/settings.toml").exists());
    }
}
