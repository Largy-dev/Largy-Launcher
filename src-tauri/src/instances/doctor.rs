//! Known problems an instance can have that the launcher knows how to fix —
//! the checklist behind the "Réparer" button. Each check is cheap (a few
//! file reads), so the list is recomputed every time the dialog opens.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::providers::LoaderKind;
use crate::util::fs::write_atomic;
use crate::util::version::compare_versions;

use super::Instance;

const OPTIONS: &str = "options.txt";
const MENU_BLUR: &str = "menuBackgroundBlurriness";
/// Minecraft's default menu blur, used when `options.txt` doesn't say.
const DEFAULT_MENU_BLUR: &str = "5";

pub const IRIS_MENU_BLUR: &str = "iris-menu-blur";
pub const MISSING_MODPACK_FILES: &str = "missing-modpack-files";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct KnownIssue {
    /// Stable id, handed back to [`fix`].
    pub id: String,
    pub title: String,
    pub description: String,
}

/// Every known problem found in `instance` right now.
pub fn diagnose(instance: &Instance) -> Vec<KnownIssue> {
    let mut issues = Vec::new();
    if iris_menu_blur_applies(instance) {
        issues.push(KnownIssue {
            id: IRIS_MENU_BLUR.to_string(),
            title: "Écran des shaders vide".to_string(),
            description:
                "Avec Iris sur Forge/NeoForge, le flou d'arrière-plan des menus rend la liste des shaders invisible. \
                          Le flou des menus sera mis à 0 (Options › Accessibilité)."
                    .to_string(),
        });
    }
    let missing = missing_modpack_files(instance);
    if !missing.is_empty() {
        issues.push(KnownIssue {
            id: MISSING_MODPACK_FILES.to_string(),
            title: format!(
                "{} fichier{} du modpack manquant{}",
                missing.len(),
                if missing.len() > 1 { "s" } else { "" },
                if missing.len() > 1 { "s" } else { "" }
            ),
            description: "Ils seront retéléchargés depuis le modpack, sans toucher à tes réglages ni à tes mondes."
                .to_string(),
        });
    }
    issues
}

/// Applies the fix for one issue that lives entirely on disk. Modpack files
/// are re-downloaded by the repair command itself, so they're a no-op here.
pub fn fix(instance: &Instance, issue_id: &str) -> AppResult<()> {
    match issue_id {
        IRIS_MENU_BLUR => set_option(&instance.directory, MENU_BLUR, "0"),
        MISSING_MODPACK_FILES => Ok(()),
        other => Err(AppError::Instance(format!("problème inconnu : {other}"))),
    }
}

/// Iris (or its Forge ports) on a Forge-family loader, on a version with the
/// menu blur (1.20.5+ until the 26.x line, where it was fixed), with a blur
/// above zero: the shader pack list renders invisible.
fn iris_menu_blur_applies(instance: &Instance) -> bool {
    let v = instance.minecraft_version.as_str();
    let affected_version = v.starts_with("1.") && compare_versions(v, "1.20.5") != std::cmp::Ordering::Less;
    if !affected_version || !matches!(instance.loader, LoaderKind::Forge | LoaderKind::NeoForge) {
        return false;
    }
    if !has_enabled_mod(&instance.directory, &["iris", "oculus", "monocle"]) {
        return false;
    }
    // A brand-new instance has no options yet: nothing to fix before the game
    // writes them (and writing a bare file would confuse Minecraft's upgrade
    // of older option files).
    let Ok(text) = std::fs::read_to_string(instance.directory.join(OPTIONS)) else {
        return false;
    };
    let blur = read_option(&text, MENU_BLUR).unwrap_or(DEFAULT_MENU_BLUR);
    blur.trim().parse::<f64>().map_or(true, |b| b > 0.0)
}

/// An enabled `.jar` in `mods/` whose name starts with one of `prefixes`.
fn has_enabled_mod(instance_dir: &Path, prefixes: &[&str]) -> bool {
    let Ok(entries) = std::fs::read_dir(instance_dir.join("mods")) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        name.ends_with(".jar") && prefixes.iter().any(|p| name.starts_with(p))
    })
}

/// Files the installed modpack version put in `mods/` that are gone — neither
/// the file nor its `.disabled` twin is there anymore.
pub fn missing_modpack_files(instance: &Instance) -> Vec<PathBuf> {
    let Some(pack) = &instance.modpack else {
        return Vec::new();
    };
    pack.installed_files
        .iter()
        .filter(|rel| rel.starts_with("mods"))
        .filter(|rel| {
            let path = instance.directory.join(rel);
            !path.exists() && !disabled_twin(&path).exists()
        })
        .cloned()
        .collect()
}

/// `mods/x.jar` → `mods/x.jar.disabled`, the name the launcher gives a mod
/// the player turned off.
pub fn disabled_twin(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".disabled");
    PathBuf::from(name)
}

fn read_option<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| line.split_once(':').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim()))
}

