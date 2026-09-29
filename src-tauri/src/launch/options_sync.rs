//! Same controls everywhere: the game options a player sets once for
//! themselves — key bindings, mouse sensitivity, FOV, volumes, language,
//! accessibility — are carried from one instance to the next through a
//! shared copy, merged into `options.txt` before a launch and read back
//! after the game closes. Settings that depend on the pack (render distance,
//! graphics, resource packs…) are left alone.
//!
//! Minecraft 1.13 changed how key bindings are written (`key.keyboard.w`
//! instead of key codes), so older versions share a separate copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths::AppPaths;
use crate::util::fs::write_atomic;
use crate::util::version::compare_versions;

const OPTIONS: &str = "options.txt";

/// Exact keys shared between instances (besides the prefixes below).
const SHARED_KEYS: &[&str] = &[
    "mouseSensitivity",
    "invertYMouse",
    "rawMouseInput",
    "discrete_mouse_scroll",
    "mouseWheelSensitivity",
    "fov",
    "fovEffectScale",
    "screenEffectScale",
    "gamma",
    "guiScale",
    "lang",
    "autoJump",
    "toggleCrouch",
    "toggleSprint",
    "sneakToggle",
    "attackIndicator",
    "mainHand",
    "bobView",
    "chatScale",
    "chatOpacity",
    "chatWidth",
    "chatHeightFocused",
    "chatHeightUnfocused",
    "chatLineSpacing",
    "chatColors",
    "chatLinks",
    "chatLinksPrompt",
    "textBackgroundOpacity",
    "backgroundForChatOnly",
    "narrator",
    "showSubtitles",
    "directionalAudio",
    "darkMojangStudiosBackground",
    "hideLightningFlashes",
    "damageTiltStrength",
    "highContrast",
    "notificationDisplayTime",
    "glintSpeed",
    "glintStrength",
    "panoramaScrollSpeed",
    "hideMatchedNames",
    "operatorItemsTab",
    "skipMultiplayerWarning",
    "onboardAccessibility",
    "reducedDebugInfo",
];

/// Key prefixes shared between instances: key bindings, volumes, skin layers.
const SHARED_PREFIXES: &[&str] = &["key_", "soundCategory_", "modelPart_"];

pub fn is_shared(key: &str) -> bool {
    SHARED_KEYS.contains(&key) || SHARED_PREFIXES.iter().any(|p| key.starts_with(p))
}

/// `key:value` lines; anything else is kept as is by [`merge`].
fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim_end_matches('\r').to_string()))
        .collect()
}

/// The shared options found in an `options.txt`.
pub fn extract(text: &str) -> BTreeMap<String, String> {
    parse(text).into_iter().filter(|(k, _)| is_shared(k)).collect()
}

/// `text` with the shared values applied: existing lines keep their place,
/// shared options it doesn't have yet are appended.
pub fn merge(text: &str, shared: &BTreeMap<String, String>) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = text
        .lines()
        .map(|line| match line.split_once(':') {
            Some((k, _)) if shared.contains_key(k.trim()) => {
                seen.insert(k.trim().to_string());
                format!("{}:{}", k.trim(), shared[k.trim()])
            }
            _ => line.to_string(),
        })
        .collect();
    out.extend(shared.iter().filter(|(k, _)| !seen.contains(*k)).map(|(k, v)| format!("{k}:{v}")));
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

/// Where the shared copy for `minecraft_version` lives.
fn shared_file(paths: &AppPaths, minecraft_version: &str) -> PathBuf {
    let modern = minecraft_version.contains('w')
        || !minecraft_version.starts_with("1.")
        || compare_versions(minecraft_version, "1.13") != std::cmp::Ordering::Less;
    let modern = modern && !minecraft_version.starts_with(['a', 'b', 'c', 'i', 'r']);
    paths.root().join("shared-options").join(if modern { "options.txt" } else { "options-legacy.txt" })
}

