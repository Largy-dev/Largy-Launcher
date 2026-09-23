//! Everything a launch needs before the game process starts: vanilla
//! files, mod loader, natives, Java, and the final `LaunchContext`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::auth::AccountSession;
use crate::download::Verify;
use crate::error::{AppError, AppResult};
use crate::instances::Instance;
use crate::minecraft::manifest::{self, Features};
use crate::minecraft::{self, launch_args::LaunchContext, libraries as mc_libraries, ClasspathEntry};
use crate::modloaders::LoaderContext;
use crate::providers::LoaderKind;
use crate::settings::{sanitize_memory, GlobalSettings};
use crate::state::AppState;

use super::{emit_phase, LaunchPhase};

#[allow(clippy::too_many_arguments)]
fn build_placeholders(
    account: &AccountSession,
    instance: &Instance,
    settings: &GlobalSettings,
    version_type: &str,
    asset_index_id: &str,
    natives_directory: &Path,
    assets_dir: &Path,
    libraries_dir: &Path,
) -> HashMap<String, String> {
    let mut p = HashMap::new();
    let mut put = |k: &str, v: String| {
        p.insert(k.to_string(), v);
    };
    put("auth_player_name", account.profile.name.clone());
    put("auth_uuid", account.profile.id.clone());
    put("auth_access_token", account.minecraft_access_token.clone());
    put("auth_session", format!("token:{}:{}", account.minecraft_access_token, account.profile.id));
    put("auth_xuid", account.xuid.clone().unwrap_or_else(|| "0".to_string()));
    put("user_type", (if settings.offline_mode { "legacy" } else { "msa" }).to_string());
    put("version_name", instance.minecraft_version.clone());
    put("version_type", version_type.to_string());
    put("game_directory", instance.directory.display().to_string());
    put("assets_root", assets_dir.display().to_string());
    put("game_assets", assets_dir.join("virtual").join(asset_index_id).display().to_string());
    put("assets_index_name", asset_index_id.to_string());
    put("user_properties", "{}".to_string());
    put("clientid", "-".to_string());
    put("launcher_name", "LargyLauncher".to_string());
    put("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    put("natives_directory", natives_directory.display().to_string());
    put("library_directory", libraries_dir.display().to_string());
    put("classpath_separator", if cfg!(windows) { ";" } else { ":" }.to_string());
    if let (Some(w), Some(h)) = (instance.window_width, instance.window_height) {
        put("resolution_width", w.to_string());
        put("resolution_height", h.to_string());
    }
    if let Some(server) = &instance.auto_join_server {
        put("quickPlayMultiplayer", server.clone());
    }
    p
}

/// `host[:port]` -> legacy `--server`/`--port` game args (pre-1.20 clients).
fn legacy_server_args(server: &str) -> Vec<String> {
    let (host, port) = match server.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, p),
        _ => (server, "25565"),
    };
    vec!["--server".to_string(), host.to_string(), "--port".to_string(), port.to_string()]
}

/// Uses the stored account, refreshing its Minecraft token when close to
/// expiry. Without network the cached session is used as-is — enough for
/// singleplayer, which is what "play offline" means for a Microsoft account.
pub(super) async fn resolve_account(state: &AppState, settings: &GlobalSettings) -> AppResult<AccountSession> {
    if settings.offline_mode {
        return crate::auth::offline_session(&settings.offline_username);
    }
    let current = state.active_account.read().clone().ok_or_else(|| {
        AppError::Auth(
            "Connecte-toi avec un compte Microsoft avant de lancer le jeu, ou active le Mode Hors-ligne dans \
             Paramètres."
                .to_string(),
        )
    })?;
    if !crate::auth::needs_refresh(&current, crate::auth::now_unix()) {
        return Ok(current);
    }
    let refreshed =
        crate::auth::refresh_or_keep(&state.paths, &state.client, &settings.azure_client_id, current).await?;
    *state.active_account.write() = Some(refreshed.clone());
    Ok(refreshed)
}

/// Picks the Java to run: instance override, then global override, then the
/// Mojang runtime the version asks for. Overrides are probed so a missing or
/// too-old Java fails here with a clear message instead of a JVM crash.
async fn resolve_java(
    app: &AppHandle,
    state: &AppState,
    downloader: &crate::download::DownloadManager,
    instance: &Instance,
    settings: &GlobalSettings,
    component: &str,
    required_major: u32,
) -> AppResult<(PathBuf, u32)> {
    let override_path = [instance.java_path.as_deref(), settings.java_path_override.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|p| !p.is_empty());
    if let Some(path) = override_path {
        let path = PathBuf::from(path);
        let (version, major) = crate::java::probe(&path)
            .await
            .ok_or_else(|| AppError::Java(format!("Java introuvable ou invalide : {}", path.display())))?;
        if major < required_major {
            return Err(AppError::Java(format!(
                "Cette version de Minecraft demande Java {required_major}, mais le Java choisi est la version \
                 {version}. Choisis un autre Java ou laisse le launcher le gérer automatiquement."
            )));
        }
        return Ok((path, major));
    }
    let runtime = state.java.ensure_runtime(app, &state.paths, downloader, component).await?;
    Ok((runtime.path, required_major))
}

