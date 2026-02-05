use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

const DB_FILE_NAME: &str = "glyph.db";
const APP_DIR_NAME: &str = "glyph";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRow {
    pub id: i64,
    pub owner: String,
    pub name: String,
    pub updated_at: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRow {
    pub id: i64,
    pub repo_id: i64,
    pub number: i64,
    pub state: String,
    pub title: String,
    pub body: String,
    pub labels: String,
    pub assignees: String,
    pub updated_at: Option<String>,
    pub is_pr: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRow {
    pub id: i64,
    pub issue_id: i64,
    pub author: String,
    pub body: String,
    pub created_at: Option<String>,
}

pub fn db_path() -> PathBuf {
    data_dir().join(APP_DIR_NAME).join(DB_FILE_NAME)
}

pub fn delete_db() -> Result<bool> {
    delete_db_at(&db_path())
}

pub fn open_db() -> Result<Connection> {
    open_db_at(&db_path())
}

pub fn upsert_repo(_conn: &Connection, _repo: &RepoRow) -> Result<()> {
    _conn.execute(
        "
        INSERT INTO repos (id, owner, name, updated_at, etag)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(id) DO UPDATE SET
            owner = excluded.owner,
            name = excluded.name,
            updated_at = excluded.updated_at,
            etag = excluded.etag
        ",
        (
            _repo.id,
            _repo.owner.as_str(),
            _repo.name.as_str(),
            _repo.updated_at.as_deref(),
            _repo.etag.as_deref(),
        ),
    )?;
    Ok(())
}

pub fn upsert_issue(_conn: &Connection, _issue: &IssueRow) -> Result<()> {
    _conn.execute(
        "
        INSERT INTO issues (
            id, repo_id, number, state, title, body, labels, assignees, updated_at, is_pr
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            repo_id = excluded.repo_id,
            number = excluded.number,
            state = excluded.state,
            title = excluded.title,
            body = excluded.body,
            labels = excluded.labels,
            assignees = excluded.assignees,
            updated_at = excluded.updated_at,
            is_pr = excluded.is_pr
        ",
        (
            _issue.id,
            _issue.repo_id,
            _issue.number,
            _issue.state.as_str(),
            _issue.title.as_str(),
            _issue.body.as_str(),
            _issue.labels.as_str(),
            _issue.assignees.as_str(),
            _issue.updated_at.as_deref(),
            if _issue.is_pr { 1 } else { 0 },
        ),
    )?;

    index_issue(_conn, _issue)?;
    Ok(())
}

pub fn upsert_comment(_conn: &Connection, _comment: &CommentRow) -> Result<()> {
    _conn.execute(
        "
        INSERT INTO comments (id, issue_id, author, author_type, body, created_at)
        VALUES (?1, ?2, ?3, NULL, ?4, ?5)
        ON CONFLICT(id) DO UPDATE SET
            issue_id = excluded.issue_id,
            author = excluded.author,
            body = excluded.body,
            created_at = excluded.created_at
        ",
        (
            _comment.id,
            _comment.issue_id,
            _comment.author.as_str(),
            _comment.body.as_str(),
            _comment.created_at.as_deref(),
        ),
    )?;

    index_comment(_conn, _comment)?;
    Ok(())
}

pub fn list_issues(_conn: &Connection, _repo_id: i64) -> Result<Vec<IssueRow>> {
    let mut statement = _conn.prepare(
        "
        SELECT id, repo_id, number, state, title, body, labels, assignees, updated_at, is_pr
        FROM issues
        WHERE repo_id = ?1
        ORDER BY updated_at DESC
        ",
    )?;

    let rows = statement.query_map([_repo_id], |row| {
        let is_pr_value: i64 = row.get(9)?;
        Ok(IssueRow {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            number: row.get(2)?,
            state: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            labels: row.get(6)?,
            assignees: row.get(7)?,
            updated_at: row.get(8)?,
            is_pr: is_pr_value != 0,
        })
    })?;

    let mut issues = Vec::new();
    for row in rows {
        issues.push(row?);
    }
    Ok(issues)
}

pub fn comments_for_issue(_conn: &Connection, _issue_id: i64) -> Result<Vec<CommentRow>> {
    let mut statement = _conn.prepare(
        "
        SELECT id, issue_id, author, body, created_at
        FROM comments
        WHERE issue_id = ?1
        ORDER BY created_at ASC
        ",
    )?;

    let rows = statement.query_map([_issue_id], |row| {
        Ok(CommentRow {
            id: row.get(0)?,
            issue_id: row.get(1)?,
            author: row.get(2)?,
            body: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;

    let mut comments = Vec::new();
    for row in rows {
        comments.push(row?);
    }
    Ok(comments)
}

pub fn search_issues(_conn: &Connection, _query: &str) -> Result<Vec<IssueRow>> {
    let mut statement = _conn.prepare(
        "
        SELECT issue_id
        FROM fts_content
        WHERE fts_content MATCH ?1
        ",
    )?;
    let rows = statement.query_map([_query], |row| row.get::<_, i64>(0))?;

    let mut issue_ids = Vec::new();
    for row in rows {
        issue_ids.push(row?);
    }

    if issue_ids.is_empty() {
        return Ok(Vec::new());
    }

    issue_ids.sort_unstable();
    issue_ids.dedup();

    let issues = fetch_issues_by_ids(_conn, &issue_ids)?;
    Ok(issues)
}

fn index_issue(conn: &Connection, issue: &IssueRow) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_content WHERE issue_id = ?1 AND comment_id IS NULL",
        [issue.id],
    )?;
    conn.execute(
        "
        INSERT INTO fts_content (issue_id, comment_id, title, body, author)
        VALUES (?1, NULL, ?2, ?3, NULL)
        ",
        (issue.id, issue.title.as_str(), issue.body.as_str()),
    )?;
    Ok(())
}

fn index_comment(conn: &Connection, comment: &CommentRow) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_content WHERE comment_id = ?1",
        [comment.id],
    )?;
    conn.execute(
        "
        INSERT INTO fts_content (issue_id, comment_id, title, body, author)
        VALUES (?1, ?2, NULL, ?3, ?4)
        ",
        (
            comment.issue_id,
            comment.id,
            comment.body.as_str(),
            comment.author.as_str(),
        ),
    )?;
    Ok(())
}

fn fetch_issues_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<IssueRow>> {
    let placeholders = ids
        .iter()
        .enumerate()
        .map(|(idx, _)| format!("?{}", idx + 1))
        .collect::<Vec<String>>()
        .join(",");
    let sql = format!(
        "
        SELECT id, repo_id, number, state, title, body, labels, assignees, updated_at, is_pr
        FROM issues
        WHERE id IN ({})
        ORDER BY updated_at DESC
        ",
        placeholders
    );

    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(ids), |row| {
        let is_pr_value: i64 = row.get(9)?;
        Ok(IssueRow {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            number: row.get(2)?,
            state: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            labels: row.get(6)?,
            assignees: row.get(7)?,
            updated_at: row.get(8)?,
            is_pr: is_pr_value != 0,
        })
    })?;

    let mut issues = Vec::new();
    for row in rows {
        issues.push(row?);
    }
    Ok(issues)
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
    use super::{
        comments_for_issue, delete_db_at, list_issues, open_db_at, search_issues, upsert_comment,
        upsert_issue, upsert_repo, CommentRow, IssueRow, RepoRow,
    };
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

    #[test]
    fn upsert_issue_inserts_and_updates() {
        let dir = unique_temp_dir("issue-upsert");
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: None,
            etag: None,
        };
        upsert_repo(&conn, &repo).expect("insert repo");

        let issue = IssueRow {
            id: 10,
            repo_id: 1,
            number: 42,
            state: "open".to_string(),
            title: "Initial".to_string(),
            body: "Body".to_string(),
            labels: "".to_string(),
            assignees: "".to_string(),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            is_pr: false,
        };
        upsert_issue(&conn, &issue).expect("insert issue");

        let updated = IssueRow {
            title: "Updated".to_string(),
            body: "New body".to_string(),
            ..issue
        };
        upsert_issue(&conn, &updated).expect("update issue");

        let issues = list_issues(&conn, 1).expect("list issues");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].title, "Updated");
        assert_eq!(issues[0].body, "New body");

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_comment_inserts_and_updates() {
        let dir = unique_temp_dir("comment-upsert");
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: None,
            etag: None,
        };
        upsert_repo(&conn, &repo).expect("insert repo");

        let issue = IssueRow {
            id: 20,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: "Body".to_string(),
            labels: "".to_string(),
            assignees: "".to_string(),
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
            is_pr: false,
        };
        upsert_issue(&conn, &issue).expect("insert issue");

        let comment = CommentRow {
            id: 300,
            issue_id: 20,
            author: "dev".to_string(),
            body: "First".to_string(),
            created_at: Some("2024-01-02T01:00:00Z".to_string()),
        };
        upsert_comment(&conn, &comment).expect("insert comment");

        let updated = CommentRow {
            body: "Updated comment".to_string(),
            ..comment
        };
        upsert_comment(&conn, &updated).expect("update comment");

        let comments = comments_for_issue(&conn, 20).expect("list comments");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Updated comment");

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_issues_returns_issue_when_comment_matches() {
        let dir = unique_temp_dir("fts-search");
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: None,
            etag: None,
        };
        upsert_repo(&conn, &repo).expect("insert repo");

        let issue = IssueRow {
            id: 40,
            repo_id: 1,
            number: 2,
            state: "open".to_string(),
            title: "Search me".to_string(),
            body: "Issue body".to_string(),
            labels: "".to_string(),
            assignees: "".to_string(),
            updated_at: Some("2024-01-03T00:00:00Z".to_string()),
            is_pr: false,
        };
        upsert_issue(&conn, &issue).expect("insert issue");

        let comment = CommentRow {
            id: 400,
            issue_id: 40,
            author: "dev".to_string(),
            body: "needle".to_string(),
            created_at: Some("2024-01-03T01:00:00Z".to_string()),
        };
        upsert_comment(&conn, &comment).expect("insert comment");

        let results = search_issues(&conn, "needle").expect("search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 40);

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn comments_are_ordered_oldest_first() {
        let dir = unique_temp_dir("comment-order");
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: None,
            etag: None,
        };
        upsert_repo(&conn, &repo).expect("insert repo");

        let issue = IssueRow {
            id: 50,
            repo_id: 1,
            number: 3,
            state: "open".to_string(),
            title: "Order".to_string(),
            body: "Body".to_string(),
            labels: "".to_string(),
            assignees: "".to_string(),
            updated_at: Some("2024-01-04T00:00:00Z".to_string()),
            is_pr: false,
        };
        upsert_issue(&conn, &issue).expect("insert issue");

        let first = CommentRow {
            id: 501,
            issue_id: 50,
            author: "dev".to_string(),
            body: "first".to_string(),
            created_at: Some("2024-01-04T01:00:00Z".to_string()),
        };
        let second = CommentRow {
            id: 502,
            issue_id: 50,
            author: "dev".to_string(),
            body: "second".to_string(),
            created_at: Some("2024-01-04T02:00:00Z".to_string()),
        };
        upsert_comment(&conn, &second).expect("insert comment 2");
        upsert_comment(&conn, &first).expect("insert comment 1");

        let comments = comments_for_issue(&conn, 50).expect("list comments");
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].body, "first");
        assert_eq!(comments[1].body, "second");

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
