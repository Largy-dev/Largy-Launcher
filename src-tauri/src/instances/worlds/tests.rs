use std::io::Write;

use serde::Serialize;

use super::*;

#[derive(Serialize)]
struct TestLevel {
    #[serde(rename = "Data")]
    data: TestData,
}

#[derive(Serialize)]
struct TestData {
    #[serde(rename = "LevelName")]
    level_name: String,
    #[serde(rename = "GameType")]
    game_type: i32,
    hardcore: i8,
    #[serde(rename = "allowCommands")]
    allow_commands: i8,
    #[serde(rename = "LastPlayed")]
    last_played: i64,
    #[serde(rename = "Version")]
    version: TestVersion,
}

#[derive(Serialize)]
struct TestVersion {
    #[serde(rename = "Name")]
    name: String,
}

fn level_dat(name: &str, last_played_ms: i64) -> Vec<u8> {
    let level = TestLevel {
        data: TestData {
            level_name: name.to_string(),
            game_type: 1,
            hardcore: 0,
            allow_commands: 1,
            last_played: last_played_ms,
            version: TestVersion { name: "1.21.1".to_string() },
        },
    };
    let nbt = fastnbt::to_bytes(&level).unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&nbt).unwrap();
    gz.finish().unwrap()
}

fn make_world(instance: &Path, folder: &str, name: &str, last_played_ms: i64) {
    let dir = instance.join("saves").join(folder);
    std::fs::create_dir_all(dir.join("region")).unwrap();
    std::fs::write(dir.join("level.dat"), level_dat(name, last_played_ms)).unwrap();
    std::fs::write(dir.join("region/r.0.0.mca"), b"chunks").unwrap();
    std::fs::write(dir.join("icon.png"), b"png").unwrap();
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
    std::fs::create_dir_all(instance.join("saves")).unwrap();
    Fixture { _root: root, paths, instance }
}

#[test]
fn lists_worlds_from_level_dat_newest_first() {
    let f = setup();
    make_world(&f.instance, "Old", "§aVieux monde", 1_000_000);
    make_world(&f.instance, "New", "Nouveau", 2_000_000);
    std::fs::create_dir_all(f.instance.join("saves/not-a-world")).unwrap();

    let worlds = list(&f.instance).unwrap();

    assert_eq!(worlds.len(), 2);
    let new = &worlds[0];
    assert_eq!((new.folder.as_str(), new.name.as_str()), ("New", "Nouveau"));
    assert_eq!(new.game_mode.as_deref(), Some("creative"));
    assert!(new.cheats && !new.hardcore);
    assert_eq!(new.last_played, Some(2_000));
    assert_eq!(new.version.as_deref(), Some("1.21.1"));
    assert!(new.size > 0 && new.icon_path.is_some());
    assert_eq!(worlds[1].name, "Vieux monde");
}

#[test]
fn a_world_backup_restores_next_to_the_original() {
    let f = setup();
    make_world(&f.instance, "Base", "Base", 1);

    let backup = backup(&f.paths, "id", &f.instance, "Base").unwrap();
    assert_eq!(backup.world.as_deref(), Some("Base"));
    assert!(backup.id.starts_with("worlds/Base/"));

    let restored = restore_backup(&f.paths, "id", &f.instance, &backup.id).unwrap();
    assert_eq!(restored, vec!["Base (2)"]);
    assert!(f.instance.join("saves/Base (2)/region/r.0.0.mca").is_file());
    assert!(f.instance.join("saves/Base/level.dat").is_file());

    delete(&f.instance, "Base").unwrap();
    assert_eq!(restore_backup(&f.paths, "id", &f.instance, &backup.id).unwrap(), vec!["Base"]);
}

#[test]
fn whole_saves_snapshots_are_listed_and_restorable() {
    let f = setup();
    make_world(&f.instance, "A", "A", 1);
    make_world(&f.instance, "B", "B", 1);
    crate::instances::backup::backup_saves(&f.paths, "id", &f.instance).unwrap();
    backup(&f.paths, "id", &f.instance, "A").unwrap();

    let backups = list_backups(&f.paths, "id");
    assert_eq!(backups.len(), 2);
    let full = backups.iter().find(|b| b.world.is_none()).unwrap();

    let mut restored = restore_backup(&f.paths, "id", &f.instance, &full.id).unwrap();
    restored.sort();
    assert_eq!(restored, vec!["A (2)", "B (2)"]);

    delete_backup(&f.paths, "id", &full.id).unwrap();
    assert_eq!(list_backups(&f.paths, "id").len(), 1);
    assert!(delete_backup(&f.paths, "id", "../../settings.json").is_err());
}

#[test]
fn per_world_backups_are_capped() {
    let f = setup();
    make_world(&f.instance, "W", "W", 1);
    let dir = world_backups_dir(&f.paths, "id", "W");
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..7 {
        std::fs::write(dir.join(format!("2000-01-0{i}.zip")), b"old").unwrap();
    }
    backup(&f.paths, "id", &f.instance, "W").unwrap();
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), KEEP_PER_WORLD);
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn imports_worlds_zipped_at_the_root_or_in_a_folder() {
    let f = setup();
    let src = tempfile::tempdir().unwrap();
    let dat = level_dat("Skyblock", 1);

    let root_zip = src.path().join("Skyblock Map.zip");
    std::fs::write(&root_zip, zip_of(&[("level.dat", &dat), ("region/r.0.0.mca", b"x")])).unwrap();
    assert_eq!(import(&f.instance, &root_zip).unwrap(), vec!["Skyblock Map"]);
    assert!(f.instance.join("saves/Skyblock Map/region/r.0.0.mca").is_file());

    let nested_zip = src.path().join("maps.zip");
    std::fs::write(
        &nested_zip,
        zip_of(&[("maps/Parkour/level.dat", &dat), ("maps/Parkour/DIM1/level.dat", &dat), ("readme.txt", b"hi")]),
    )
    .unwrap();
    assert_eq!(import(&f.instance, &nested_zip).unwrap(), vec!["Parkour"]);
    assert!(f.instance.join("saves/Parkour/DIM1/level.dat").is_file());

    let not_a_world = src.path().join("junk.zip");
    std::fs::write(&not_a_world, zip_of(&[("a.txt", b"x")])).unwrap();
    assert!(import(&f.instance, &not_a_world).is_err());
}

#[test]
fn imports_a_world_folder() {
    let f = setup();
    let src = tempfile::tempdir().unwrap();
    make_world(src.path(), "Creative", "Creative", 1);
    let imported = import(&f.instance, &src.path().join("saves/Creative")).unwrap();
    assert_eq!(imported, vec!["Creative"]);
    assert!(import(&f.instance, src.path()).is_err());
}

#[test]
fn folder_names_are_sanitised() {
    assert_eq!(sanitize("a/b:c*"), "a_b_c_");
    assert_eq!(sanitize("  World.  "), "World");
    assert!(delete(Path::new("/nowhere"), "../x").is_err());
}
