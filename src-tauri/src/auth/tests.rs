use super::*;

#[test]
fn offline_uuid_is_deterministic_and_matches_java_algorithm() {
    assert_eq!(offline_uuid("Player"), "a01e3843e5213998958af459800e4d11");
}

#[test]
fn offline_uuid_differs_per_username_but_is_stable() {
    assert_eq!(offline_uuid("Alice"), offline_uuid("Alice"));
    assert_ne!(offline_uuid("Alice"), offline_uuid("Bob"));
}

#[test]
fn offline_name_falls_back_to_steve_only_without_a_microsoft_account() {
    assert_eq!(offline_name("  Alex ", true).unwrap(), "Alex");
    assert_eq!(offline_name("", false).unwrap(), DEFAULT_OFFLINE_NAME);
    assert_eq!(offline_name("   ", false).unwrap(), DEFAULT_OFFLINE_NAME);
    assert!(offline_name("", true).is_err());
}

#[test]
fn offline_session_rejects_invalid_usernames() {
    for bad in ["", "   ", "ThisNameIsWayTooLong", "with space"] {
        assert!(offline_session(bad).is_err(), "{bad:?}");
    }
}

fn fixture_session(expires_at: i64) -> AccountSession {
    AccountSession {
        profile: MinecraftProfile { id: "uuid".to_string(), name: "Steve".to_string() },
        minecraft_access_token: "token".to_string(),
        expires_at,
        xuid: None,
    }
}

#[test]
fn needs_refresh_follows_the_buffer_window() {
    assert!(!needs_refresh(&fixture_session(10_000), 1_000));
    assert!(needs_refresh(&fixture_session(1_200), 1_000));
    assert!(needs_refresh(&fixture_session(500), 1_000));
    assert!(!needs_refresh(&offline_session("Steve").unwrap(), now_unix()));
}

#[test]
fn offline_session_trims_and_builds_legacy_session() {
    let session = offline_session("  Steve  ").unwrap();
    assert_eq!(session.profile.name, "Steve");
    assert_eq!(session.profile.id.len(), 32);
    assert_eq!(session.minecraft_access_token, "-");
}

#[test]
fn legacy_single_account_file_is_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    std::fs::write(paths.accounts_file(), r#"{"id":"abc","name":"Steve"}"#).unwrap();

    let file = load_accounts(&paths).unwrap();
    assert_eq!(file.active.as_deref(), Some("abc"));
    assert_eq!(file.accounts, vec![AccountMeta { id: "abc".into(), name: "Steve".into() }]);
}

#[test]
fn upsert_adds_or_renames_and_activates() {
    let mut file = AccountsFile::default();
    file.upsert(&MinecraftProfile { id: "a".into(), name: "Old".into() });
    file.upsert(&MinecraftProfile { id: "b".into(), name: "Bob".into() });
    file.upsert(&MinecraftProfile { id: "a".into(), name: "New".into() });
    assert_eq!(file.accounts.len(), 2);
    assert_eq!(file.get("a").unwrap().name, "New");
    assert_eq!(file.active.as_deref(), Some("a"));
}

#[test]
fn cached_session_is_reported_as_offline() {
    let session = cached_session(&AccountMeta { id: "a".into(), name: "Steve".into() });
    assert!(session.view().offline);
    assert!(!fixture_session(now_unix() + 3600).view().offline);
    assert!(fixture_session(0).view().offline, "an expired token is offline-only");
}