/// Sets one `key:value` in `options.txt`, keeping every other line in place.
fn set_option(instance_dir: &Path, key: &str, value: &str) -> AppResult<()> {
    let path = instance_dir.join(OPTIONS);
    let text = std::fs::read_to_string(&path)?;
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| match line.split_once(':') {
            Some((k, _)) if k.trim() == key => {
                found = true;
                format!("{key}:{value}")
            }
            _ => line.trim_end_matches('\r').to_string(),
        })
        .collect();
    if !found {
        lines.push(format!("{key}:{value}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    write_atomic(&path, out.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::ModpackRef;

    fn instance(dir: &Path, loader: LoaderKind, version: &str) -> Instance {
        serde_json::from_value(serde_json::json!({
            "id": "i",
            "name": "Test",
            "minecraft_version": version,
            "loader": loader,
            "loader_version": null,
            "directory": dir,
        }))
        .unwrap()
    }

    fn setup(options: Option<&str>, mods: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("mods")).unwrap();
        for m in mods {
            std::fs::write(dir.path().join("mods").join(m), b"").unwrap();
        }
        if let Some(text) = options {
            std::fs::write(dir.path().join(OPTIONS), text).unwrap();
        }
        dir
    }

    #[test]
    fn menu_blur_with_iris_on_neoforge_is_flagged_and_fixed() {
        let dir = setup(Some("version:3955\nfov:0.5\nmenuBackgroundBlurriness:5\n"), &["iris-neoforge-1.8.12.jar"]);
        let inst = instance(dir.path(), LoaderKind::NeoForge, "1.21.1");
        let issues = diagnose(&inst);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, IRIS_MENU_BLUR);

        fix(&inst, IRIS_MENU_BLUR).unwrap();
        let text = std::fs::read_to_string(dir.path().join(OPTIONS)).unwrap();
        assert_eq!(text, "version:3955\nfov:0.5\nmenuBackgroundBlurriness:0\n");
        assert!(diagnose(&inst).is_empty());
    }

    #[test]
    fn default_blur_counts_and_the_key_is_added() {
        let dir = setup(Some("version:3955\r\nfov:0.5\r\n"), &["oculus-mc1.20.6.jar"]);
        let inst = instance(dir.path(), LoaderKind::Forge, "1.20.6");
        assert_eq!(diagnose(&inst).len(), 1);
        fix(&inst, IRIS_MENU_BLUR).unwrap();
        let text = std::fs::read_to_string(dir.path().join(OPTIONS)).unwrap();
        assert_eq!(text, "version:3955\nfov:0.5\nmenuBackgroundBlurriness:0\n");
    }

    #[test]
    fn menu_blur_is_left_alone_when_it_cannot_be_the_cause() {
        let blur = Some("version:3955\nmenuBackgroundBlurriness:5\n");
        // Fabric's Iris draws its own list fine.
        let d = setup(blur, &["iris-fabric.jar"]);
        assert!(diagnose(&instance(d.path(), LoaderKind::Fabric, "1.21.1")).is_empty());
        // Disabled Iris, no Iris, no menu blur yet, newer line, no options yet.
        let d = setup(blur, &["iris-neoforge.jar.disabled"]);
        assert!(diagnose(&instance(d.path(), LoaderKind::NeoForge, "1.21.1")).is_empty());
        let d = setup(blur, &["sodium.jar"]);
        assert!(diagnose(&instance(d.path(), LoaderKind::NeoForge, "1.21.1")).is_empty());
        let d = setup(blur, &["oculus.jar"]);
        assert!(diagnose(&instance(d.path(), LoaderKind::Forge, "1.20.1")).is_empty());
        let d = setup(blur, &["iris.jar"]);
        assert!(diagnose(&instance(d.path(), LoaderKind::NeoForge, "26.3")).is_empty());
        let d = setup(None, &["iris.jar"]);
        assert!(diagnose(&instance(d.path(), LoaderKind::NeoForge, "1.21.1")).is_empty());
    }

    #[test]
    fn missing_pack_mods_are_found_but_disabled_ones_are_not_missing() {
        let dir = setup(None, &["here.jar", "off.jar.disabled"]);
        let mut inst = instance(dir.path(), LoaderKind::Fabric, "1.21.1");
        inst.modpack = Some(ModpackRef {
            provider: "modrinth".into(),
            pack_id: "p".into(),
            version_id: "v".into(),
            pack_name: "Pack".into(),
            installed_files: ["mods/here.jar", "mods/off.jar", "mods/gone.jar", "config/gone.toml"]
                .map(PathBuf::from)
                .to_vec(),
        });
        assert_eq!(missing_modpack_files(&inst), vec![PathBuf::from("mods/gone.jar")]);
        let issues = diagnose(&inst);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].title, "1 fichier du modpack manquant");
    }

    #[test]
    fn unknown_issues_are_refused() {
        let dir = setup(None, &[]);
        assert!(fix(&instance(dir.path(), LoaderKind::Vanilla, "1.21.1"), "nope").is_err());
    }
}
