//! Reading and extracting entries out of an open Forge/NeoForge installer
//! jar: `install_profile.json`/`version.json` text, embedded `maven/`
//! resources, and a processor jar's `Main-Class` manifest entry.

use std::io::Read;
use std::path::Path;

use crate::minecraft::libraries::maven_path;
use crate::util::fs::safe_join;

use super::super::LoaderError;

pub(super) fn read_zip_text(archive: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Result<String, LoaderError> {
    let mut entry = archive.by_name(path)?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Extracts `zip_path` from the installer jar to `dest`. Some `data` entries
/// name a coordinate that is the *output* of a later processor step rather
/// than something embedded in the jar — in that case there's nothing to
/// extract yet, which is fine: we only need `dest`'s path to exist as a
/// classpath location, not its contents.
pub(super) fn extract_zip_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    zip_path: &str,
    dest: &Path,
) -> Result<(), LoaderError> {
    if dest.exists() {
        return Ok(());
    }
    let mut entry = match archive.by_name(zip_path) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = std::fs::File::create(dest)?;
    std::io::copy(&mut entry, &mut out)?;
    Ok(())
}

/// Resolves one `data`-map value, or one processor argument, against the
/// installer jar. Forge's install profile mixes three conventions for the
/// same string:
/// - `[group:artifact:version]` — a Maven coordinate; extract from this
///   jar's `maven/` folder into the shared library cache (or just compute
///   the path if it's a processor *output* that doesn't exist yet) and
///   substitute the resulting file path.
/// - `'literal value'` — a literal string (a hash, a version string, ...)
///   to use as-is, quotes stripped; never a path.
/// - `/relative/path/in/jar` (no quotes) — a real embedded resource;
///   extract into a scratch folder and substitute the resulting file path.
///
/// Anything else is returned unchanged (e.g. plain task-name arguments).
pub(super) fn resolve_token(
    archive: &mut zip::ZipArchive<std::fs::File>,
    raw_value: &str,
    libraries_dir: &Path,
    scratch_dir: &Path,
) -> Result<String, LoaderError> {
    if let Some(coord) = raw_value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let rel_path = maven_path(coord)
            .ok_or_else(|| LoaderError::Other(format!("coordonnée maven invalide: {coord}")))?;
        let dest = safe_join(libraries_dir, &rel_path)
            .ok_or_else(|| LoaderError::Other(format!("chemin invalide: {rel_path}")))?;
        extract_zip_entry(archive, &format!("maven/{rel_path}"), &dest)?;
        return Ok(dest.display().to_string());
    }

    if let Some(literal) = raw_value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(literal.to_string());
    }

    if raw_value.starts_with('/') {
        let zip_path = raw_value.trim_start_matches('/');
        let dest = safe_join(scratch_dir, zip_path)
            .ok_or_else(|| LoaderError::Other(format!("chemin invalide: {zip_path}")))?;
        extract_zip_entry(archive, zip_path, &dest)?;
        return Ok(dest.display().to_string());
    }

    Ok(raw_value.to_string())
}

pub(super) fn read_main_class(jar_path: &Path) -> Result<String, LoaderError> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let manifest_text = read_zip_text(&mut archive, "META-INF/MANIFEST.MF")?;
    for line in manifest_text.lines() {
        if let Some(value) = line.strip_prefix("Main-Class:") {
            return Ok(value.trim().to_string());
        }
    }
    Err(LoaderError::Other(format!(
        "pas de Main-Class dans {}",
        jar_path.display()
    )))
}
