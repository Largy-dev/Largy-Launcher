use super::*;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn game_dir(dir: &Path, mods: usize, worlds: usize) {
    for i in 0..mods {
        write(&dir.join(format!("mods/mod{i}.jar")), "jar");
    }
    for i in 0..worlds {
        write(&dir.join(format!("saves/World{i}/level.dat")), "dat");
    }
    write(&dir.join("config/a.toml"), "x=1");
    write(&dir.join("options.txt"), "fov:0.5");
    write(&dir.join("logs/latest.log"), "log");
}

fn roots(base: &Path) -> Roots {
    Roots {
        official: vec![base.join(".minecraft")],
        prism: vec![base.join("PrismLauncher/instances")],
        curseforge: vec![base.join("curseforge/Instances")],
        modrinth: vec![base.join("ModrinthApp/profiles")],
    }
}

#[test]
fn version_ids_map_to_loaders() {
    assert_eq!(
        loader_from_version_id("fabric-loader-0.15.11-1.20.1", "1.20.1"),
        (LoaderKind::Fabric, Some("0.15.11".into()))
    );
    assert_eq!(loader_from_version_id("quilt-loader-0.26.0-1.20.1", "1.20.1"), (LoaderKind::Quilt, Some("0.26.0".into())));
    assert_eq!(loader_from_version_id("neoforge-21.1.77", "1.21.1"), (LoaderKind::NeoForge, Some("21.1.77".into())));
    assert_eq!(loader_from_version_id("1.20.1-forge-47.2.0", "1.20.1"), (LoaderKind::Forge, Some("47.2.0".into())));
    assert_eq!(
        loader_from_version_id("1.7.10-Forge10.13.4.1614-1.7.10", "1.7.10"),
        (LoaderKind::Forge, Some("10.13.4.1614-1.7.10".into()))
    );
    assert_eq!(loader_from_version_id("1.21.1", "1.21.1"), (LoaderKind::Vanilla, None));
    assert_eq!(loader_from_version_id("1.20.1-OptiFine_HD_U_I6", "1.20.1"), (LoaderKind::Vanilla, None));
}

#[test]
fn finds_instances_of_every_launcher() {
    let base = tempfile::tempdir().unwrap();
    let b = base.path();

    let official = b.join(".minecraft");
    game_dir(&official, 2, 1);
    let custom = b.join("modded");
    game_dir(&custom, 5, 0);
    write(
        &official.join("launcher_profiles.json"),
        &format!(
            r#"{{"profiles": {{
                "a": {{"type": "latest-release", "lastVersionId": "latest-release", "lastUsed": "2024-01-01T00:00:00Z"}},
                "b": {{"type": "custom", "name": "Fabric", "lastVersionId": "fabric-loader-0.15.11-1.20.1",
                       "gameDir": {}, "lastUsed": "2025-01-01T00:00:00Z"}},
                "c": {{"type": "latest-snapshot", "lastVersionId": "latest-snapshot"}}
            }}}}"#,
            serde_json::to_string(&custom.display().to_string()).unwrap()
        ),
    );
    write(
        &official.join("versions/fabric-loader-0.15.11-1.20.1/fabric-loader-0.15.11-1.20.1.json"),
        r#"{"inheritsFrom": "1.20.1"}"#,
    );

    let prism = b.join("PrismLauncher/instances/Create");
    game_dir(&prism.join(".minecraft"), 3, 2);
    write(
        &prism.join("mmc-pack.json"),
        r#"{"components": [{"uid": "net.minecraft", "version": "1.20.1"}, {"uid": "net.minecraftforge", "version": "47.2.0"}]}"#,
    );
    write(&prism.join("instance.cfg"), "name=Create Pack\nlastLaunchTime=1700000000000\n");

    let cf = b.join("curseforge/Instances/ATM9");
    game_dir(&cf, 4, 0);
    write(
        &cf.join("minecraftinstance.json"),
        r#"{"name": "All the Mods 9", "gameVersion": "1.20.1", "baseModLoader": {"name": "forge-47.2.0"}}"#,
    );

    let mr = b.join("ModrinthApp/profiles/FO");
    game_dir(&mr, 1, 0);
    write(
        &mr.join("profile.json"),
        r#"{"metadata": {"name": "Fabulously Optimized", "game_version": "1.21.1", "loader": "fabric",
            "loader_version": {"id": "0.16.5"}}}"#,
    );

    let found = detect(&roots(b), Some("1.21.1"), &[format!("prism:{}", prism.join(".minecraft").display()).replace('\\', "/")]);
    let summary: Vec<_> = found
        .iter()
        .map(|e| (e.source, e.name.as_str(), e.minecraft_version.as_str(), e.loader, e.mods, e.worlds, e.already_imported))
        .collect();

    assert_eq!(summary.len(), 5, "{summary:?}");
    assert_eq!(summary[0], (ExternalSource::Official, "Fabric", "1.20.1", LoaderKind::Fabric, 5, 0, false));
    assert!(summary.contains(&(ExternalSource::Official, "Minecraft 1.21.1", "1.21.1", LoaderKind::Vanilla, 2, 1, false)));
    assert!(summary.contains(&(ExternalSource::Prism, "Create Pack", "1.20.1", LoaderKind::Forge, 3, 2, true)));
    assert!(summary.contains(&(ExternalSource::Curseforge, "All the Mods 9", "1.20.1", LoaderKind::Forge, 4, 0, false)));
    assert!(summary.contains(&(ExternalSource::Modrinth, "Fabulously Optimized", "1.21.1", LoaderKind::Fabric, 1, 0, false)));
    let cf_entry = found.iter().find(|e| e.source == ExternalSource::Curseforge).unwrap();
    assert_eq!(cf_entry.loader_version.as_deref(), Some("47.2.0"));
}

#[test]
fn import_copies_content_but_not_logs_and_worlds_only_on_request() {
    let base = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(base.path().join("data"));
    let prism = base.path().join("PrismLauncher/instances/Create");
    game_dir(&prism.join(".minecraft"), 3, 2);
    write(
        &prism.join("mmc-pack.json"),
        r#"{"components": [{"uid": "net.minecraft", "version": "1.20.1"}, {"uid": "net.minecraftforge", "version": "47.2.0"}]}"#,
    );
    let found = detect(&roots(base.path()), None, &[]);

    let without = import(&paths, &found[0], false).unwrap();
    assert_eq!(std::fs::read_dir(without.directory.join("mods")).unwrap().count(), 3);
    assert!(without.directory.join("config/a.toml").is_file());
    assert!(without.directory.join("options.txt").is_file());
    assert!(!without.directory.join("logs").exists());
    assert_eq!(std::fs::read_dir(without.directory.join("saves")).unwrap().count(), 0);
    assert_eq!(without.imported_from.as_deref(), Some(found[0].id.as_str()));
    assert_eq!((without.loader, without.loader_version.as_deref()), (LoaderKind::Forge, Some("47.2.0")));

    let with = import(&paths, &found[0], true).unwrap();
    assert_eq!(std::fs::read_dir(with.directory.join("saves")).unwrap().count(), 2);
}
