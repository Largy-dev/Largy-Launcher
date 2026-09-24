use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::launch::{orchestrator, ProcessStats};
use crate::state::AppState;

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    server: Option<String>,
) -> AppResult<()> {
    orchestrator::launch_instance(&app, &state, &instance_id, server).await
}

/// Instance a desktop shortcut asked to launch before the UI was listening
/// (`--launch <id>` on the command line), handed over once.
#[tauri::command]
pub fn take_pending_launch(state: State<'_, AppState>) -> Option<String> {
    state.pending_launch.lock().take()
}

/// Stops a running game, or cancels a launch that's still preparing.
#[tauri::command]
pub async fn stop_instance(state: State<'_, AppState>, instance_id: String) -> AppResult<()> {
    orchestrator::stop_instance(&state, &instance_id).await
}

#[tauri::command]
pub async fn repair_instance(app: AppHandle, state: State<'_, AppState>, instance_id: String) -> AppResult<()> {
    orchestrator::repair_instance(&app, &state, &instance_id).await
}

#[tauri::command]
pub fn is_instance_running(state: State<'_, AppState>, instance_id: String) -> bool {
    orchestrator::is_running(&state, &instance_id)
}

#[tauri::command]
pub fn instance_process_stats(state: State<'_, AppState>, instance_id: String) -> Option<ProcessStats> {
    orchestrator::process_stats(&state, &instance_id)
}
