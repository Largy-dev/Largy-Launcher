//! Cross-cutting glue: pulls together the active account, an instance's
//! resolved vanilla version, its mod loader (if any), and a matching Java
//! runtime, then hands the assembled command line to [`super::spawn`].

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::discord::GameActivity;
use crate::download::Verify;
use crate::error::{AppError, AppResult};
use crate::instances;
use crate::settings::LauncherBehavior;
use crate::state::{cancellable, AppState};

use super::prepare::{prepare, resolve_account};
use super::{emit_phase, logs, InstanceExit, LaunchPhase, ProcessStats, RunningChild};

/// Removes the instance's `running` entry on drop unless disarmed — any
/// early return during preparation (error, cancel) frees the instance.
struct Registration<'a> {
    state: &'a AppState,
    id: String,
    armed: bool,
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.state.running.lock().remove(&self.id);
        }
    }
}

fn register<'a>(state: &'a AppState, instance_id: &str) -> AppResult<(Registration<'a>, Arc<Notify>)> {
    instances::validate_id(instance_id)?;
    let mut running = state.running.lock();
    if running.contains_key(instance_id) {
        return Err(AppError::Launch("cette instance est déjà en cours d'exécution ou de préparation".to_string()));
    }
    let kill = Arc::new(Notify::new());
    running.insert(instance_id.to_string(), RunningChild { pid: None, kill: kill.clone() });
    Ok((Registration { state, id: instance_id.to_string(), armed: true }, kill))
}

fn apply_window_behavior(app: &AppHandle, behavior: LauncherBehavior, game_started: bool, others_running: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    match (behavior, game_started) {
        (LauncherBehavior::Minimize, true) => {
            let _ = window.minimize();
        }
        (LauncherBehavior::Hide, true) => {
            let _ = window.hide();
        }
        (LauncherBehavior::Hide, false) if !others_running => {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        _ => {}
    }
}

/// Launches an instance; `server` (`host[:port]`) joins that server for this
/// launch only, over the instance's own auto-join setting.
pub async fn launch_instance(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    server: Option<String>,
) -> AppResult<()> {
    let server = server.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(server) = &server {
        crate::servers::validate_address(server)?;
    }
    let (mut registration, kill) = register(state, instance_id)?;

    let mut instance = instances::get(&state.paths, instance_id)?;
    if server.is_some() {
        instance.auto_join_server = server;
    }
    let settings = state.settings.read().clone();

    emit_phase(app, instance_id, LaunchPhase::Auth);
    let account = cancellable(&kill, resolve_account(state, &settings)).await?;
    let prepared =
        cancellable(&kill, prepare(app, state, &instance, &settings, &account, Verify::Fast, true)).await?;

    emit_phase(app, instance_id, LaunchPhase::Starting);
    let mut child = super::spawn(instance_id, &prepared.ctx, &account.minecraft_access_token)?;
    let pid = child.id();
    if let Some(entry) = state.running.lock().get_mut(instance_id) {
        entry.pid = pid;
    }
    registration.armed = false;

    let log_buffer = logs::new_log_buffer();
    let log_task = logs::stream_output(
        app,
        instance_id,
        &mut child,
        log_buffer.clone(),
        prepared.xml_logs,
        vec![account.minecraft_access_token.clone()],
    );
    if let Err(e) = instances::touch_last_played(&state.paths, instance_id) {
        tracing::warn!("failed to record last-played for {instance_id}: {e}");
    }
    emit_phase(app, instance_id, LaunchPhase::Running);
    apply_window_behavior(app, settings.on_game_launch, true, false);
    state.discord.game_started(instance_id, GameActivity::for_instance(&instance, crate::auth::now_unix()));

    let app_for_wait = app.clone();
    let state_running = state.running.clone();
    let paths = state.paths.clone();
    let instance_id_owned = instance_id.to_string();
    let behavior = settings.on_game_launch;
    let argfile = prepared.ctx.argfile_path();
    let started = std::time::Instant::now();
    let discord = state.discord.clone();
    tokio::spawn(async move {
        let (status, killed) = tokio::select! {
            status = child.wait() => (status, false),
            _ = kill.notified() => {
                if let Err(e) = child.start_kill() {
                    tracing::warn!("failed to kill instance {instance_id_owned}: {e}");
                }
                (child.wait().await, true)
            }
        };
        // Let the readers drain what the game printed last; a grandchild
        // still holding the pipe open must not block the exit report.
        let _ = tokio::time::timeout(Duration::from_secs(3), log_task).await;
        let _ = std::fs::remove_file(argfile);

        let others_running = {
            let mut running = state_running.lock();
            running.remove(&instance_id_owned);
            !running.is_empty()
        };
        discord.game_stopped(&instance_id_owned);
        if let Err(e) = instances::add_play_time(&paths, &instance_id_owned, started.elapsed().as_secs()) {
            tracing::warn!("failed to record play time for {instance_id_owned}: {e}");
        }
        let code = status.ok().and_then(|s| s.code());
        let crash_analysis = {
            let mut buffer = log_buffer.lock();
            crate::launch::crash_detect::analyze(buffer.make_contiguous(), code, killed)
        };
        apply_window_behavior(&app_for_wait, behavior, false, others_running);
        let _ = app_for_wait.emit(
            "instance-exit",
            InstanceExit { instance_id: instance_id_owned, code, crash_analysis, killed },
        );
    });

    tracing::info!("instance {instance_id} launched (pid={pid:?})");
    Ok(())
}

/// Re-verifies every file an instance needs (full checksums), re-runs the
/// mod loader install and re-extracts natives — without starting the game.
pub async fn repair_instance(app: &AppHandle, state: &AppState, instance_id: &str) -> AppResult<()> {
    let (_registration, kill) = register(state, instance_id)?;
    let instance = instances::get(&state.paths, instance_id)?;
    let settings = state.settings.read().clone();
    let placeholder_account = crate::auth::offline_session("Player")?;
    cancellable(&kill, prepare(app, state, &instance, &settings, &placeholder_account, Verify::Full, false)).await?;
    Ok(())
}

pub async fn stop_instance(state: &AppState, instance_id: &str) -> AppResult<()> {
    let child = state.running.lock().get(instance_id).cloned();
    match child {
        Some(child) => {
            child.kill.notify_one();
            Ok(())
        }
        None => Err(AppError::Launch("cette instance n'est pas en cours d'exécution".to_string())),
    }
}

pub fn process_stats(state: &AppState, instance_id: &str) -> Option<ProcessStats> {
    let pid = state.running.lock().get(instance_id)?.pid?;
    let pid = sysinfo::Pid::from_u32(pid);
    let mut sys = state.system.lock();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    let process = sys.process(pid)?;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
    Some(ProcessStats { memory_mb: process.memory() / 1024 / 1024, cpu_percent: process.cpu_usage() / cores })
}

pub fn is_running(state: &AppState, instance_id: &str) -> bool {
    state.running.lock().contains_key(instance_id)
}
