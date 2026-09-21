//! Builds the final `java` invocation from a resolved version manifest plus
//! any mod-loader contributions (`LoaderProfile`), substituting the standard
//! `${auth_player_name}`-style placeholders Mojang's version JSON uses.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::modloaders::LoaderProfile;

#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub java_path: PathBuf,
    pub game_directory: PathBuf,
    pub natives_directory: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub main_class: String,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub extra_jvm_args: Vec<String>,
    pub placeholders: HashMap<String, String>,
    pub raw_jvm_args: Vec<String>,
    pub raw_game_args: Vec<String>,
}

/// Merges a mod loader's contributions (extra libraries already folded into
/// `classpath` by the caller) into a `LaunchContext` built from the vanilla
/// version manifest.
pub fn apply_loader_profile(ctx: &mut LaunchContext, profile: &LoaderProfile) {
    if let Some(main_class) = &profile.main_class_override {
        ctx.main_class = main_class.clone();
    }
    ctx.extra_jvm_args.extend(profile.extra_jvm_args.iter().cloned());
    ctx.raw_game_args.extend(profile.extra_game_args.iter().cloned());
}

fn substitute(arg: &str, placeholders: &HashMap<String, String>) -> String {
    let mut result = arg.to_string();
    for (key, value) in placeholders {
        result = result.replace(&format!("${{{key}}}"), value);
    }
    result
}

/// Produces the full argument list to pass to `Command::new(java_path).args(...)`.
pub fn build_command_args(ctx: &LaunchContext) -> Vec<String> {
    let mut args = Vec::new();

    args.push(format!("-Xms{}M", ctx.min_memory_mb));
    args.push(format!("-Xmx{}M", ctx.max_memory_mb));
    args.extend(ctx.extra_jvm_args.iter().cloned());
    args.push(format!(
        "-Djava.library.path={}",
        ctx.natives_directory.display()
    ));
    args.push("-Dminecraft.launcher.brand=LargyLauncher".to_string());

    for raw in &ctx.raw_jvm_args {
        args.push(substitute(raw, &ctx.placeholders));
    }

    let classpath = ctx
        .classpath
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(if cfg!(windows) { ";" } else { ":" });
    args.push("-cp".to_string());
    args.push(classpath);

    args.push(ctx.main_class.clone());

    for raw in &ctx.raw_game_args {
        args.push(substitute(raw, &ctx.placeholders));
    }

    args
}
