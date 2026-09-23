//! Thin `#[tauri::command]` handlers, one module per screen/domain. Each
//! handler just validates input and delegates into the matching top-level
//! module (`auth`, `instances`, `providers`, ...) — no business logic here.

pub mod auth;
pub mod instances;
pub mod launch;
pub mod minecraft;
pub mod modpacks;
pub mod mods;
pub mod providers;
pub mod settings;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::java::JavaInstallation;
use crate::providers::LoaderKind;
use crate::state::AppState;

#[tauri::command]
pub fn app_version() -> AppResult<String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

/// Total physical RAM in MB, used to bound the memory-allocation sliders.
#[tauri::command]
pub fn system_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

#[derive(serde::Serialize)]
pub struct SystemMemoryInfo {
    pub total_mb: u64,
    pub available_mb: u64,
}

/// Total and currently free physical RAM in MB, used by the RAM advice.
#[tauri::command]
pub fn system_memory_info() -> SystemMemoryInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    SystemMemoryInfo { total_mb: sys.total_memory() / 1024 / 1024, available_mb: sys.available_memory() / 1024 / 1024 }
}

#[tauri::command]
pub async fn loaders_list_versions(
    state: State<'_, AppState>,
    loader: LoaderKind,
    minecraft_version: String,
) -> AppResult<Vec<String>> {
    let installer = state
        .loaders
        .get(loader)
        .ok_or_else(|| AppError::Loader("mod loader non supporté".to_string()))?;
    installer.list_versions(&state.meta, &minecraft_version).await.map_err(AppError::from)
}

/// Every working Java found on this machine, plus the launcher's own.
#[tauri::command]
pub async fn java_list_installations(state: State<'_, AppState>) -> AppResult<Vec<JavaInstallation>> {
    Ok(crate::java::discover(&state.paths).await)
}

/// Version of the Java at `path`, or an error when it isn't a working Java.
#[tauri::command]
pub async fn java_probe(path: String) -> AppResult<JavaInstallation> {
    let (version, major) = crate::java::probe(std::path::Path::new(&path))
        .await
        .ok_or_else(|| AppError::Java(format!("{path} n'est pas un exécutable Java valide")))?;
    Ok(JavaInstallation { path, version, major, source: "system" })
}

/// Answer to the "close the window?" prompt: `tray` hides the window (the
/// launcher keeps running in the notification area), `quit` exits. With
/// `remember`, the choice becomes the setting and the prompt won't return.
#[tauri::command]
pub fn app_close_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: crate::settings::CloseBehavior,
    remember: bool,
) -> AppResult<()> {
    use crate::settings::CloseBehavior;
    use tauri::Manager;
    if remember && action != CloseBehavior::Ask {
        let mut settings = state.settings.read().clone();
        settings.on_close = action;
        settings.save(&state.paths)?;
        *state.settings.write() = settings;
    }
    match action {
        CloseBehavior::Quit => app.exit(0),
        _ => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
    }
    Ok(())
}

/// Opens the folder holding the launcher's own log files.
#[tauri::command]
pub fn open_launcher_logs(state: State<'_, AppState>) -> AppResult<()> {
    let dir = state.paths.launcher_logs_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    }
    Ok(())
}
