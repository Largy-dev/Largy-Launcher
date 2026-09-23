pub mod auth;
pub mod commands;
pub mod download;
pub mod error;
pub mod instances;
pub mod java;
pub mod launch;
pub mod minecraft;
pub mod modloaders;
pub mod paths;
pub mod process_ext;
pub mod providers;
pub mod settings;
pub mod state;
pub mod util;

use tauri::Manager;

use state::AppState;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Logs to `launcher-logs/launcher.log` (a GUI app has no console to read
/// stdout from), rotating the previous file to `launcher.old.log` once it
/// grows past [`MAX_LOG_BYTES`].
fn init_logging(paths: &paths::AppPaths) {
    use tracing_subscriber::fmt;
    let dir = paths.launcher_logs_dir();
    let file = std::fs::create_dir_all(&dir).ok().and_then(|_| {
        let path = dir.join("launcher.log");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
            let _ = std::fs::rename(&path, dir.join("launcher.old.log"));
        }
        std::fs::OpenOptions::new().create(true).append(true).open(path).ok()
    });
    let builder = fmt().with_ansi(false).with_max_level(tracing::Level::INFO);
    let _ = match file {
        Some(file) => builder.with_writer(std::sync::Mutex::new(file)).try_init(),
        None => builder.try_init(),
    };
    std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            init_logging(&paths::AppPaths::new(app.handle()));
            tracing::info!("Largy Launcher {} starting", env!("CARGO_PKG_VERSION"));
            let state = AppState::new(app.handle())?;
            app.manage(state);
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::system_memory_mb,
            commands::system_memory_info,
            commands::loaders_list_versions,
            commands::java_list_installations,
            commands::java_probe,
            commands::open_launcher_logs,
            commands::auth::auth_begin_login,
            commands::auth::auth_complete_login,
            commands::auth::auth_cancel_login,
            commands::auth::auth_try_silent_login,
            commands::auth::auth_switch_account,
            commands::auth::auth_list_accounts,
            commands::auth::auth_logout,
            commands::auth::auth_get_active_account,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::minecraft::minecraft_list_versions,
            commands::providers::providers_search,
            commands::providers::providers_get_modpack,
            commands::providers::providers_get_versions,
            commands::instances::instances_list,
            commands::instances::instances_get,
            commands::instances::instances_create,
            commands::instances::instances_delete,
            commands::instances::instances_rename,
            commands::instances::instances_duplicate,
            commands::instances::instances_update_settings,
            commands::instances::instances_open_folder,
            commands::instances::instances_reveal_file,
            commands::modpacks::instances_install_modpack,
            commands::modpacks::instances_cancel_install,
            commands::modpacks::instances_update_modpack,
            commands::modpacks::instances_import,
            commands::modpacks::instances_export,
            commands::modpacks::instances_backup_worlds,
            commands::mods::instance_mods_list,
            commands::mods::instance_mods_set_enabled,
            commands::mods::instance_mods_delete,
            commands::mods::instance_mods_add,
            commands::mods::instance_mods_check_updates,
            commands::mods::instance_mods_apply_updates,
            commands::mods::content_search,
            commands::mods::content_install,
            commands::launch::launch_instance,
            commands::launch::stop_instance,
            commands::launch::repair_instance,
            commands::launch::is_instance_running,
            commands::launch::instance_process_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