/// Before a launch: the shared options into the instance's `options.txt`.
/// Only an existing file the game wrote (it has a `version:` line) is
/// touched: without that line Minecraft runs its pre-1.13 key-binding
/// conversions on the modern values and mangles them. A brand-new instance
/// gets the shared options from its second launch on.
pub fn apply(paths: &AppPaths, instance_dir: &Path, minecraft_version: &str) -> std::io::Result<()> {
    let Ok(shared_text) = std::fs::read_to_string(shared_file(paths, minecraft_version)) else {
        return Ok(());
    };
    let shared = extract(&shared_text);
    if shared.is_empty() {
        return Ok(());
    }
    let target = instance_dir.join(OPTIONS);
    let Ok(current) = std::fs::read_to_string(&target) else {
        return Ok(());
    };
    if !current.lines().any(|l| l.starts_with("version:")) {
        return Ok(());
    }
    let merged = merge(&current, &shared);
    if merged != current {
        write_atomic(&target, merged.as_bytes())?;
    }
    Ok(())
}

/// After the game closed: what the player changed becomes the shared copy.
pub fn collect(paths: &AppPaths, instance_dir: &Path, minecraft_version: &str) -> std::io::Result<()> {
    let Ok(text) = std::fs::read_to_string(instance_dir.join(OPTIONS)) else {
        return Ok(());
    };
    let options = extract(&text);
    if options.is_empty() {
        return Ok(());
    }
    let file = shared_file(paths, minecraft_version);
    let current = std::fs::read_to_string(&file).unwrap_or_default();
    let merged = merge(&current, &options);
    if merged != current {
        write_atomic(&file, merged.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_personal_options_are_shared() {
        let options = extract("fov:0.25\nrenderDistance:24\nkey_key.jump:key.keyboard.space\nsoundCategory_music:0.0\nresourcePacks:[\"vanilla\"]\nmodelPart_cape:true\n");
        assert_eq!(options.len(), 4);
        assert!(!options.contains_key("renderDistance") && !options.contains_key("resourcePacks"));
    }

    #[test]
    fn merging_keeps_order_updates_values_and_appends_new_keys() {
        let shared = extract("fov:0.5\nkey_key.jump:key.keyboard.x\nlang:fr_fr\n");
        let merged = merge("version:3955\nfov:0.0\nrenderDistance:12\nlang:en_us\n", &shared);
        assert_eq!(merged, "version:3955\nfov:0.5\nrenderDistance:12\nlang:fr_fr\nkey_key.jump:key.keyboard.x\n");
    }

    #[test]
    fn options_travel_from_one_instance_to_the_next() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(root.path().join("data"));
        let (a, b, old) = (root.path().join("a"), root.path().join("b"), root.path().join("old"));
        for dir in [&a, &b, &old] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(a.join(OPTIONS), "fov:0.8\nrenderDistance:32\nkey_key.drop:key.keyboard.g\n").unwrap();
        std::fs::write(b.join(OPTIONS), "version:3955\nrenderDistance:6\nfov:0.0\n").unwrap();

        collect(&paths, &a, "1.21.1").unwrap();
        apply(&paths, &b, "1.20.1").unwrap();
        apply(&paths, &old, "1.12.2").unwrap();

        let b_options = std::fs::read_to_string(b.join(OPTIONS)).unwrap();
        assert_eq!(b_options, "version:3955\nrenderDistance:6\nfov:0.8\nkey_key.drop:key.keyboard.g\n");
        // Pre-1.13 key bindings aren't compatible: that era has its own copy.
        assert!(!old.join(OPTIONS).exists());
    }

    #[test]
    fn files_the_game_has_not_written_are_left_alone() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(root.path().join("data"));
        let (a, fresh, unversioned) = (root.path().join("a"), root.path().join("fresh"), root.path().join("u"));
        for dir in [&a, &fresh, &unversioned] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(a.join(OPTIONS), "version:3955\nfov:0.8\n").unwrap();
        std::fs::write(unversioned.join(OPTIONS), "fov:0.1\n").unwrap();
        collect(&paths, &a, "1.21.1").unwrap();

        apply(&paths, &fresh, "1.21.1").unwrap();
        apply(&paths, &unversioned, "1.21.1").unwrap();

        assert!(!fresh.join(OPTIONS).exists());
        assert_eq!(std::fs::read_to_string(unversioned.join(OPTIONS)).unwrap(), "fov:0.1\n");
    }

    #[test]
    fn eras_are_told_apart() {
        let paths = AppPaths::from_root(PathBuf::from("/data"));
        let name = |v: &str| shared_file(&paths, v).file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name("1.21.1"), "options.txt");
        assert_eq!(name("26.3"), "options.txt");
        assert_eq!(name("24w14a"), "options.txt");
        assert_eq!(name("1.12.2"), "options-legacy.txt");
        assert_eq!(name("b1.7.3"), "options-legacy.txt");
    }
}
