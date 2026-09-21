//! Spawns the final `java` process built by `minecraft::launch_args` and
//! streams stdout/stderr to the frontend as `instance-log` events. The
//! cross-module assembly of *what* to launch lives in [`orchestrator`].

pub mod orchestrator;

use std::process::Stdio;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{AppError, AppResult};
use crate::minecraft::launch_args::{build_command_args, LaunchContext};

pub type RunningChild = Arc<AsyncMutex<Child>>;

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
}

pub fn spawn(instance_id: &str, ctx: &LaunchContext) -> AppResult<Child> {
    let args = build_command_args(ctx);
    tracing::info!("launching instance {instance_id}: {} {:?}", ctx.java_path.display(), args);

    Command::new(&ctx.java_path)
        .args(&args)
        .current_dir(&ctx.game_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Launch(format!("échec du lancement de java: {e}")))
}

pub fn stream_output(app: &AppHandle, instance_id: &str, child: &mut Child) {
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(app.clone(), instance_id.to_string(), stdout, "stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(app.clone(), instance_id.to_string(), stderr, "stderr");
    }
}

fn spawn_log_reader<R>(app: AppHandle, instance_id: String, reader: R, stream: &'static str)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
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
