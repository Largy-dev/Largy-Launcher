//! Spawns the final `java` process built by `minecraft::launch_args` and
//! streams stdout/stderr to the frontend as `instance-log` events. The
//! cross-module assembly of *what* to launch lives in [`orchestrator`].

pub mod crash_detect;
pub mod orchestrator;

use std::process::Stdio;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::minecraft::launch_args::{build_command_args, LaunchContext};
use crash_detect::CrashAnalysis;

/// A running game as seen from outside its waiter task: the waiter owns the
/// `Child` itself (it has to hold it mutably for the whole `wait()`), so
/// stopping goes through `kill` instead of locking the process.
#[derive(Clone)]
pub struct RunningChild {
    pub pid: Option<u32>,
    pub kill: Arc<Notify>,
}

/// Coarse launch steps, emitted as `launch-phase` events so the UI can show
/// where a launch currently is (auth -> files -> loader -> Java -> game).
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
    let _ = app.emit(
        "launch-phase",
        LaunchPhaseEvent {
            instance_id: instance_id.to_string(),
            phase,
        },
    );
}

/// Memory/CPU usage of a running game's process, for the live stats panel.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessStats {
    pub memory_mb: u64,
    pub cpu_percent: f32,
}

/// How many of the most recent stdout/stderr lines are kept for crash
/// analysis once the process exits — enough to catch a crash trace without
/// holding an unbounded amount of a long play session's log in memory.
const LOG_BUFFER_CAPACITY: usize = 500;

/// Shared, bounded ring of recent log lines for one launch — local to that
/// launch's tasks (not `AppState`), so it's dropped automatically once the
/// instance exits and nothing else needs to reach into a running instance's
/// live log from outside this module.
pub type LogBuffer = Arc<Mutex<Vec<String>>>;

pub fn new_log_buffer() -> LogBuffer {
    Arc::new(Mutex::new(Vec::with_capacity(LOG_BUFFER_CAPACITY)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchState {
    pub instance_id: String,
    pub running: bool,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceLogLine {
    pub instance_id: String,
    pub line: String,
    pub stream: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceExit {
    pub instance_id: String,
    pub code: Option<i32>,
    pub crash_analysis: Option<CrashAnalysis>,
}

pub fn spawn(instance_id: &str, ctx: &LaunchContext) -> AppResult<Child> {
    let args = build_command_args(ctx);
    tracing::info!("launching instance {instance_id}: {} {:?}", ctx.java_path.display(), args);

    let mut cmd = Command::new(&ctx.java_path);
    cmd.args(&args)
        .current_dir(&ctx.game_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::process_ext::hide_console_window(&mut cmd);

    cmd.spawn().map_err(|e| AppError::Launch(format!("échec du lancement de java: {e}")))
}

pub fn stream_output(app: &AppHandle, instance_id: &str, child: &mut Child, log_buffer: LogBuffer) {
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(app.clone(), instance_id.to_string(), stdout, "stdout", log_buffer.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(app.clone(), instance_id.to_string(), stderr, "stderr", log_buffer);
    }
}

fn spawn_log_reader<R>(app: AppHandle, instance_id: String, reader: R, stream: &'static str, log_buffer: LogBuffer)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            {
                let mut buffer = log_buffer.lock();
                if buffer.len() >= LOG_BUFFER_CAPACITY {
                    buffer.remove(0);
                }
                buffer.push(line.clone());
            }
            let _ = app.emit(
                "instance-log",
                InstanceLogLine {
                    instance_id: instance_id.clone(),
                    line,
                    stream,
                },
            );
        }
    });
}
