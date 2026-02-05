use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;

const DB_FILE_NAME: &str = "glyph.db";
const APP_DIR_NAME: &str = "glyph";

pub fn db_path() -> PathBuf {
    data_dir().join(APP_DIR_NAME).join(DB_FILE_NAME)
}

pub fn delete_db() -> Result<bool> {
    delete_db_at(&db_path())
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

#[cfg(test)]
mod tests {
    use super::delete_db_at;
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

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("glyph-test-{}-{}", label, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
