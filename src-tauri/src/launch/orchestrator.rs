//! Cross-cutting glue: pulls together the active account, an instance's
//! resolved vanilla version, its mod loader (if any), and a matching Java
//! runtime, then hands the assembled command line to [`super::spawn`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as AsyncMutex;

use crate::auth::AccountSession;
use crate::error::{AppError, AppResult};
use crate::instances::{self, Instance};
use crate::minecraft::{self, launch_args::LaunchContext, libraries as mc_libraries};
use crate::providers::LoaderKind;
use crate::settings::GlobalSettings;
use crate::state::AppState;

use super::{InstanceExit, RunningChild};

/// Builds the `${...}`-style substitution map for a launch's JVM/game
/// arguments from the account, instance, resolved settings, and version
/// metadata involved. Split out from [`launch_instance`] so this assembly —
/// the most complex pure logic in the launch path — is testable without
/// spawning a real java process.
#[allow(clippy::too_many_arguments)]
fn build_placeholders(
    account: &AccountSession,
    instance: &Instance,
    settings: &GlobalSettings,
    asset_index_id: &str,
    natives_directory: &Path,
    assets_dir: &Path,
    libraries_dir: &Path,
) -> HashMap<String, String> {
    let mut placeholders = HashMap::new();
    placeholders.insert("auth_player_name".to_string(), account.profile.name.clone());
    placeholders.insert("auth_uuid".to_string(), account.profile.id.clone());
    placeholders.insert("auth_access_token".to_string(), account.minecraft_access_token.clone());
    placeholders.insert("auth_xuid".to_string(), account.profile.id.clone());
    placeholders.insert(
        "user_type".to_string(),
        (if settings.offline_mode { "legacy" } else { "msa" }).to_string(),
    );
    placeholders.insert("version_name".to_string(), instance.minecraft_version.clone());
    placeholders.insert("game_directory".to_string(), instance.directory.display().to_string());
    placeholders.insert("assets_root".to_string(), assets_dir.display().to_string());
    placeholders.insert("assets_index_name".to_string(), asset_index_id.to_string());
    placeholders.insert("version_type".to_string(), "release".to_string());
    placeholders.insert("user_properties".to_string(), "{}".to_string());
    placeholders.insert("clientid".to_string(), "-".to_string());
    placeholders.insert("launcher_name".to_string(), "LargyLauncher".to_string());
    placeholders.insert("launcher_version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    placeholders.insert("natives_directory".to_string(), natives_directory.display().to_string());
    placeholders.insert("library_directory".to_string(), libraries_dir.display().to_string());
    placeholders.insert(
        "classpath_separator".to_string(),
        if cfg!(windows) { ";" } else { ":" }.to_string(),
    );
    placeholders
}

pub async fn launch_instance(app: &AppHandle, state: &AppState, instance_id: &str) -> AppResult<()> {
    {
        let running = state.running.lock();
        if running.contains_key(instance_id) {
            return Err(AppError::Launch("cette instance est déjà en cours d'exécution".to_string()));
        }
    }

    let instance = instances::get(&state.paths, instance_id)?;
    let settings = state.settings.read().clone();
    let account = if settings.offline_mode {
        crate::auth::offline_session(&settings.offline_username)?
    } else {
        let current = state.active_account.read().clone().ok_or_else(|| {
            AppError::Auth(
                "Connecte-toi avec un compte Microsoft avant de lancer le jeu, ou active le Mode \
                 Hors-ligne dans Paramètres."
                    .to_string(),
            )
        })?;

        // A session opened long enough before pressing Play can hold a
        // Minecraft access token that's since expired — refresh it now
        // rather than handing the game a token it will reject mid-session
        // with "invalid session, restart the game/launcher".
        if crate::auth::needs_refresh(&current, crate::auth::now_unix()) {
            let refreshed = crate::auth::try_silent_login(&state.paths, &state.client, &settings.azure_client_id)
                .await?
                .ok_or_else(|| {
                    AppError::Auth(
                        "Ta session Microsoft a expiré — reconnecte-toi avant de relancer le jeu.".to_string(),
                    )
                })?;
            *state.active_account.write() = Some(refreshed.clone());
            refreshed
        } else {
            current
        }
    };

    let prepared = minecraft::prepare_version(
        app,
        &state.paths,
        &state.client,
        &state.downloader,
        &instance.minecraft_version,
    )
    .await?;

    let mut classpath = prepared.classpath.clone();
    let mut main_class = prepared.main_class.clone();
    let mut raw_jvm_args = prepared.jvm_args.clone();
    let mut raw_game_args = prepared.game_args.clone();
    let native_jars = prepared.native_jars.clone();
    let java_component = prepared.java_component.clone();
    let mut library_index = prepared.library_index.clone();

    if instance.loader != LoaderKind::Vanilla {
        let loader_version = instance
            .loader_version
            .clone()
            .ok_or_else(|| AppError::Loader("version du mod loader manquante sur cette instance".to_string()))?;
        let installer = state
            .loaders
            .get(instance.loader)
            .ok_or_else(|| AppError::Loader(format!("mod loader non supporté: {:?}", instance.loader)))?;

        let profile = installer
            .resolve(app, &instance.minecraft_version, &loader_version)
            .await
            .map_err(AppError::from)?;

        // A mod loader frequently needs a different version of a library
        // vanilla also ships (e.g. NeoForge's asm-commons vs. Minecraft's
        // own older one) — having both on the classpath/module path at once
        // crashes the JVM at launch, so the loader's version replaces
        // vanilla's rather than sitting alongside it.
        for lib in &profile.extra_libraries {
            if let Some(key) = mc_libraries::group_artifact(&lib.name) {
                if let Some(old_path) = library_index.insert(key, lib.path.clone()) {
                    classpath.retain(|p| p != &old_path);
                }
            }
            classpath.push(lib.path.clone());
        }
        if let Some(mc) = profile.main_class_override {
            main_class = mc;
        }
        raw_jvm_args.extend(profile.extra_jvm_args);
        raw_game_args.extend(profile.extra_game_args);
    }

    let natives_directory = instance.directory.join("natives");
    mc_libraries::extract_natives(&native_jars, &natives_directory)?;

    let java_path = match &settings.java_path_override {
        Some(path) if !path.trim().is_empty() => std::path::PathBuf::from(path),
        _ => state.java.ensure_runtime(app, &state.paths, &java_component).await?.path,
    };

    let placeholders = build_placeholders(
        &account,
        &instance,
        &settings,
        &prepared.asset_index_id,
        &natives_directory,
        &state.paths.assets_dir(),
        &state.paths.libraries_dir(),
    );

    let mut extra_jvm_args = settings.default_jvm_args.clone();
    extra_jvm_args.extend(instance.extra_jvm_args.clone());

    let ctx = LaunchContext {
        java_path,
        game_directory: instance.directory.clone(),
        natives_directory,
        classpath,
        main_class,
        min_memory_mb: instance.min_memory_mb.unwrap_or(settings.default_min_memory_mb),
        max_memory_mb: instance.max_memory_mb.unwrap_or(settings.default_max_memory_mb),
        extra_jvm_args,
        placeholders,
        raw_jvm_args,
        raw_game_args,
    };

    let mut child = super::spawn(instance_id, &ctx)?;
    let log_buffer = super::new_log_buffer();
    super::stream_output(app, instance_id, &mut child, log_buffer.clone());
    instances::touch_last_played(&state.paths, instance_id)?;

    let pid = child.id();
    let shared: RunningChild = Arc::new(AsyncMutex::new(child));

    state.running.lock().insert(instance_id.to_string(), shared.clone());

    let app_for_wait = app.clone();
    let state_running = state.running.clone();
    let instance_id_owned = instance_id.to_string();
    tokio::spawn(async move {
        let status = shared.lock().await.wait().await;
        state_running.lock().remove(&instance_id_owned);
        let code = status.ok().and_then(|s| s.code());
        let crash_analysis = crate::launch::crash_detect::analyze(&log_buffer.lock(), code);
        let _ = app_for_wait.emit(
            "instance-exit",
            InstanceExit {
                instance_id: instance_id_owned,
                code,
                crash_analysis,
            },
        );
    });

    tracing::info!("instance {instance_id} launched (pid={pid:?})");
    Ok(())
}

pub async fn stop_instance(state: &AppState, instance_id: &str) -> AppResult<()> {
    let child = state.running.lock().get(instance_id).cloned();
    match child {
        Some(child) => {
            child
                .lock()
                .await
                .start_kill()
                .map_err(|e| AppError::Launch(format!("impossible d'arrêter le processus: {e}")))?;
            Ok(())
        }
        None => Err(AppError::Launch("cette instance n'est pas en cours d'exécution".to_string())),
    }
}

pub fn is_running(state: &AppState, instance_id: &str) -> bool {
    state.running.lock().contains_key(instance_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::MinecraftProfile;
    use crate::providers::LoaderKind;
    use std::path::PathBuf;

    fn fixture_account() -> AccountSession {
        AccountSession {
            profile: MinecraftProfile { id: "uuid-1".to_string(), name: "Steve".to_string() },
            minecraft_access_token: "token-abc".to_string(),
            expires_at: 0,
        }
    }

    fn fixture_instance() -> Instance {
        Instance {
            id: "instance-1".to_string(),
            name: "Demo".to_string(),
            minecraft_version: "1.20.1".to_string(),
            loader: LoaderKind::Vanilla,
            loader_version: None,
            directory: PathBuf::from("/instances/instance-1"),
            icon_url: None,
            min_memory_mb: None,
            max_memory_mb: None,
            extra_jvm_args: Vec::new(),
            modpack: None,
            created_at: 0,
            last_played_at: None,
        }
    }

    #[test]
    fn build_placeholders_fills_auth_and_version_fields_from_account_and_instance() {
        let placeholders = build_placeholders(
            &fixture_account(),
            &fixture_instance(),
            &GlobalSettings::default(),
            "17",
            Path::new("/instances/instance-1/natives"),
            Path::new("/cache/assets"),
            Path::new("/cache/libraries"),
        );

        assert_eq!(placeholders["auth_player_name"], "Steve");
        assert_eq!(placeholders["auth_uuid"], "uuid-1");
        assert_eq!(placeholders["auth_access_token"], "token-abc");
        assert_eq!(placeholders["auth_xuid"], "uuid-1");
        assert_eq!(placeholders["version_name"], "1.20.1");
        assert_eq!(placeholders["assets_index_name"], "17");
        assert_eq!(placeholders["user_type"], "msa");
    }

    #[test]
    fn build_placeholders_uses_legacy_user_type_in_offline_mode() {
        let settings = GlobalSettings { offline_mode: true, ..GlobalSettings::default() };
        let placeholders = build_placeholders(
            &fixture_account(),
            &fixture_instance(),
            &settings,
            "17",
            Path::new("/natives"),
            Path::new("/assets"),
            Path::new("/libraries"),
        );

        assert_eq!(placeholders["user_type"], "legacy");
    }

    #[test]
    fn build_placeholders_uses_platform_classpath_separator() {
        let placeholders = build_placeholders(
            &fixture_account(),
            &fixture_instance(),
            &GlobalSettings::default(),
            "17",
            Path::new("/natives"),
            Path::new("/assets"),
            Path::new("/libraries"),
        );

        let expected = if cfg!(windows) { ";" } else { ":" };
        assert_eq!(placeholders["classpath_separator"], expected);
    }
}
