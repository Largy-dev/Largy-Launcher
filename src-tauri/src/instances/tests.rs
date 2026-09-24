use super::*;

fn test_input(name: &str) -> CreateInstanceInput {
    CreateInstanceInput {
        name: name.to_string(),
        minecraft_version: "1.20.1".to_string(),
        loader: LoaderKind::Vanilla,
        loader_version: None,
        modpack: None,
        icon_url: None,
    }
}

#[test]
fn create_then_get_round_trips_and_creates_subfolders() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());

    let created = create(&paths, test_input("Demo")).unwrap();
    let fetched = get(&paths, &created.id).unwrap();

    assert_eq!(fetched.name, "Demo");
    assert_eq!(fetched.minecraft_version, "1.20.1");
    for sub in ["mods", "saves", "config", "resourcepacks", "shaderpacks", "natives"] {
        assert!(created.directory.join(sub).is_dir());
    }
}

#[test]
fn list_sorts_newest_first_and_skips_unreadable_entries() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());

    let mut first = create(&paths, test_input("First")).unwrap();
    first.created_at = 100;
    save(&first).unwrap();

    let mut second = create(&paths, test_input("Second")).unwrap();
    second.created_at = 200;
    save(&second).unwrap();

    // An instance folder with a corrupt instance.json must not break listing.
    let broken_dir = paths.instances_dir().join("broken");
    std::fs::create_dir_all(&broken_dir).unwrap();
    std::fs::write(broken_dir.join("instance.json"), "not json").unwrap();

    let listed = list(&paths).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "Second");
    assert_eq!(listed[1].name, "First");
}

#[test]
fn get_missing_instance_returns_instance_error() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    assert!(matches!(get(&paths, "does-not-exist"), Err(AppError::Instance(_))));
}

#[test]
fn delete_removes_the_instance_directory() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("ToDelete")).unwrap();

    delete(&paths, &created.id).unwrap();

    assert!(!created.directory.exists());
    assert!(get(&paths, &created.id).is_err());
}

#[test]
fn touch_last_played_updates_and_persists_the_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Played")).unwrap();
    assert!(created.last_played_at.is_none());

    touch_last_played(&paths, &created.id).unwrap();

    let fetched = get(&paths, &created.id).unwrap();
    assert!(fetched.last_played_at.is_some());
}

#[test]
fn add_play_time_accumulates_across_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Timed")).unwrap();
    assert_eq!(created.play_time_seconds, 0);

    add_play_time(&paths, &created.id, 90).unwrap();
    add_play_time(&paths, &created.id, 30).unwrap();

    assert_eq!(get(&paths, &created.id).unwrap().play_time_seconds, 120);
}

#[test]
fn instances_saved_before_play_time_existed_default_to_zero() {
    let json = r#"{"id":"a","name":"Old","minecraft_version":"1.20.1","loader":"vanilla",
        "loader_version":null,"directory":"x"}"#;
    let instance: Instance = serde_json::from_str(json).unwrap();
    assert_eq!(instance.play_time_seconds, 0);
}

#[test]
fn ids_that_could_escape_the_instances_folder_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    create(&paths, test_input("Survivor")).unwrap();

    for bad in ["", ".", "..", "../x", "a/b", "a\\b"] {
        assert!(delete(&paths, bad).is_err(), "{bad:?} should be rejected");
        assert!(get(&paths, bad).is_err());
    }
    assert_eq!(list(&paths).unwrap().len(), 1);
}

#[test]
fn duplicate_copies_files_under_a_new_id_and_resets_stats() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let mut source = create(&paths, test_input("Original")).unwrap();
    source.play_time_seconds = 99;
    save(&source).unwrap();
    std::fs::write(source.directory.join("mods/a.jar"), b"jar").unwrap();

    let copy = duplicate(&paths, &source.id, "Copie").unwrap();

    assert_ne!(copy.id, source.id);
    assert_eq!(copy.name, "Copie");
    assert_eq!(copy.play_time_seconds, 0);
    assert_eq!(std::fs::read(copy.directory.join("mods/a.jar")).unwrap(), b"jar");
    assert_eq!(get(&paths, &copy.id).unwrap().directory, paths.instance_dir(&copy.id));
}

#[test]
fn rename_trims_and_persists_the_new_name() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Before")).unwrap();

    let renamed = rename(&paths, &created.id, "  After  ").unwrap();

    assert_eq!(renamed.name, "After");
    assert_eq!(get(&paths, &created.id).unwrap().name, "After");
}

#[test]
fn rename_rejects_empty_and_too_long_names() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Keep")).unwrap();

    assert!(rename(&paths, &created.id, "   ").is_err());
    assert!(rename(&paths, &created.id, &"x".repeat(MAX_NAME_LEN + 1)).is_err());
    assert_eq!(get(&paths, &created.id).unwrap().name, "Keep");
}

#[test]
fn protected_instances_cannot_be_deleted_until_unprotected() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Guarded")).unwrap();
    set_protected(&paths, &created.id, true).unwrap();

    assert!(delete(&paths, &created.id).is_err());
    assert!(created.directory.exists());

    set_protected(&paths, &created.id, false).unwrap();
    delete(&paths, &created.id).unwrap();
    assert!(!created.directory.exists());
}

#[test]
fn set_pinned_persists() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Pin me")).unwrap();
    assert!(!created.pinned);

    set_pinned(&paths, &created.id, true).unwrap();
    assert!(get(&paths, &created.id).unwrap().pinned);
}

#[test]
fn set_notes_trims_and_rejects_too_long() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Noted")).unwrap();

    set_notes(&paths, &created.id, "  Mods de perf à jour  ").unwrap();
    assert_eq!(get(&paths, &created.id).unwrap().notes, "Mods de perf à jour");

    assert!(set_notes(&paths, &created.id, &"x".repeat(MAX_NOTES_LEN + 1)).is_err());
}

#[test]
fn record_session_prepends_and_caps_history_and_skips_instant_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let created = create(&paths, test_input("Sessions")).unwrap();

    record_session(&paths, &created.id, 100, 0).unwrap();
    assert!(get(&paths, &created.id).unwrap().sessions.is_empty());

    for i in 0..(MAX_SESSIONS + 5) {
        record_session(&paths, &created.id, 1000 + i as i64, 60).unwrap();
    }
    let sessions = get(&paths, &created.id).unwrap().sessions;
    assert_eq!(sessions.len(), MAX_SESSIONS);
    assert_eq!(sessions[0].started_at, 1000 + (MAX_SESSIONS + 4) as i64);
}

#[test]
fn copy_settings_overwrites_target_launch_settings_only() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    let mut source = create(&paths, test_input("Source")).unwrap();
    source.min_memory_mb = Some(1024);
    source.max_memory_mb = Some(4096);
    source.extra_jvm_args = vec!["-XX:+UseZGC".to_string()];
    save(&source).unwrap();
    let target = create(&paths, test_input("Target")).unwrap();

    let updated = copy_settings(&paths, &source.id, &target.id).unwrap();

    assert_eq!(updated.name, "Target");
    assert_eq!(updated.min_memory_mb, Some(1024));
    assert_eq!(updated.max_memory_mb, Some(4096));
    assert_eq!(updated.extra_jvm_args, vec!["-XX:+UseZGC".to_string()]);
}
