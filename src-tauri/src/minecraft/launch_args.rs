//! Builds the final `java` invocation from a resolved version manifest plus
//! any mod-loader contributions, substituting the standard
//! `${auth_player_name}`-style placeholders Mojang's version JSON uses.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::util::placeholders::substitute_dollar_braces;

#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub java_path: PathBuf,
    /// Major version of `java_path` (argfiles need Java 9+).
    pub java_major: u32,
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

/// Windows caps a whole command line at 32 767 chars; big modpacks' classpaths
/// get close, so past this the arguments go through a Java `@argfile`.
const MAX_COMMAND_LINE: usize = 30_000;

impl LaunchContext {
    pub fn argfile_path(&self) -> PathBuf {
        self.game_directory.join(".largy-launch-args.txt")
    }
}

fn quote_for_argfile(arg: &str) -> String {
    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The arguments to actually pass to `java`: the full list, or a single
/// `@file` pointing at them when the command line would be too long.
pub fn command_args(ctx: &LaunchContext) -> std::io::Result<Vec<String>> {
    let args = build_command_args(ctx);
    let length: usize = args.iter().map(|a| a.len() + 3).sum();
    if length <= MAX_COMMAND_LINE || ctx.java_major < 9 {
        return Ok(args);
    }
    let path = ctx.argfile_path();
    let content: Vec<String> = args.iter().map(|a| quote_for_argfile(a)).collect();
    crate::util::fs::write_atomic(&path, content.join("\n").as_bytes())?;
    Ok(vec![format!("@{}", path.display())])
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
        args.push(substitute_dollar_braces(raw, &ctx.placeholders));
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
        args.push(substitute_dollar_braces(raw, &ctx.placeholders));
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
            java_major: 17,
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
    fn argfile_quoting_escapes_backslashes_and_quotes() {
        assert_eq!(quote_for_argfile(r#"C:\dir"b"#), r#""C:\\dir\"b""#);
    }

    #[test]
    fn short_command_lines_are_passed_directly() {
        let ctx = fixture_ctx();
        assert_eq!(command_args(&ctx).unwrap(), build_command_args(&ctx));
    }
}
