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

pub fn upsert_repo(conn: &Connection, repo: &RepoRow) -> Result<()> {
    conn.execute(
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
            repo.id,
            repo.owner.as_str(),
            repo.name.as_str(),
            repo.updated_at.as_deref(),
            repo.etag.as_deref(),
        ),
    )?;
    Ok(())
}

pub fn update_repo_sync_state(
    conn: &Connection,
    repo_id: i64,
    updated_at: Option<&str>,
    etag: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE repos SET updated_at = ?1, etag = ?2 WHERE id = ?3",
        (updated_at, etag, repo_id),
    )?;
    Ok(())
}

pub fn upsert_issue(conn: &Connection, issue: &IssueRow) -> Result<()> {
    conn.execute(
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
            issue.id,
            issue.repo_id,
            issue.number,
            issue.state.as_str(),
            issue.title.as_str(),
            issue.body.as_str(),
            issue.labels.as_str(),
            issue.assignees.as_str(),
            issue.comments_count,
            issue.updated_at.as_deref(),
            if issue.is_pr { 1 } else { 0 },
        ),
    )?;

    index_issue(conn, issue)?;
    Ok(())
}

pub fn upsert_comment(conn: &Connection, comment: &CommentRow) -> Result<()> {
    conn.execute(
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
            comment.id,
            comment.issue_id,
            comment.author.as_str(),
            comment.body.as_str(),
            comment.created_at.as_deref(),
            comment.last_accessed_at,
        ),
    )?;

    index_comment(conn, comment)?;
    Ok(())
}

pub fn update_comment_body_by_id(conn: &Connection, comment_id: i64, body: &str) -> Result<()> {
    conn.execute(
        "UPDATE comments SET body = ?1 WHERE id = ?2",
        (body, comment_id),
    )?;
    conn.execute(
        "UPDATE fts_content SET body = ?1 WHERE comment_id = ?2",
        (body, comment_id),
    )?;
    Ok(())
}

pub fn delete_comment_by_id(conn: &Connection, comment_id: i64) -> Result<()> {
    conn.execute("DELETE FROM comments WHERE id = ?1", [comment_id])?;
    conn.execute(
        "DELETE FROM fts_content WHERE comment_id = ?1",
        [comment_id],
    )?;
    Ok(())
}

pub fn list_issues(conn: &Connection, repo_id: i64) -> Result<Vec<IssueRow>> {
    let mut statement = conn.prepare(
        "
        SELECT id, repo_id, number, state, title, body, labels, assignees, comments_count, updated_at, is_pr
        FROM issues
        WHERE repo_id = ?1
        ORDER BY number DESC
        ",
    )?;

    let rows = statement.query_map([repo_id], |row| {
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

pub fn comments_for_issue(conn: &Connection, issue_id: i64) -> Result<Vec<CommentRow>> {
    let mut statement = conn.prepare(
        "
        SELECT id, issue_id, author, body, created_at, last_accessed_at
        FROM comments
        WHERE issue_id = ?1
        ORDER BY created_at ASC
        ",
    )?;

    let rows = statement.query_map([issue_id], |row| {
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

pub fn upsert_local_repo(conn: &Connection, repo: &LocalRepoRow) -> Result<()> {
    conn.execute(
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
            repo.path.as_str(),
            repo.remote_name.as_str(),
            repo.owner.as_str(),
            repo.repo.as_str(),
            repo.url.as_str(),
            repo.last_seen.as_deref(),
            repo.last_scanned.as_deref(),
        ),
    )?;
    Ok(())
}

pub fn list_local_repos(conn: &Connection) -> Result<Vec<LocalRepoRow>> {
    let mut statement = conn.prepare(
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

pub fn get_repo_by_slug(conn: &Connection, owner: &str, repo: &str) -> Result<Option<RepoRow>> {
    let mut statement = conn.prepare(
        "
        SELECT id, owner, name, updated_at, etag
        FROM repos
        WHERE owner = ?1 AND name = ?2
        LIMIT 1
        ",
    )?;
    let mut rows = statement.query([owner, repo])?;
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

pub fn update_issue_comments_count(conn: &Connection, issue_id: i64, count: i64) -> Result<()> {
    conn.execute(
        "UPDATE issues SET comments_count = ?1 WHERE id = ?2",
        (count, issue_id),
    )?;
    Ok(())
}

pub fn touch_comments_for_issue(conn: &Connection, issue_id: i64, timestamp: i64) -> Result<()> {
    conn.execute(
        "UPDATE comments SET last_accessed_at = ?1 WHERE issue_id = ?2",
        (timestamp, issue_id),
    )?;
    Ok(())
}

pub fn prune_comments(conn: &Connection, ttl_seconds: i64, max_count: i64) -> Result<()> {
    let cutoff = comment_now_epoch() - ttl_seconds;
    conn.execute(
        "DELETE FROM comments WHERE last_accessed_at IS NOT NULL AND last_accessed_at < ?1",
        [cutoff],
    )?;

    let total: i64 = conn.query_row("SELECT COUNT(*) FROM comments", [], |row| row.get(0))?;
    if total <= max_count {
        return Ok(());
    }

    let to_delete = total - max_count;
    conn.execute(
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

fn apply_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
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
    add_comment_accessed_column(conn)?;
    add_issue_comments_count_column(conn)?;
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
mod tests;
