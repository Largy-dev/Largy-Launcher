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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_ctx() -> LaunchContext {
        let mut placeholders = HashMap::new();
        placeholders.insert("auth_player_name".to_string(), "Steve".to_string());
        LaunchContext {
            java_path: PathBuf::from("/usr/bin/java"),
            game_directory: PathBuf::from("/instances/demo"),
            natives_directory: PathBuf::from("/instances/demo/natives"),
            classpath: vec![PathBuf::from("/libs/a.jar"), PathBuf::from("/libs/b.jar")],
            main_class: "net.minecraft.client.main.Main".to_string(),
            min_memory_mb: 1024,
            max_memory_mb: 4096,
            extra_jvm_args: vec!["-Dfoo=bar".to_string()],
            placeholders,
            raw_jvm_args: vec!["-Dextra=1".to_string()],
            raw_game_args: vec!["--username".to_string(), "${auth_player_name}".to_string()],
        }
    }

    #[test]
    fn build_command_args_assembles_memory_natives_classpath_and_main_class_in_order() {
        let ctx = fixture_ctx();
        let args = build_command_args(&ctx);

        assert_eq!(args[0], "-Xms1024M");
        assert_eq!(args[1], "-Xmx4096M");
        assert!(args.contains(&"-Dfoo=bar".to_string()));
        assert!(args.iter().any(|a| a.starts_with("-Djava.library.path=")));
        assert!(args.contains(&"-Dminecraft.launcher.brand=LargyLauncher".to_string()));
        assert!(args.contains(&"-Dextra=1".to_string()));

        let cp_index = args.iter().position(|a| a == "-cp").unwrap();
        let separator = if cfg!(windows) { ";" } else { ":" };
        assert_eq!(args[cp_index + 1], format!("/libs/a.jar{separator}/libs/b.jar"));

        let main_class_index = args.iter().position(|a| a == "net.minecraft.client.main.Main").unwrap();
        assert!(main_class_index > cp_index);

        assert_eq!(args[args.len() - 2], "--username");
        assert_eq!(args[args.len() - 1], "Steve");
    }

    #[test]
    fn apply_loader_profile_merges_main_class_and_extra_args() {
        let mut ctx = fixture_ctx();
        let profile = LoaderProfile {
            extra_libraries: Vec::new(),
            main_class_override: Some("net.minecraftforge.Main".to_string()),
            extra_jvm_args: vec!["-Dloader=1".to_string()],
            extra_game_args: vec!["--forge".to_string()],
            install_side_effects: Vec::new(),
        };

        apply_loader_profile(&mut ctx, &profile);

        assert_eq!(ctx.main_class, "net.minecraftforge.Main");
        assert!(ctx.extra_jvm_args.contains(&"-Dloader=1".to_string()));
        assert!(ctx.raw_game_args.contains(&"--forge".to_string()));
    }

    #[test]
    fn apply_loader_profile_keeps_vanilla_main_class_when_no_override() {
        let mut ctx = fixture_ctx();
        let profile = LoaderProfile {
            extra_libraries: Vec::new(),
            main_class_override: None,
            extra_jvm_args: Vec::new(),
            extra_game_args: Vec::new(),
            install_side_effects: Vec::new(),
        };

        apply_loader_profile(&mut ctx, &profile);
        assert_eq!(ctx.main_class, "net.minecraft.client.main.Main");
    }
}
