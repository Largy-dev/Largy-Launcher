//! Recognizes an archive the user dropped on the launcher: a Modrinth
//! `.mrpack`, a CurseForge modpack zip, or a Prism/MultiMC instance export.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::providers::archive;
use crate::providers::LoaderKind;

#[derive(Debug, PartialEq)]
pub enum ImportKind {
    Mrpack,
    CurseForge,
    /// `root` is the folder inside the zip holding `mmc-pack.json`.
    Prism { root: String },
}

pub fn detect(path: &Path) -> AppResult<ImportKind> {
    let names = archive::entry_names(path)?;
    if names.iter().any(|n| n == "modrinth.index.json") {
        return Ok(ImportKind::Mrpack);
    }
    if names.iter().any(|n| n == "manifest.json") {
        return Ok(ImportKind::CurseForge);
    }
    if let Some(entry) = names.iter().filter(|n| n.ends_with("mmc-pack.json")).min_by_key(|n| n.len()) {
        return Ok(ImportKind::Prism { root: entry.trim_end_matches("mmc-pack.json").to_string() });
    }
    Err(AppError::Instance(
        "Format non reconnu : importe un .mrpack, un zip de modpack CurseForge ou une instance Prism/MultiMC."
            .to_string(),
    ))
}

#[derive(Debug, Deserialize)]
struct MmcPack {
    #[serde(default)]
    components: Vec<MmcComponent>,
}

#[derive(Debug, Deserialize)]
struct MmcComponent {
    uid: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct PrismInstance {
    pub name: Option<String>,
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    /// Folder inside the zip that becomes the instance directory.
    pub game_dir_prefix: String,
}

fn cfg_value<'a>(cfg: &'a str, key: &str) -> Option<&'a str> {
    cfg.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(str::trim)
}

pub fn parse_prism(path: &Path, root: &str) -> AppResult<PrismInstance> {
    let pack_text = archive::read_text(path, &format!("{root}mmc-pack.json"))?
        .ok_or_else(|| AppError::Instance("mmc-pack.json introuvable".to_string()))?;
    let pack: MmcPack = serde_json::from_str(&pack_text)?;
    let cfg = archive::read_text(path, &format!("{root}instance.cfg"))?.unwrap_or_default();

    let version_of = |uid: &str| pack.components.iter().find(|c| c.uid == uid).and_then(|c| c.version.clone());
    let minecraft_version = version_of("net.minecraft")
        .ok_or_else(|| AppError::Instance("version de Minecraft absente de l'instance importée".to_string()))?;
    let (loader, loader_version) = [
        ("net.neoforged", LoaderKind::NeoForge),
        ("net.minecraftforge", LoaderKind::Forge),
        ("net.fabricmc.fabric-loader", LoaderKind::Fabric),
        ("org.quiltmc.quilt-loader", LoaderKind::Quilt),
    ]
    .into_iter()
    .find_map(|(uid, kind)| version_of(uid).map(|v| (kind, Some(v))))
    .unwrap_or((LoaderKind::Vanilla, None));

    let names = archive::entry_names(path)?;
    let game_dir_prefix = [".minecraft/", "minecraft/"]
        .iter()
        .map(|d| format!("{root}{d}"))
        .find(|prefix| names.iter().any(|n| n.starts_with(prefix.as_str())))
        .unwrap_or_else(|| format!("{root}.minecraft/"));

    let overrides_memory = cfg_value(&cfg, "OverrideMemory") == Some("true");
    let memory = |key| overrides_memory.then(|| cfg_value(&cfg, key).and_then(|v| v.parse().ok())).flatten();

    Ok(PrismInstance {
        name: cfg_value(&cfg, "name").map(str::to_string).filter(|n| !n.is_empty()),
        minecraft_version,
        loader,
        loader_version,
        min_memory_mb: memory("MinMemAlloc"),
        max_memory_mb: memory("MaxMemAlloc"),
        game_dir_prefix,
    })
}

/// Extracts the game folder of a Prism export to `extract_dir`, returning it.
pub fn extract_prism_game_dir(path: &Path, prefix: &str, extract_dir: &Path) -> AppResult<Option<PathBuf>> {
    Ok(archive::extract_prefixes(path, extract_dir, &[prefix])?.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, entries: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn detects_each_supported_format() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            ("a.mrpack", vec![("modrinth.index.json", "{}")], ImportKind::Mrpack),
            ("b.zip", vec![("manifest.json", "{}")], ImportKind::CurseForge),
            ("c.zip", vec![("My Pack/mmc-pack.json", "{}")], ImportKind::Prism { root: "My Pack/".to_string() }),
        ];
        for (name, entries, expected) in cases {
            let path = dir.path().join(name);
            make_zip(&path, &entries);
            assert_eq!(detect(&path).unwrap(), expected);
        }
        let other = dir.path().join("d.zip");
        make_zip(&other, &[("readme.txt", "hi")]);
        assert!(detect(&other).is_err());
    }

    #[test]
    fn parses_a_prism_export() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prism.zip");
        make_zip(
            &path,
            &[
                (
                    "mmc-pack.json",
                    r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.15.11"}]}"#,
                ),
                ("instance.cfg", "name=Survie\nOverrideMemory=true\nMaxMemAlloc=6144\nMinMemAlloc=1024\n"),
                (".minecraft/options.txt", "x"),
            ],
        );
        let parsed = parse_prism(&path, "").unwrap();
        assert_eq!(
            parsed,
            PrismInstance {
                name: Some("Survie".to_string()),
                minecraft_version: "1.20.1".to_string(),
                loader: LoaderKind::Fabric,
                loader_version: Some("0.15.11".to_string()),
                min_memory_mb: Some(1024),
                max_memory_mb: Some(6144),
                game_dir_prefix: ".minecraft/".to_string(),
            }
        );
    }
}
