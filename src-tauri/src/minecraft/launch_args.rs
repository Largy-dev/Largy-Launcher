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

/// The arguments to actually pass to `java`: the full list, or — when the
/// command line would be too long — the JVM options through a `@file` with
/// the main class and game arguments still on the command line. The game
/// arguments carry the access token, which must never be written to disk
/// (the argfile sits in the instance folder, which gets zipped and shared).
pub fn command_args(ctx: &LaunchContext) -> std::io::Result<Vec<String>> {
    let jvm = jvm_args(ctx);
    let game = game_args(ctx);
    let length: usize = jvm.iter().chain(&game).map(|a| a.len() + 3).sum();
    if length <= MAX_COMMAND_LINE || ctx.java_major < 9 {
        return Ok(jvm.into_iter().chain(game).collect());
    }
    let token = ctx.placeholders.get("auth_access_token").filter(|t| t.len() >= 8);
    let (secret, plain): (Vec<String>, Vec<String>) =
        jvm.into_iter().partition(|a| token.is_some_and(|t| a.contains(t.as_str())));
    let path = ctx.argfile_path();
    let content: Vec<String> = plain.iter().map(|a| quote_for_argfile(a)).collect();
    crate::util::fs::write_atomic(&path, content.join("\n").as_bytes())?;
    Ok(std::iter::once(format!("@{}", path.display())).chain(secret).chain(game).collect())
}

/// Produces the full argument list to pass to `Command::new(java_path).args(...)`.
pub fn build_command_args(ctx: &LaunchContext) -> Vec<String> {
    jvm_args(ctx).into_iter().chain(game_args(ctx)).collect()
}

/// Everything before the main class: memory, natives, JVM flags, classpath.
fn jvm_args(ctx: &LaunchContext) -> Vec<String> {
    let mut args = Vec::new();

    args.push(format!("-Xms{}M", ctx.min_memory_mb));
    args.push(format!("-Xmx{}M", ctx.max_memory_mb));
    args.extend(ctx.extra_jvm_args.iter().cloned());
    args.push(format!("-Djava.library.path={}", ctx.natives_directory.display()));
    args.push("-Dminecraft.launcher.brand=LargyLauncher".to_string());

    for raw in &ctx.raw_jvm_args {
        args.push(substitute_dollar_braces(raw, &ctx.placeholders));
    }

    let classpath = ctx.classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(if cfg!(windows) {
        ";"
    } else {
        ":"
    });
    args.push("-cp".to_string());
    args.push(classpath);
    args
}

/// The main class followed by the game's own arguments (username, token...).
fn game_args(ctx: &LaunchContext) -> Vec<String> {
    std::iter::once(ctx.main_class.clone())
        .chain(ctx.raw_game_args.iter().map(|raw| substitute_dollar_braces(raw, &ctx.placeholders)))
        .collect()
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
    fn long_command_lines_keep_the_access_token_out_of_the_argfile() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = fixture_ctx();
        ctx.game_directory = dir.path().to_path_buf();
        ctx.placeholders.insert("auth_access_token".to_string(), "secret-token-123".to_string());
        ctx.raw_game_args.extend(["--accessToken".to_string(), "${auth_access_token}".to_string()]);
        ctx.classpath = (0..2000).map(|i| PathBuf::from(format!("/libs/some-library-{i}.jar"))).collect();

        let args = command_args(&ctx).unwrap();

        assert!(args[0].starts_with('@'));
        assert_eq!(args[1], "net.minecraft.client.main.Main");
        assert_eq!(args.last().unwrap(), "secret-token-123");
        let file = std::fs::read_to_string(ctx.argfile_path()).unwrap();
        assert!(file.contains("some-library-1999.jar"));
        assert!(!file.contains("secret-token-123"));
    }

    #[test]
    fn short_command_lines_are_passed_directly() {
        let ctx = fixture_ctx();
        assert_eq!(command_args(&ctx).unwrap(), build_command_args(&ctx));
    }
}
