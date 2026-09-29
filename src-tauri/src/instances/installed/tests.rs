use std::io::Write;

use super::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nicon";

fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

struct Fixture {
    _root: tempfile::TempDir,
    paths: AppPaths,
    instance: PathBuf,
}

fn setup() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(root.path().join("data"));
    let instance = root.path().join("inst");
    for sub in ["mods", "resourcepacks", "shaderpacks"] {
        std::fs::create_dir_all(instance.join(sub)).unwrap();
    }
    Fixture { _root: root, paths, instance }
}

#[test]
fn lists_mods_with_their_metadata_and_icon() {
    let f = setup();
    let sodium = jar(&[
        ("fabric.mod.json", br#"{"id":"sodium","name":"Sodium","version":"0.5","icon":"icon.png","depends":{"indium":"*"}}"#),
        ("icon.png", PNG),
    ]);
    std::fs::write(f.instance.join("mods/sodium.jar"), &sodium).unwrap();
    std::fs::write(f.instance.join("mods/old.jar.disabled"), jar(&[])).unwrap();
    std::fs::write(f.instance.join("mods/readme.txt"), b"x").unwrap();

    let items = list(&f.paths, &f.instance, LoaderKind::Fabric, ContentKind::Mod).unwrap();

    assert_eq!(items.len(), 2);
    let old = &items[0];
    assert_eq!((old.file_name.as_str(), old.enabled), ("old.jar", false));
    let s = &items[1];
    assert_eq!(s.name.as_deref(), Some("Sodium"));
    assert_eq!(s.depends, vec!["indium"]);
    assert_eq!(s.sha1.as_deref(), Some(hex::encode(Sha1::digest(&sodium)).as_str()));
    let icon = s.icon_path.as_ref().expect("icon extracted");
    assert_eq!(std::fs::read(icon).unwrap(), PNG);
}

#[test]
fn second_listing_is_served_from_the_cache() {
    let f = setup();
    let path = f.instance.join("mods/a.jar");
    std::fs::write(&path, jar(&[("fabric.mod.json", br#"{"id":"a","name":"A"}"#)])).unwrap();
    assert_eq!(list(&f.paths, &f.instance, LoaderKind::Fabric, ContentKind::Mod).unwrap()[0].name.as_deref(), Some("A"));

    // Same key (name, size, mtime) → the cached metadata wins, the jar isn't re-read.
    cache::update(&f.paths, |entries| {
        for e in entries.values_mut() {
            e.meta.name = Some("Cached".into());
        }
    });
    assert_eq!(list(&f.paths, &f.instance, LoaderKind::Fabric, ContentKind::Mod).unwrap()[0].name.as_deref(), Some("Cached"));
}

#[test]
fn resource_packs_include_folders_and_shaders_any_zip() {
    let f = setup();
    std::fs::write(
        f.instance.join("resourcepacks/faithful.zip"),
        jar(&[("pack.mcmeta", br#"{"pack":{"description":"Faithful"}}"#)]),
    )
    .unwrap();
    let folder = f.instance.join("resourcepacks/Mine");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("pack.mcmeta"), br#"{"pack":{"description":"Mine"}}"#).unwrap();
    std::fs::create_dir_all(f.instance.join("resourcepacks/not-a-pack")).unwrap();
    std::fs::write(f.instance.join("shaderpacks/Complementary.zip"), jar(&[])).unwrap();

    let packs = list(&f.paths, &f.instance, LoaderKind::Fabric, ContentKind::ResourcePack).unwrap();
    let names: Vec<_> = packs.iter().map(|p| (p.file_name.as_str(), p.is_dir, p.description.as_deref())).collect();
    assert_eq!(names, vec![("faithful.zip", false, Some("Faithful")), ("Mine", true, Some("Mine"))]);
    assert_eq!(list(&f.paths, &f.instance, LoaderKind::Fabric, ContentKind::Shader).unwrap().len(), 1);
}

#[test]
fn summary_counts_without_reading_files() {
    let f = setup();
    std::fs::write(f.instance.join("mods/a.jar"), b"x").unwrap();
    std::fs::write(f.instance.join("mods/b.jar.disabled"), b"x").unwrap();
    std::fs::write(f.instance.join("mods/notes.txt"), b"x").unwrap();
    std::fs::write(f.instance.join("resourcepacks/p.zip"), b"x").unwrap();
    std::fs::create_dir_all(f.instance.join("saves/W")).unwrap();
    std::fs::write(f.instance.join("saves/W/level.dat"), b"x").unwrap();
    std::fs::create_dir_all(f.instance.join("saves/empty")).unwrap();

    assert_eq!(
        summary(&f.instance),
        ContentSummary { mods: 2, mods_enabled: 1, resource_packs: 1, shaders: 0, worlds: 1 }
    );
}

#[test]
fn enabling_disabling_and_deleting() {
    let f = setup();
    std::fs::write(f.instance.join("mods/a.jar"), b"x").unwrap();

    set_enabled(&f.instance, ContentKind::Mod, "a.jar", false).unwrap();
    assert!(f.instance.join("mods/a.jar.disabled").exists());
    // Already in the wanted state: a no-op, not an error (bulk actions).
    set_enabled(&f.instance, ContentKind::Mod, "a.jar", false).unwrap();
    set_enabled(&f.instance, ContentKind::Mod, "a.jar", true).unwrap();
    assert!(f.instance.join("mods/a.jar").exists());

    delete(&f.instance, ContentKind::Mod, "a.jar").unwrap();
    assert!(!f.instance.join("mods/a.jar").exists());
    assert!(delete(&f.instance, ContentKind::Mod, "a.jar").is_err());
    assert!(set_enabled(&f.instance, ContentKind::Mod, "missing.jar", true).is_err());
}

#[test]
fn folders_can_be_deleted_but_not_disabled() {
    let f = setup();
    let folder = f.instance.join("shaderpacks/BSL");
    std::fs::create_dir_all(folder.join("shaders")).unwrap();
    assert!(set_enabled(&f.instance, ContentKind::Shader, "BSL", false).is_err());
    delete(&f.instance, ContentKind::Shader, "BSL").unwrap();
    assert!(!folder.exists());
}

#[test]
fn names_with_path_components_are_rejected() {
    let f = setup();
    std::fs::write(f.instance.join("victim.jar"), b"x").unwrap();
    assert!(delete(&f.instance, ContentKind::Mod, "../victim.jar").is_err());
    assert!(set_enabled(&f.instance, ContentKind::Mod, "../victim.jar", false).is_err());
    assert!(f.instance.join("victim.jar").exists());
}

#[test]
fn adding_checks_extension_and_archive_header() {
    let f = setup();
    let src = tempfile::tempdir().unwrap();
    let good = src.path().join("cool.jar");
    std::fs::write(&good, jar(&[])).unwrap();
    let fake = src.path().join("fake.jar");
    std::fs::write(&fake, b"not a zip").unwrap();
    let pack = src.path().join("pack.zip");
    std::fs::write(&pack, jar(&[])).unwrap();

    assert_eq!(add_from_path(&f.instance, ContentKind::Mod, &good).unwrap(), "cool.jar");
    assert!(f.instance.join("mods/cool.jar").exists());
    assert!(add_from_path(&f.instance, ContentKind::Mod, &fake).is_err());
    assert!(add_from_path(&f.instance, ContentKind::Mod, &pack).is_err());
    add_from_path(&f.instance, ContentKind::ResourcePack, &pack).unwrap();
    assert!(f.instance.join("resourcepacks/pack.zip").exists());
}

#[test]
fn modrinth_urls_use_the_project_type() {
    assert_eq!(modrinth_url("mod", "sodium"), "https://modrinth.com/mod/sodium");
    assert_eq!(modrinth_url("shader", "bsl"), "https://modrinth.com/shader/bsl");
    assert_eq!(modrinth_url("", "x"), "https://modrinth.com/mod/x");
}
