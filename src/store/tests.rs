use super::{
    CommentRow, IssueRow, LocalRepoRow, RepoRow, comments_for_issue, delete_db_at,
    get_repo_by_slug, list_issues, list_local_repos, open_db_at, upsert_comment, upsert_issue,
    upsert_local_repo, upsert_repo,
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
