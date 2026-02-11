use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use rusqlite::Connection;

const DB_FILE_NAME: &str = "blippy.db";
const APP_DIR_NAME: &str = "blippy";
const DB_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

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
    pub comments_count: i64,
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
    pub last_accessed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRepoRow {
    pub path: String,
    pub remote_name: String,
    pub owner: String,
    pub repo: String,
    pub url: String,
    pub last_seen: Option<String>,
    pub last_scanned: Option<String>,
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
            updated_at = COALESCE(excluded.updated_at, repos.updated_at),
            etag = COALESCE(excluded.etag, repos.etag)
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

pub fn update_repo_sync_state(
    _conn: &Connection,
    _repo_id: i64,
    _updated_at: Option<&str>,
    _etag: Option<&str>,
) -> Result<()> {
    _conn.execute(
        "UPDATE repos SET updated_at = ?1, etag = ?2 WHERE id = ?3",
        (_updated_at, _etag, _repo_id),
    )?;
    Ok(())
}

pub fn upsert_issue(_conn: &Connection, _issue: &IssueRow) -> Result<()> {
    _conn.execute(
        "
        INSERT INTO issues (
            id, repo_id, number, state, title, body, labels, assignees, comments_count, updated_at, is_pr
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            repo_id = excluded.repo_id,
            number = excluded.number,
            state = excluded.state,
            title = excluded.title,
            body = excluded.body,
            labels = excluded.labels,
            assignees = excluded.assignees,
            comments_count = excluded.comments_count,
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
            _issue.comments_count,
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
        INSERT INTO comments (id, issue_id, author, author_type, body, created_at, last_accessed_at)
        VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            issue_id = excluded.issue_id,
            author = excluded.author,
            body = excluded.body,
            created_at = excluded.created_at,
            last_accessed_at = excluded.last_accessed_at
        ",
        (
            _comment.id,
            _comment.issue_id,
            _comment.author.as_str(),
            _comment.body.as_str(),
            _comment.created_at.as_deref(),
            _comment.last_accessed_at,
        ),
    )?;

    index_comment(_conn, _comment)?;
    Ok(())
}

pub fn update_comment_body_by_id(_conn: &Connection, _comment_id: i64, _body: &str) -> Result<()> {
    _conn.execute(
        "UPDATE comments SET body = ?1 WHERE id = ?2",
        (_body, _comment_id),
    )?;
    _conn.execute(
        "UPDATE fts_content SET body = ?1 WHERE comment_id = ?2",
        (_body, _comment_id),
    )?;
    Ok(())
}

pub fn delete_comment_by_id(_conn: &Connection, _comment_id: i64) -> Result<()> {
    _conn.execute("DELETE FROM comments WHERE id = ?1", [_comment_id])?;
    _conn.execute(
        "DELETE FROM fts_content WHERE comment_id = ?1",
        [_comment_id],
    )?;
    Ok(())
}

pub fn list_issues(_conn: &Connection, _repo_id: i64) -> Result<Vec<IssueRow>> {
    let mut statement = _conn.prepare(
        "
        SELECT id, repo_id, number, state, title, body, labels, assignees, comments_count, updated_at, is_pr
        FROM issues
        WHERE repo_id = ?1
        ORDER BY number DESC
        ",
    )?;

    let rows = statement.query_map([_repo_id], |row| {
        let is_pr_value: i64 = row.get(10)?;
        Ok(IssueRow {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            number: row.get(2)?,
            state: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            labels: row.get(6)?,
            assignees: row.get(7)?,
            comments_count: row.get(8)?,
            updated_at: row.get(9)?,
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
        SELECT id, issue_id, author, body, created_at, last_accessed_at
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
            last_accessed_at: row.get(5)?,
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

pub fn upsert_local_repo(_conn: &Connection, _repo: &LocalRepoRow) -> Result<()> {
    _conn.execute(
        "
        INSERT INTO local_repos (
            path, remote_name, owner, repo, url, last_seen, last_scanned
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(path, remote_name) DO UPDATE SET
            owner = excluded.owner,
            repo = excluded.repo,
            url = excluded.url,
            last_seen = excluded.last_seen,
            last_scanned = excluded.last_scanned
        ",
        (
            _repo.path.as_str(),
            _repo.remote_name.as_str(),
            _repo.owner.as_str(),
            _repo.repo.as_str(),
            _repo.url.as_str(),
            _repo.last_seen.as_deref(),
            _repo.last_scanned.as_deref(),
        ),
    )?;
    Ok(())
}

pub fn list_local_repos(_conn: &Connection) -> Result<Vec<LocalRepoRow>> {
    let mut statement = _conn.prepare(
        "
        SELECT path, remote_name, owner, repo, url, last_seen, last_scanned
        FROM local_repos
        ORDER BY last_seen DESC
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(LocalRepoRow {
            path: row.get(0)?,
            remote_name: row.get(1)?,
            owner: row.get(2)?,
            repo: row.get(3)?,
            url: row.get(4)?,
            last_seen: row.get(5)?,
            last_scanned: row.get(6)?,
        })
    })?;

    let mut repos = Vec::new();
    for row in rows {
        repos.push(row?);
    }
    Ok(repos)
}

pub fn get_repo_by_slug(_conn: &Connection, _owner: &str, _repo: &str) -> Result<Option<RepoRow>> {
    let mut statement = _conn.prepare(
        "
        SELECT id, owner, name, updated_at, etag
        FROM repos
        WHERE owner = ?1 AND name = ?2
        LIMIT 1
        ",
    )?;
    let mut rows = statement.query([_owner, _repo])?;
    let row = rows.next()?;
    let row = match row {
        Some(row) => row,
        None => return Ok(None),
    };
    Ok(Some(RepoRow {
        id: row.get(0)?,
        owner: row.get(1)?,
        name: row.get(2)?,
        updated_at: row.get(3)?,
        etag: row.get(4)?,
    }))
}

pub fn update_issue_comments_count(_conn: &Connection, _issue_id: i64, _count: i64) -> Result<()> {
    _conn.execute(
        "UPDATE issues SET comments_count = ?1 WHERE id = ?2",
        (_count, _issue_id),
    )?;
    Ok(())
}

pub fn touch_comments_for_issue(_conn: &Connection, _issue_id: i64, _timestamp: i64) -> Result<()> {
    _conn.execute(
        "UPDATE comments SET last_accessed_at = ?1 WHERE issue_id = ?2",
        (_timestamp, _issue_id),
    )?;
    Ok(())
}

pub fn prune_comments(_conn: &Connection, _ttl_seconds: i64, _max_count: i64) -> Result<()> {
    let cutoff = comment_now_epoch() - _ttl_seconds;
    _conn.execute(
        "DELETE FROM comments WHERE last_accessed_at IS NOT NULL AND last_accessed_at < ?1",
        [cutoff],
    )?;

    let total: i64 = _conn.query_row("SELECT COUNT(*) FROM comments", [], |row| row.get(0))?;
    if total <= _max_count {
        return Ok(());
    }

    let to_delete = total - _max_count;
    _conn.execute(
        "
        DELETE FROM comments
        WHERE id IN (
            SELECT id FROM comments
            ORDER BY last_accessed_at ASC NULLS FIRST
            LIMIT ?1
        )
        ",
        [to_delete],
    )?;
    Ok(())
}

pub fn comment_now_epoch() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now as i64
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
        SELECT id, repo_id, number, state, title, body, labels, assignees, comments_count, updated_at, is_pr
        FROM issues
        WHERE id IN ({})
        ORDER BY number DESC
        ",
        placeholders
    );

    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(ids), |row| {
        let is_pr_value: i64 = row.get(10)?;
        Ok(IssueRow {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            number: row.get(2)?,
            state: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            labels: row.get(6)?,
            assignees: row.get(7)?,
            comments_count: row.get(8)?,
            updated_at: row.get(9)?,
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

pub(crate) fn open_db_at(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;
    configure_connection(&conn)?;
    apply_migrations(&conn)?;
    Ok(conn)
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(DB_BUSY_TIMEOUT)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
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
            comments_count INTEGER NOT NULL DEFAULT 0,
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
            last_accessed_at INTEGER,
            FOREIGN KEY(issue_id) REFERENCES issues(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_content USING fts5(
            issue_id UNINDEXED,
            comment_id UNINDEXED,
            title,
            body,
            author
        );

        CREATE TABLE IF NOT EXISTS local_repos (
            path TEXT NOT NULL,
            remote_name TEXT NOT NULL,
            owner TEXT NOT NULL,
            repo TEXT NOT NULL,
            url TEXT NOT NULL,
            last_seen TEXT,
            last_scanned TEXT,
            PRIMARY KEY (path, remote_name)
        );
        ",
    )?;
    add_comment_accessed_column(_conn)?;
    add_issue_comments_count_column(_conn)?;
    Ok(())
}

fn add_comment_accessed_column(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("PRAGMA table_info(comments)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == "last_accessed_at" {
            return Ok(());
        }
    }

    let result = conn.execute(
        "ALTER TABLE comments ADD COLUMN last_accessed_at INTEGER",
        [],
    );
    if let Err(error) = result {
        let message = error.to_string();
        if message.contains("duplicate column") {
            return Ok(());
        }
        return Err(error.into());
    }
    Ok(())
}

fn add_issue_comments_count_column(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("PRAGMA table_info(issues)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == "comments_count" {
            return Ok(());
        }
    }

    let result = conn.execute(
        "ALTER TABLE issues ADD COLUMN comments_count INTEGER NOT NULL DEFAULT 0",
        [],
    );
    if let Err(error) = result {
        let message = error.to_string();
        if message.contains("duplicate column") {
            return Ok(());
        }
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CommentRow, IssueRow, LocalRepoRow, RepoRow, comments_for_issue, delete_db_at,
        get_repo_by_slug, list_issues, list_local_repos, open_db_at, search_issues, upsert_comment,
        upsert_issue, upsert_local_repo, upsert_repo,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn delete_db_returns_false_when_missing() {
        let dir = unique_temp_dir("missing");
        let db_path = dir.join("blippy.db");
        let deleted = delete_db_at(&db_path).expect("delete succeeds");

        assert!(!deleted);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_db_removes_existing_file() {
        let dir = unique_temp_dir("present");
        let db_path = dir.join("blippy.db");
        fs::write(&db_path, "cache").expect("write db");

        let deleted = delete_db_at(&db_path).expect("delete succeeds");

        assert!(deleted);
        assert!(!db_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_db_creates_file() {
        let dir = unique_temp_dir("create");
        let db_path = dir.join("blippy.db");

        let conn = open_db_at(&db_path).expect("open db");

        assert!(db_path.exists());
        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_db_creates_tables() {
        let dir = unique_temp_dir("tables");
        let db_path = dir.join("blippy.db");

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
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
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
            comments_count: 0,
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
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
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
            comments_count: 0,
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
            last_accessed_at: Some(1),
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
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
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
            comments_count: 0,
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
            last_accessed_at: Some(1),
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
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
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
            comments_count: 0,
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
            last_accessed_at: Some(1),
        };
        let second = CommentRow {
            id: 502,
            issue_id: 50,
            author: "dev".to_string(),
            body: "second".to_string(),
            created_at: Some("2024-01-04T02:00:00Z".to_string()),
            last_accessed_at: Some(1),
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

    #[test]
    fn issues_are_ordered_newest_number_first() {
        let dir = unique_temp_dir("issue-order");
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: None,
            etag: None,
        };
        upsert_repo(&conn, &repo).expect("insert repo");

        let older_number_newer_update = IssueRow {
            id: 60,
            repo_id: 1,
            number: 4,
            state: "open".to_string(),
            title: "older number".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: Some("2025-01-05T00:00:00Z".to_string()),
            is_pr: false,
        };
        let newer_number_older_update = IssueRow {
            id: 61,
            repo_id: 1,
            number: 5,
            state: "open".to_string(),
            title: "newer number".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            is_pr: false,
        };

        upsert_issue(&conn, &older_number_newer_update).expect("insert issue 1");
        upsert_issue(&conn, &newer_number_older_update).expect("insert issue 2");

        let issues = list_issues(&conn, 1).expect("list issues");
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].number, 5);
        assert_eq!(issues[1].number, 4);

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_local_repo_inserts_and_updates() {
        let dir = unique_temp_dir("local-repos");
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = LocalRepoRow {
            path: "/tmp/repo".to_string(),
            remote_name: "origin".to_string(),
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            url: "https://github.com/acme/blippy.git".to_string(),
            last_seen: Some("2024-01-05T00:00:00Z".to_string()),
            last_scanned: Some("2024-01-05T00:00:00Z".to_string()),
        };
        upsert_local_repo(&conn, &repo).expect("insert repo");

        let updated = LocalRepoRow {
            last_seen: Some("2024-01-06T00:00:00Z".to_string()),
            ..repo
        };
        upsert_local_repo(&conn, &updated).expect("update repo");

        let repos = list_local_repos(&conn).expect("list repos");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].last_seen, Some("2024-01-06T00:00:00Z".to_string()));

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_repo_by_slug_returns_repo() {
        let dir = unique_temp_dir("repo-slug");
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = RepoRow {
            id: 99,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: None,
            etag: None,
        };
        upsert_repo(&conn, &repo).expect("insert repo");

        let found = get_repo_by_slug(&conn, "acme", "blippy").expect("lookup");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 99);

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_repo_preserves_existing_sync_state_when_new_values_missing() {
        let dir = unique_temp_dir("repo-sync-state");
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let with_state = RepoRow {
            id: 7,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: Some("2024-01-05T00:00:00Z".to_string()),
            etag: Some("etag-1".to_string()),
        };
        upsert_repo(&conn, &with_state).expect("insert repo with sync state");

        let without_state = RepoRow {
            updated_at: None,
            etag: None,
            ..with_state
        };
        upsert_repo(&conn, &without_state).expect("upsert repo without sync state");

        let repo = get_repo_by_slug(&conn, "acme", "blippy")
            .expect("lookup")
            .expect("repo");
        assert_eq!(repo.etag.as_deref(), Some("etag-1"));
        assert_eq!(repo.updated_at.as_deref(), Some("2024-01-05T00:00:00Z"));

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("blippy-test-{}-{}", label, nanos));
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
