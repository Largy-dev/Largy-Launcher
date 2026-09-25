//! Spawns the final `java` process built by `minecraft::launch_args`; the
//! cross-module assembly of *what* to launch lives in [`orchestrator`], and
//! log streaming in [`logs`].

pub mod crash_detect;
mod jvm_args;
pub mod logs;
pub mod orchestrator;
mod prepare;

use std::process::Stdio;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::minecraft::launch_args::{command_args, LaunchContext};
use crash_detect::CrashAnalysis;

/// A launch as seen from outside its own tasks. Registered as soon as a
/// launch *starts preparing* (so a second click can't start a parallel
/// preparation), `pid` is filled in once the game process exists; `kill`
/// cancels the preparation or stops the game.
#[derive(Clone)]
pub struct RunningChild {
    pub pid: Option<u32>,
    pub kill: Arc<Notify>,
}

/// Coarse launch steps, emitted as `launch-phase` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPhase {
    Auth,
    Version,
    Loader,
    Natives,
    Java,
    Starting,
    Running,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchPhaseEvent {
    pub instance_id: String,
    pub phase: LaunchPhase,
}

pub fn emit_phase(app: &AppHandle, instance_id: &str, phase: LaunchPhase) {
    let _ = app.emit("launch-phase", LaunchPhaseEvent { instance_id: instance_id.to_string(), phase });
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessStats {
    pub memory_mb: u64,
    pub cpu_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchState {
    pub instance_id: String,
    pub running: bool,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceExit {
    pub instance_id: String,
    pub code: Option<i32>,
    pub crash_analysis: Option<CrashAnalysis>,
    /// Stopped from the launcher — a non-zero code is expected, not a crash.
    pub killed: bool,
}

/// The command line as it's safe to log: the access token replaced.
fn redacted(args: &[String], secret: &str) -> Vec<String> {
    if secret.len() < 8 {
        return args.to_vec();
    }
    args.iter().map(|a| a.replace(secret, "********")).collect()
}

pub fn spawn(instance_id: &str, ctx: &LaunchContext, secret: &str) -> AppResult<Child> {
    let args = command_args(ctx)?;
    tracing::info!(
        "launching instance {instance_id}: {} {:?}",
        ctx.java_path.display(),
        redacted(&args, secret)
    );

    let mut cmd = Command::new(&ctx.java_path);
    cmd.args(&args)
        .current_dir(&ctx.game_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);
    crate::process_ext::hide_console_window(&mut cmd);

    cmd.spawn()
        .map_err(|e| AppError::Launch(format!("échec du lancement de java ({}): {e}", ctx.java_path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_hides_the_access_token() {
        let args = vec!["--accessToken".to_string(), "eyJhbGciOiJSUzI1NiJ9.secret".to_string()];
        assert_eq!(redacted(&args, "eyJhbGciOiJSUzI1NiJ9.secret")[1], "********");
        assert_eq!(redacted(&args, "-")[1], "eyJhbGciOiJSUzI1NiJ9.secret");
    }
}
