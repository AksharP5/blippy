use super::{
    CommentRow, IssueRow, LocalRepoRow, RepoRow, comments_for_issue, delete_db_at,
    get_repo_by_slug, list_issues, list_local_repos, open_db_at, replace_comments_for_issue,
    replace_local_repos_for_path, upsert_comment, upsert_issue, upsert_local_repo, upsert_repo,
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
fn empty_data_directory_overrides_use_the_platform_fallback() {
    const PROFILE_ENV: &str = "BLIPPY_TEST_DATA_PROFILE";
    if let Some(profile) = std::env::var_os(PROFILE_ENV) {
        let profile = PathBuf::from(profile);
        assert_eq!(super::unix_data_dir(), profile.join(".local/share"));
        assert_eq!(super::windows_data_dir(), profile.join("roaming"));
        return;
    }

    let profile = std::env::temp_dir().join("blippy-test-data-profile");
    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "store::tests::empty_data_directory_overrides_use_the_platform_fallback",
        ])
        .env(PROFILE_ENV, &profile)
        .env("HOME", &profile)
        .env("XDG_DATA_HOME", "")
        .env("LOCALAPPDATA", "")
        .env("APPDATA", profile.join("roaming"))
        .output()
        .expect("isolated data directory test");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
    assert!(!table_exists(&conn, "fts_content"));
    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_db_removes_legacy_search_index() {
    let dir = unique_temp_dir("legacy-search-index");
    let db_path = dir.join("blippy.db");
    let conn = rusqlite::Connection::open(&db_path).expect("open raw db");
    conn.execute_batch(
        "CREATE VIRTUAL TABLE fts_content USING fts5(issue_id, comment_id, title, body, author);",
    )
    .expect("create legacy index");
    drop(conn);

    let conn = open_db_at(&db_path).expect("open db");

    assert!(!table_exists(&conn, "fts_content"));
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
        labels: Vec::new(),
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
        labels: Vec::new(),
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
        labels: Vec::new(),
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
fn replace_comments_for_issue_removes_comments_missing_from_snapshot() {
    let dir = unique_temp_dir("comment-snapshot");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");
    seed_issue(&conn, 20);

    let old_comment = test_comment(300, 20, "old");
    let kept_comment = test_comment(301, 20, "before");
    upsert_comment(&conn, &old_comment).expect("insert old comment");
    upsert_comment(&conn, &kept_comment).expect("insert kept comment");

    let refreshed = CommentRow {
        body: "after".to_string(),
        ..kept_comment
    };
    replace_comments_for_issue(&conn, 20, &[refreshed]).expect("replace comments");

    let comments = comments_for_issue(&conn, 20).expect("list comments");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, 301);
    assert_eq!(comments[0].body, "after");

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn replace_comments_for_issue_rejects_another_issues_comments() {
    let dir = unique_temp_dir("comment-snapshot-mismatch");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");
    seed_issue(&conn, 20);
    let old_comment = test_comment(300, 20, "old");
    upsert_comment(&conn, &old_comment).expect("insert old comment");
    let wrong_comment = test_comment(301, 21, "wrong issue");

    let result = replace_comments_for_issue(&conn, 20, &[wrong_comment]);

    assert!(result.is_err());
    assert_eq!(
        comments_for_issue(&conn, 20).expect("list comments"),
        vec![old_comment]
    );
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
        labels: Vec::new(),
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
        labels: Vec::new(),
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
fn replace_local_repos_for_path_removes_missing_remotes() {
    let dir = unique_temp_dir("local-repo-snapshot");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");
    let origin = test_local_repo("/tmp/repo", "origin");
    let upstream = test_local_repo("/tmp/repo", "upstream");
    upsert_local_repo(&conn, &origin).expect("insert origin");
    upsert_local_repo(&conn, &upstream).expect("insert upstream");

    replace_local_repos_for_path(&conn, "/tmp/repo", &[origin]).expect("replace remotes");

    let repos = list_local_repos(&conn).expect("list repos");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].remote_name, "origin");

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
    assert_eq!(
        get_repo_by_slug(&conn, "AcMe", "BLIPPY").expect("mixed-case lookup"),
        Some(repo)
    );

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn delayed_comment_deletion_keeps_a_refreshed_snapshot_count() {
    let conn = open_db_at(std::path::Path::new(":memory:")).expect("db");
    conn.execute_batch(
        "INSERT INTO repos (id, owner, name) VALUES (1, 'acme', 'blippy');
         INSERT INTO issues (id, repo_id, number, state, title, body, comments_count)
         VALUES (7, 1, 7, 'open', 'Issue', '', 1);
         INSERT INTO comments (id, issue_id, author, body)
         VALUES (51, 7, 'alex', 'Remaining comment');",
    )
    .expect("cache refreshed post-delete snapshot");

    let count =
        super::delete_comment_by_id(&conn, 50, 7).expect("delayed deletion acknowledgement");

    assert_eq!(count, 1);
    assert_eq!(
        list_issues(&conn, 1).expect("cached issue")[0].comments_count,
        1
    );
    assert_eq!(
        comments_for_issue(&conn, 7).expect("remaining comments")[0].id,
        51
    );
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

#[test]
fn legacy_label_migration_preserves_literal_names_and_runs_once() {
    let dir = unique_temp_dir("legacy-labels");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");
    for (id, labels) in [(10, r#"["bug"]"#), (11, "bug,docs"), (12, "")] {
        seed_issue(&conn, id);
        conn.execute("UPDATE issues SET labels = ?1 WHERE id = ?2", (labels, id))
            .expect("write legacy labels");
    }
    super::update_repo_sync_state(&conn, 1, Some("2024-01-01T00:00:00Z"), Some("old-etag"))
        .expect("write old cursor");
    conn.pragma_update(None, "user_version", 0)
        .expect("mark legacy schema");
    drop(conn);

    let conn = open_db_at(&db_path).expect("migrate legacy db");
    let issues = list_issues(&conn, 1).expect("read migrated issues");
    assert_eq!(
        issues
            .iter()
            .find(|issue| issue.id == 10)
            .expect("literal label")
            .labels,
        vec![r#"["bug"]"#]
    );
    assert_eq!(
        issues
            .iter()
            .find(|issue| issue.id == 11)
            .expect("multiple labels")
            .labels,
        vec!["bug", "docs"]
    );
    assert!(
        issues
            .iter()
            .find(|issue| issue.id == 12)
            .expect("empty labels")
            .labels
            .is_empty()
    );
    let repo = get_repo_by_slug(&conn, "acme", "blippy")
        .expect("lookup")
        .expect("repo");
    assert_eq!(repo.updated_at, None);
    assert_eq!(repo.etag, None);

    let refreshed = IssueRow {
        labels: vec!["api, v2".to_string()],
        ..issues
            .iter()
            .find(|issue| issue.id == 10)
            .expect("issue")
            .clone()
    };
    upsert_issue(&conn, &refreshed).expect("cache refreshed label");
    super::update_repo_sync_state(&conn, 1, Some("2024-01-02T00:00:00Z"), Some("new-etag"))
        .expect("write new cursor");
    drop(conn);

    let conn = open_db_at(&db_path).expect("reopen migrated db");
    assert_eq!(
        list_issues(&conn, 1)
            .expect("read labels")
            .into_iter()
            .find(|issue| issue.id == 10)
            .expect("issue")
            .labels,
        vec!["api, v2"]
    );
    let repo = get_repo_by_slug(&conn, "acme", "blippy")
        .expect("lookup")
        .expect("repo");
    assert_eq!(repo.updated_at.as_deref(), Some("2024-01-02T00:00:00Z"));
    assert_eq!(repo.etag.as_deref(), Some("new-etag"));

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

fn seed_issue(conn: &rusqlite::Connection, issue_id: i64) {
    upsert_repo(
        conn,
        &RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: None,
            etag: None,
        },
    )
    .expect("insert repo");
    upsert_issue(
        conn,
        &IssueRow {
            id: issue_id,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: Vec::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    )
    .expect("insert issue");
}

fn test_comment(id: i64, issue_id: i64, body: &str) -> CommentRow {
    CommentRow {
        id,
        issue_id,
        author: "dev".to_string(),
        body: body.to_string(),
        created_at: Some("2024-01-02T01:00:00Z".to_string()),
        last_accessed_at: Some(1),
    }
}

fn test_local_repo(path: &str, remote_name: &str) -> LocalRepoRow {
    LocalRepoRow {
        path: path.to_string(),
        remote_name: remote_name.to_string(),
        owner: "acme".to_string(),
        repo: "blippy".to_string(),
        url: format!("https://github.com/acme/blippy-{}.git", remote_name),
        last_seen: Some("2024-01-05T00:00:00Z".to_string()),
        last_scanned: Some("2024-01-05T00:00:00Z".to_string()),
    }
}

fn table_exists(conn: &rusqlite::Connection, name: &str) -> bool {
    let mut statement = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?1")
        .expect("prepare");
    let mut rows = statement.query([name]).expect("query");
    rows.next().expect("row check").is_some()
}
