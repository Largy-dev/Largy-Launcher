use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::launch::{orchestrator, ProcessStats};
use crate::state::AppState;

#[tauri::command]
pub async fn launch_instance(app: AppHandle, state: State<'_, AppState>, instance_id: String) -> AppResult<()> {
    orchestrator::launch_instance(&app, &state, &instance_id).await
}

#[tauri::command]
pub async fn stop_instance(state: State<'_, AppState>, instance_id: String) -> AppResult<()> {
    orchestrator::stop_instance(&state, &instance_id).await
}

#[tauri::command]
pub fn is_instance_running(state: State<'_, AppState>, instance_id: String) -> bool {
    orchestrator::is_running(&state, &instance_id)
}

#[tauri::command]
pub fn instance_process_stats(state: State<'_, AppState>, instance_id: String) -> Option<ProcessStats> {
    orchestrator::process_stats(&state, &instance_id)
}
