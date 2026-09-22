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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::system_memory_mb,
            commands::loaders_list_versions,
            commands::auth::auth_begin_login,
            commands::auth::auth_complete_login,
            commands::auth::auth_try_silent_login,
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
            commands::instances::instances_update_settings,
            commands::instances::instances_open_folder,
            commands::instances::instances_install_modpack,
            commands::instances::instances_update_modpack,
            commands::launch::launch_instance,
            commands::launch::stop_instance,
            commands::launch::is_instance_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
