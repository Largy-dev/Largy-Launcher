//! Running one resolved install-profile processor step, and building the
//! full plan of processor invocations (classpath + args, every placeholder
//! and embedded-jar token already resolved) from the raw install profile.

use std::collections::HashMap;
use std::path::Path;

use crate::minecraft::libraries::maven_path;
use crate::paths::AppPaths;
use crate::util::placeholders::substitute_braces;

use super::super::LoaderError;
use super::zip_resolve::{read_main_class, resolve_token};
use super::{DataEntry, ProcessorEntry};

pub(super) type ProcessorPlan = Vec<(String, Vec<String>, Vec<String>)>;

/// Runs one processor with already fully-resolved `args` (placeholders
/// substituted and any embedded `[coord]`/`'literal'` tokens resolved).
pub(super) async fn run_processor(
    jar: &str,
    classpath: &[String],
    args: &[String],
    libraries_dir: &Path,
    java_path: &Path,
) -> Result<(), LoaderError> {
    let jar_rel = maven_path(jar).ok_or_else(|| LoaderError::Other(format!("coordonnée maven invalide: {jar}")))?;
    let jar_path = libraries_dir.join(jar_rel);
    let main_class = read_main_class(&jar_path)?;

    let mut full_classpath = vec![jar_path.display().to_string()];
    full_classpath.extend(classpath.iter().cloned());
    let separator = if cfg!(windows) { ";" } else { ":" };

    let mut cmd = tokio::process::Command::new(java_path);
    cmd.arg("-cp").arg(full_classpath.join(separator)).arg(&main_class).args(args);
    crate::process_ext::hide_console_window(&mut cmd);
    let output = cmd.output().await?;

    if !output.status.success() {
        return Err(LoaderError::Other(format!(
            "processeur {main_class} a échoué:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

/// Resolves every `data` entry and processor argument against the still-open
/// installer archive, extracting embedded resources as needed. Pure CPU/disk
/// work (no network, no async) — run inside `spawn_blocking` so it doesn't
/// stall the async runtime for as long as the archive takes to walk.
pub(super) fn resolve_processor_plan(
    mut archive: zip::ZipArchive<std::fs::File>,
    data: &HashMap<String, DataEntry>,
    processors: &[ProcessorEntry],
    paths: &AppPaths,
    mc_version: &str,
    installer_path: &Path,
    cache_key: &str,
) -> Result<(HashMap<String, String>, ProcessorPlan), LoaderError> {
    let scratch_dir = paths.installers_dir().join("extracted").join(cache_key);
    let libraries_dir = paths.libraries_dir();

    let mut placeholders = HashMap::new();
    let minecraft_jar = paths.versions_dir().join(mc_version).join(format!("{mc_version}.jar"));
    placeholders.insert("SIDE".to_string(), "client".to_string());
    placeholders.insert("MINECRAFT_JAR".to_string(), minecraft_jar.display().to_string());
    placeholders.insert("MINECRAFT_VERSION".to_string(), mc_version.to_string());
    placeholders.insert("ROOT".to_string(), paths.installers_dir().display().to_string());
    placeholders.insert("INSTALLER".to_string(), installer_path.display().to_string());
    placeholders.insert("LIBRARY_DIR".to_string(), libraries_dir.display().to_string());

    for (key, entry) in data {
        let resolved = resolve_token(&mut archive, &entry.client, &libraries_dir, &scratch_dir)?;
        placeholders.insert(key.clone(), resolved);
    }

    // Resolve every processor's classpath/args now, while the jar is still
    // open — a `[coord]` can appear directly in `args` (not just via a
    // `{TOKEN}`), which needs the same jar-extraction as `data`.
    let mut runnable_processors = Vec::new();
    for processor in processors {
        if !processor.sides.is_empty() && !processor.sides.iter().any(|s| s == "client") {
            continue;
        }
        let classpath = processor
            .classpath
            .iter()
            .filter_map(|coord| maven_path(coord).map(|rel| libraries_dir.join(rel).display().to_string()))
            .collect::<Vec<_>>();
        let mut args = Vec::with_capacity(processor.args.len());
        for raw_arg in &processor.args {
            let substituted = substitute_braces(raw_arg, &placeholders);
            args.push(resolve_token(&mut archive, &substituted, &libraries_dir, &scratch_dir)?);
        }
        runnable_processors.push((processor.jar.clone(), classpath, args));
    }

    Ok((placeholders, runnable_processors))
}
