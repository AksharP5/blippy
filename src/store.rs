use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

const DB_FILE_NAME: &str = "glyph.db";
const APP_DIR_NAME: &str = "glyph";

pub fn db_path() -> PathBuf {
    data_dir().join(APP_DIR_NAME).join(DB_FILE_NAME)
}

pub fn delete_db() -> Result<bool> {
    delete_db_at(&db_path())
}

pub fn open_db() -> Result<Connection> {
    open_db_at(&db_path())
}

fn data_dir() -> PathBuf {
    if cfg!(windows) {
        return windows_data_dir();
    }

    unix_data_dir()
}

fn unix_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("XDG_DATA_HOME") {
        return PathBuf::from(dir);
    }

    if let Ok(home) = env::var("HOME") {
        return Path::new(&home).join(".local").join("share");
    }

    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn windows_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("LOCALAPPDATA") {
        return PathBuf::from(dir);
    }

    if let Ok(dir) = env::var("APPDATA") {
        return PathBuf::from(dir);
    }

    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn delete_db_at(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    std::fs::remove_file(path)?;
    Ok(true)
}

fn open_db_at(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;
    apply_migrations(&conn)?;
    Ok(conn)
}

fn apply_migrations(_conn: &Connection) -> Result<()> {
    _conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS repos (
            id INTEGER PRIMARY KEY,
            owner TEXT NOT NULL,
            name TEXT NOT NULL,
            updated_at TEXT,
            etag TEXT,
            UNIQUE(owner, name)
        );

        CREATE TABLE IF NOT EXISTS issues (
            id INTEGER PRIMARY KEY,
            repo_id INTEGER NOT NULL,
            number INTEGER NOT NULL,
            state TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            labels TEXT NOT NULL DEFAULT '',
            assignees TEXT NOT NULL DEFAULT '',
            updated_at TEXT,
            is_pr INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS comments (
            id INTEGER PRIMARY KEY,
            issue_id INTEGER NOT NULL,
            author TEXT NOT NULL,
            author_type TEXT,
            body TEXT NOT NULL,
            created_at TEXT,
            FOREIGN KEY(issue_id) REFERENCES issues(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_content USING fts5(
            issue_id UNINDEXED,
            comment_id UNINDEXED,
            title,
            body,
            author
        );
        ",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{delete_db_at, open_db_at};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn delete_db_returns_false_when_missing() {
        let dir = unique_temp_dir("missing");
        let db_path = dir.join("glyph.db");
        let deleted = delete_db_at(&db_path).expect("delete succeeds");

        assert!(!deleted);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_db_removes_existing_file() {
        let dir = unique_temp_dir("present");
        let db_path = dir.join("glyph.db");
        fs::write(&db_path, "cache").expect("write db");

        let deleted = delete_db_at(&db_path).expect("delete succeeds");

        assert!(deleted);
        assert!(!db_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_db_creates_file() {
        let dir = unique_temp_dir("create");
        let db_path = dir.join("glyph.db");

        let conn = open_db_at(&db_path).expect("open db");

        assert!(db_path.exists());
        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_db_creates_tables() {
        let dir = unique_temp_dir("tables");
        let db_path = dir.join("glyph.db");

        let conn = open_db_at(&db_path).expect("open db");

        assert!(table_exists(&conn, "repos"));
        assert!(table_exists(&conn, "issues"));
        assert!(table_exists(&conn, "comments"));
        assert!(table_exists(&conn, "fts_content"));
        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("glyph-test-{}-{}", label, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn table_exists(conn: &rusqlite::Connection, name: &str) -> bool {
        let mut statement = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?1")
            .expect("prepare");
        let mut rows = statement.query([name]).expect("query");
        rows.next().expect("row check").is_some()
    }
}
