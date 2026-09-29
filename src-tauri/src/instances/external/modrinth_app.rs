//! The Modrinth App keeps its instances in a SQLite database (`app.db` next
//! to the `profiles/` folder): a `profiles` table in older versions, then
//! `instances` + `instance_content_sets` (version and loader moved there).
//! The database is copied (with its WAL) and the copy read, so a running
//! Modrinth App is never locked or disturbed.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

#[derive(Debug, Clone, PartialEq)]
pub struct AppProfile {
    /// Folder name inside `profiles/`.
    pub path: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    /// Unix seconds.
    pub last_played: Option<i64>,
}

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |_| Ok(()))
        .is_ok()
}

fn query(conn: &Connection) -> rusqlite::Result<Vec<AppProfile>> {
    let sql = if has_table(conn, "instances") && has_table(conn, "instance_content_sets") {
        // The applied content set, else the most recently modified one.
        "SELECT i.path, i.name, s.game_version, s.loader, s.loader_version, i.last_played
         FROM instances i
         JOIN instance_content_sets s ON s.instance_id = i.id
         ORDER BY (s.id = i.applied_content_set_id) DESC, s.modified DESC"
    } else if has_table(conn, "profiles") {
        "SELECT path, name, game_version, mod_loader, mod_loader_version, last_played FROM profiles"
    } else {
        return Ok(Vec::new());
    };
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([], |row| {
        Ok(AppProfile {
            path: row.get(0)?,
            name: row.get(1)?,
            game_version: row.get(2)?,
            loader: row.get(3)?,
            loader_version: row.get(4)?,
            last_played: row.get(5)?,
        })
    })?;
    let mut seen = std::collections::HashSet::new();
    Ok(rows.filter_map(Result::ok).filter(|p| seen.insert(p.path.clone())).collect())
}

/// Every instance recorded in the Modrinth App database at `db`.
pub fn read_profiles(db: &Path) -> Vec<AppProfile> {
    if !db.is_file() {
        return Vec::new();
    }
    let copy = std::env::temp_dir().join(format!("largy-modrinth-app-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        std::fs::create_dir_all(&copy).ok()?;
        let target = copy.join("app.db");
        std::fs::copy(db, &target).ok()?;
        for suffix in ["-wal", "-shm"] {
            let side = db.with_file_name(format!("app.db{suffix}"));
            if side.is_file() {
                let _ = std::fs::copy(&side, copy.join(format!("app.db{suffix}")));
            }
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        match Connection::open_with_flags(&target, flags).and_then(|conn| query(&conn)) {
            Ok(profiles) => Some(profiles),
            Err(e) => {
                tracing::warn!("Modrinth App database unreadable: {e}");
                None
            }
        }
    })();
    let _ = std::fs::remove_dir_all(&copy);
    result.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(dir: &Path, schema: &str) -> std::path::PathBuf {
        let path = dir.join("app.db");
        Connection::open(&path).unwrap().execute_batch(schema).unwrap();
        path
    }

    #[test]
    fn reads_the_old_profiles_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = db(
            dir.path(),
            "CREATE TABLE profiles (path TEXT, name TEXT, game_version TEXT, mod_loader TEXT,
                                    mod_loader_version TEXT, last_played INTEGER);
             INSERT INTO profiles VALUES ('fo', 'Fabulously Optimized', '1.21.1', 'fabric', '0.16.5', 1700000000);",
        );
        assert_eq!(
            read_profiles(&path),
            vec![AppProfile {
                path: "fo".into(),
                name: "Fabulously Optimized".into(),
                game_version: "1.21.1".into(),
                loader: "fabric".into(),
                loader_version: Some("0.16.5".into()),
                last_played: Some(1_700_000_000),
            }]
        );
    }

    #[test]
    fn reads_instances_with_their_applied_content_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = db(
            dir.path(),
            "CREATE TABLE instances (id TEXT, path TEXT, applied_content_set_id TEXT, name TEXT, last_played INTEGER);
             CREATE TABLE instance_content_sets (id TEXT, instance_id TEXT, game_version TEXT, loader TEXT,
                                                 loader_version TEXT, modified INTEGER);
             INSERT INTO instances VALUES ('i1', 'create', 's1', 'Create', NULL);
             INSERT INTO instance_content_sets VALUES ('s1', 'i1', '1.20.1', 'forge', '47.2.0', 1);
             INSERT INTO instance_content_sets VALUES ('s2', 'i1', '1.21.1', 'neoforge', '21.1.77', 9);",
        );
        let profiles = read_profiles(&path);
        assert_eq!(profiles.len(), 1);
        assert_eq!((profiles[0].game_version.as_str(), profiles[0].loader.as_str()), ("1.20.1", "forge"));
    }

    #[test]
    fn missing_or_unknown_databases_give_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_profiles(&dir.path().join("app.db")).is_empty());
        let other = db(dir.path(), "CREATE TABLE settings (x INTEGER);");
        assert!(read_profiles(&other).is_empty());
    }
}