pub(super) struct Prepared {
    pub ctx: LaunchContext,
    pub xml_logs: bool,
}

pub(super) async fn prepare(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
    settings: &GlobalSettings,
    account: &AccountSession,
    verify: Verify,
    phases: bool,
) -> AppResult<Prepared> {
    let phase = |p| {
        if phases {
            emit_phase(app, &instance.id, p);
        }
    };
    let downloader = state.downloader.with_verify(verify);

    phase(LaunchPhase::Version);
    let prepared =
        minecraft::prepare_version(app, &state.paths, &state.meta, &downloader, &instance.minecraft_version).await?;

    let supports_quick_play = prepared
        .arguments
        .as_ref()
        .is_some_and(|a| manifest::flatten_args_with(&a.game, &Features { quick_play_multiplayer: true, ..Default::default() })
            .iter()
            .any(|arg| arg.contains("quickPlayMultiplayer")));
    let features = Features {
        custom_resolution: instance.window_width.is_some() && instance.window_height.is_some(),
        quick_play_multiplayer: instance.auto_join_server.is_some() && supports_quick_play,
    };

    let mut classpath: Vec<ClasspathEntry> = prepared.classpath;
    let mut library_index = prepared.library_index;
    let mut main_class = prepared.main_class;
    let (mut raw_jvm_args, mut raw_game_args) = match &prepared.arguments {
        Some(args) => (
            minecraft::strip_builtin_jvm_args(manifest::flatten_args_with(&args.jvm, &features)),
            manifest::flatten_args_with(&args.game, &features),
        ),
        None => (Vec::new(), prepared.legacy_game_args),
    };
    let modern_args = prepared.arguments.is_some();

    if instance.loader != LoaderKind::Vanilla {
        phase(LaunchPhase::Loader);
        let loader_version = instance
            .loader_version
            .clone()
            .ok_or_else(|| AppError::Loader("version du mod loader manquante sur cette instance".to_string()))?;
        let installer = state
            .loaders
            .get(instance.loader)
            .ok_or_else(|| AppError::Loader(format!("mod loader non supporté: {:?}", instance.loader)))?;
        let ctx = LoaderContext {
            app,
            paths: &state.paths,
            meta: &state.meta,
            downloader: &downloader,
            java: &state.java,
            java_component: &prepared.java_component,
            force_reinstall: verify == Verify::Full,
        };
        let profile = installer.resolve(&ctx, &instance.minecraft_version, &loader_version).await?;

        // A loader frequently ships a newer version of a library vanilla
        // also has (e.g. asm-commons) — both on the classpath/module path
        // crashes the JVM, so the loader's copy replaces vanilla's.
        for lib in profile.extra_libraries {
            if let Some(key) = mc_libraries::group_artifact(&lib.name) {
                if let Some(old_path) = library_index.insert(key, lib.path.clone()) {
                    classpath.retain(|c| c.path != old_path);
                }
            }
            classpath.retain(|c| c.path != lib.path);
            classpath.push(ClasspathEntry { name: lib.name, path: lib.path });
        }
        if let Some(mc) = profile.main_class_override {
            main_class = mc;
        }
        raw_jvm_args.extend(profile.extra_jvm_args);
        match profile.game_args_override {
            Some(args) => raw_game_args = args,
            None => raw_game_args.extend(profile.extra_game_args),
        }
    }

    if !modern_args {
        if let (Some(w), Some(h)) = (instance.window_width, instance.window_height) {
            raw_game_args.extend(["--width".to_string(), w.to_string(), "--height".to_string(), h.to_string()]);
        }
    }
    if let Some(server) = &instance.auto_join_server {
        if !features.quick_play_multiplayer {
            raw_game_args.extend(legacy_server_args(server));
        }
    }
    if instance.fullscreen {
        raw_game_args.push("--fullscreen".to_string());
    }

    phase(LaunchPhase::Natives);
    let natives_directory = instance.directory.join("natives");
    if verify == Verify::Full {
        let _ = std::fs::remove_dir_all(&natives_directory);
    }
    {
        let (jars, dir, key) = (prepared.native_jars.clone(), natives_directory.clone(), prepared.id.clone());
        tokio::task::spawn_blocking(move || mc_libraries::extract_natives(&jars, &dir, &key))
            .await
            .map_err(|e| AppError::Other(format!("tâche de fond interrompue: {e}")))??;
    }

    phase(LaunchPhase::Java);
    let (java_path, java_major) = resolve_java(
        app,
        state,
        &downloader,
        instance,
        settings,
        &prepared.java_component,
        prepared.java_major,
    )
    .await?;

    let placeholders = build_placeholders(
        account,
        instance,
        settings,
        &prepared.version_type,
        &prepared.asset_index_id,
        &natives_directory,
        &state.paths.assets_dir(),
        &state.paths.libraries_dir(),
    );

    let mut extra_jvm_args = prepared.logging_jvm_args;
    if !modern_args && cfg!(windows) {
        // What 1.13+ version JSONs add themselves: works around Intel
        // drivers that special-case `minecraft.exe` in their heap dumps.
        extra_jvm_args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string());
    }
    extra_jvm_args.extend(settings.default_jvm_args.iter().cloned());
    extra_jvm_args.extend(instance.extra_jvm_args.iter().cloned());

    let (min_memory_mb, max_memory_mb) = sanitize_memory(
        instance.min_memory_mb.unwrap_or(settings.default_min_memory_mb),
        instance.max_memory_mb.unwrap_or(settings.default_max_memory_mb),
    );

    Ok(Prepared {
        ctx: LaunchContext {
            java_path,
            java_major,
            game_directory: instance.directory.clone(),
            natives_directory,
            classpath: classpath.into_iter().map(|c| c.path).collect(),
            main_class,
            min_memory_mb,
            max_memory_mb,
            extra_jvm_args,
            placeholders,
            raw_jvm_args,
            raw_game_args,
        },
        xml_logs: prepared.xml_logs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::MinecraftProfile;

    fn fixture_account() -> AccountSession {
        AccountSession {
            profile: MinecraftProfile { id: "uuid-1".to_string(), name: "Steve".to_string() },
            minecraft_access_token: "token-abc".to_string(),
            expires_at: 0,
            xuid: Some("2535".to_string()),
        }
    }

    fn fixture_instance() -> Instance {
        serde_json::from_value(serde_json::json!({
            "id": "instance-1",
            "name": "Demo",
            "minecraft_version": "1.20.1",
            "loader": "vanilla",
            "loader_version": null,
            "directory": "/instances/instance-1",
        }))
        .unwrap()
    }

    fn placeholders_for(instance: &Instance, settings: &GlobalSettings) -> HashMap<String, String> {
        build_placeholders(
            &fixture_account(),
            instance,
            settings,
            "release",
            "17",
            Path::new("/natives"),
            Path::new("/assets"),
            Path::new("/libraries"),
        )
    }

    #[test]
    fn build_placeholders_fills_auth_and_version_fields_from_account_and_instance() {
        let p = placeholders_for(&fixture_instance(), &GlobalSettings::default());
        assert_eq!(p["auth_player_name"], "Steve");
        assert_eq!(p["auth_uuid"], "uuid-1");
        assert_eq!(p["auth_access_token"], "token-abc");
        assert_eq!(p["auth_xuid"], "2535");
        assert_eq!(p["version_name"], "1.20.1");
        assert_eq!(p["assets_index_name"], "17");
        assert_eq!(p["user_type"], "msa");
        assert!(!p.contains_key("resolution_width"));
    }

    #[test]
    fn build_placeholders_uses_legacy_user_type_in_offline_mode() {
        let settings = GlobalSettings { offline_mode: true, ..GlobalSettings::default() };
        assert_eq!(placeholders_for(&fixture_instance(), &settings)["user_type"], "legacy");
    }

    #[test]
    fn build_placeholders_exposes_resolution_and_server() {
        let mut instance = fixture_instance();
        instance.window_width = Some(1280);
        instance.window_height = Some(720);
        instance.auto_join_server = Some("play.example.net:25570".to_string());
        let p = placeholders_for(&instance, &GlobalSettings::default());
        assert_eq!(p["resolution_width"], "1280");
        assert_eq!(p["quickPlayMultiplayer"], "play.example.net:25570");
    }

    #[test]
    fn legacy_server_args_split_host_and_port() {
        assert_eq!(legacy_server_args("mc.example.net:25570"), vec!["--server", "mc.example.net", "--port", "25570"]);
        assert_eq!(legacy_server_args("mc.example.net"), vec!["--server", "mc.example.net", "--port", "25565"]);
    }
}
