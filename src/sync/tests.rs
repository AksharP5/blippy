use super::{
    GitHubApi, SyncStats, map_comment_to_row, map_issue_to_row, map_repo_to_row,
    sync_repo_with_progress,
};
use crate::github::{ApiComment, ApiIssue, ApiIssuesPageResult, ApiLabel, ApiRepo, ApiUser};
use crate::store::{comments_for_issue, get_repo_by_slug, list_issues, open_db_at};
use anyhow::Result;
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

async fn sync_repo(
    client: &dyn GitHubApi,
    conn: &rusqlite::Connection,
    owner: &str,
    repo: &str,
) -> Result<SyncStats> {
    sync_repo_with_progress(client, conn, owner, repo, |_page, _stats| {}).await
}

#[test]
fn map_repo_to_row_copies_owner_and_name() {
    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let row = map_repo_to_row(&repo);
    assert_eq!(row.id, 1);
    assert_eq!(row.owner, "acme");
    assert_eq!(row.name, "blippy");
}

#[test]
fn map_issue_to_row_marks_pull_requests() {
    let issue = ApiIssue {
        id: 10,
        number: 1,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: Some("body".to_string()),
        comments: 0,
        updated_at: None,
        labels: Vec::new(),
        assignees: Vec::new(),
        user: ApiUser {
            login: "dev".to_string(),
            user_type: None,
        },
        pull_request: Some(serde_json::json!({"url": "x"})),
    };
    let row = map_issue_to_row(1, &issue);
    assert!(row.is_some());
    assert!(row.is_some_and(|row| row.is_pr));
}

#[test]
fn map_issue_to_row_marks_merged_pull_requests() {
    let issue = ApiIssue {
        id: 12,
        number: 3,
        state: "closed".to_string(),
        title: "Merged PR".to_string(),
        body: Some("body".to_string()),
        comments: 0,
        updated_at: None,
        labels: Vec::new(),
        assignees: Vec::new(),
        user: ApiUser {
            login: "dev".to_string(),
            user_type: None,
        },
        pull_request: Some(serde_json::json!({
            "url": "x",
            "merged_at": "2024-02-01T12:00:00Z"
        })),
    };

    let row = map_issue_to_row(1, &issue).expect("row");
    assert!(row.is_pr);
    assert_eq!(row.state, "merged");
}

#[test]
fn map_issue_to_row_builds_label_and_assignee_strings() {
    let issue = ApiIssue {
        id: 11,
        number: 2,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: Some("body".to_string()),
        comments: 3,
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        labels: vec![ApiLabel {
            name: "bug".to_string(),
            color: "ff0000".to_string(),
        }],
        assignees: vec![ApiUser {
            login: "dev".to_string(),
            user_type: None,
        }],
        user: ApiUser {
            login: "dev".to_string(),
            user_type: None,
        },
        pull_request: None,
    };
    let row = map_issue_to_row(1, &issue).expect("row");
    assert_eq!(row.labels, "bug");
    assert_eq!(row.assignees, "dev");
    assert_eq!(row.comments_count, 3);
}

#[test]
fn map_comment_to_row_copies_author() {
    let comment = ApiComment {
        id: 50,
        body: Some("hello".to_string()),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        user: ApiUser {
            login: "dev".to_string(),
            user_type: None,
        },
    };
    let row = map_comment_to_row(99, &comment);
    assert_eq!(row.issue_id, 99);
    assert_eq!(row.author, "dev");
    assert_eq!(row.body, "hello");
}

#[tokio::test]
async fn sync_repo_inserts_issues_and_comments() {
    let dir = unique_temp_dir("sync");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let issues = vec![
        ApiIssue {
            id: 10,
            number: 1,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: Some("body".to_string()),
            comments: 1,
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
        ApiIssue {
            id: 11,
            number: 2,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: None,
            comments: 0,
            updated_at: None,
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: Some(serde_json::json!({"url": "x"})),
        },
    ];
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: Some("etag-sync".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 2);
    assert_eq!(stats.comments, 0);

    let rows = list_issues(&conn, 1).expect("list issues");
    assert_eq!(rows.len(), 2);
    let comments = comments_for_issue(&conn, 10).expect("comments");
    assert_eq!(comments.len(), 0);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

struct FakeGitHub {
    repo: ApiRepo,
    issues: Vec<ApiIssue>,
    fail_get_repo: bool,
    fail_issue_page: Option<u32>,
    issue_page_size: usize,
    page_etag: Option<String>,
    not_modified_when_etag_matches: bool,
}

#[async_trait]
impl GitHubApi for FakeGitHub {
    async fn get_repo(&self, _owner: &str, _repo: &str) -> anyhow::Result<ApiRepo> {
        if self.fail_get_repo {
            return Err(anyhow::anyhow!("get repo failed"));
        }
        Ok(ApiRepo {
            id: self.repo.id,
            name: self.repo.name.clone(),
            owner: ApiUser {
                login: self.repo.owner.login.clone(),
                user_type: None,
            },
            permissions: None,
        })
    }

    async fn list_issues_page(
        &self,
        _owner: &str,
        _repo: &str,
        page: u32,
        if_none_match: Option<&str>,
        _since: Option<&str>,
    ) -> anyhow::Result<ApiIssuesPageResult> {
        if page == 1
            && self.not_modified_when_etag_matches
            && self
                .page_etag
                .as_deref()
                .is_some_and(|etag| Some(etag) == if_none_match)
        {
            return Ok(ApiIssuesPageResult::NotModified);
        }

        if self
            .fail_issue_page
            .is_some_and(|fail_page| fail_page == page)
        {
            return Err(anyhow::anyhow!("rate limit"));
        }

        let page_index = page.saturating_sub(1) as usize;
        let start = page_index.saturating_mul(self.issue_page_size);
        if start >= self.issues.len() {
            return Ok(ApiIssuesPageResult::Page(crate::github::ApiIssuesPage {
                issues: Vec::new(),
                etag: self.page_etag.clone(),
            }));
        }
        let end = (start + self.issue_page_size).min(self.issues.len());
        Ok(ApiIssuesPageResult::Page(crate::github::ApiIssuesPage {
            issues: self.issues[start..end].to_vec(),
            etag: self.page_etag.clone(),
        }))
    }
}

#[tokio::test]
async fn sync_repo_persists_partial_when_later_page_fails() {
    let dir = unique_temp_dir("sync-partial");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let issues = vec![
        ApiIssue {
            id: 10,
            number: 1,
            state: "open".to_string(),
            title: "Issue 1".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
        ApiIssue {
            id: 11,
            number: 2,
            state: "open".to_string(),
            title: "Issue 2".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
        ApiIssue {
            id: 12,
            number: 3,
            state: "open".to_string(),
            title: "Issue 3".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-03T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
    ];
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: Some(3),
        issue_page_size: 1,
        page_etag: Some("etag-partial".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 2);

    let rows = list_issues(&conn, 1).expect("list issues");
    assert_eq!(rows.len(), 2);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_reports_progress_per_page() {
    let dir = unique_temp_dir("sync-progress");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let issues = vec![
        ApiIssue {
            id: 10,
            number: 1,
            state: "open".to_string(),
            title: "Issue 1".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
        ApiIssue {
            id: 11,
            number: 2,
            state: "open".to_string(),
            title: "Issue 2".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
    ];
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 1,
        page_etag: Some("etag-progress".to_string()),
        not_modified_when_etag_matches: false,
    };

    let mut progress = Vec::new();
    let stats = sync_repo_with_progress(&client, &conn, "acme", "blippy", |page, stats| {
        progress.push((page, stats.issues));
    })
    .await
    .expect("sync");

    assert_eq!(stats.issues, 2);
    assert_eq!(progress, vec![(1, 1), (2, 2)]);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_updates_repo_sync_cursor_after_success() {
    let dir = unique_temp_dir("sync-cursor");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let issues = vec![
        ApiIssue {
            id: 10,
            number: 1,
            state: "open".to_string(),
            title: "Issue 1".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
        ApiIssue {
            id: 11,
            number: 2,
            state: "open".to_string(),
            title: "Issue 2".to_string(),
            body: Some("body".to_string()),
            comments: 0,
            updated_at: Some("2024-01-03T00:00:00Z".to_string()),
            labels: Vec::new(),
            assignees: Vec::new(),
            user: ApiUser {
                login: "dev".to_string(),
                user_type: None,
            },
            pull_request: None,
        },
    ];
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: Some("etag-cursor".to_string()),
        not_modified_when_etag_matches: false,
    };

    sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");

    let stored_repo = get_repo_by_slug(&conn, "acme", "blippy")
        .expect("lookup")
        .expect("repo");
    assert_eq!(
        stored_repo.updated_at.as_deref(),
        Some("2024-01-03T00:00:00Z")
    );
    assert_eq!(stored_repo.etag.as_deref(), Some("etag-cursor"));

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_skips_fetch_when_etag_not_modified() {
    let dir = unique_temp_dir("sync-not-modified");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let existing = crate::store::RepoRow {
        id: 1,
        owner: "acme".to_string(),
        name: "blippy".to_string(),
        updated_at: Some("2024-01-05T00:00:00Z".to_string()),
        etag: Some("etag-stable".to_string()),
    };
    crate::store::upsert_repo(&conn, &existing).expect("seed repo state");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let client = FakeGitHub {
        repo,
        issues: Vec::new(),
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: Some("etag-stable".to_string()),
        not_modified_when_etag_matches: true,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert!(stats.not_modified);
    assert_eq!(stats.issues, 0);

    let stored_repo = get_repo_by_slug(&conn, "acme", "blippy")
        .expect("lookup")
        .expect("repo");
    assert_eq!(
        stored_repo.updated_at.as_deref(),
        Some("2024-01-05T00:00:00Z")
    );
    assert_eq!(stored_repo.etag.as_deref(), Some("etag-stable"));

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_uses_cached_repo_without_get_repo_call() {
    let dir = unique_temp_dir("sync-cached-repo");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let existing = crate::store::RepoRow {
        id: 1,
        owner: "acme".to_string(),
        name: "blippy".to_string(),
        updated_at: Some("2024-01-05T00:00:00Z".to_string()),
        etag: Some("etag-stable".to_string()),
    };
    crate::store::upsert_repo(&conn, &existing).expect("seed repo state");

    let client = FakeGitHub {
        repo: ApiRepo {
            id: 1,
            name: "blippy".to_string(),
            owner: ApiUser {
                login: "acme".to_string(),
                user_type: None,
            },
            permissions: None,
        },
        issues: Vec::new(),
        fail_get_repo: true,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: Some("etag-stable".to_string()),
        not_modified_when_etag_matches: true,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert!(stats.not_modified);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_does_not_advance_cursor_on_partial_failure() {
    let dir = unique_temp_dir("sync-cursor-partial");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let existing = crate::store::RepoRow {
        id: 1,
        owner: "acme".to_string(),
        name: "blippy".to_string(),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        etag: Some("etag-old".to_string()),
    };
    crate::store::upsert_repo(&conn, &existing).expect("seed repo state");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let issues = vec![ApiIssue {
        id: 10,
        number: 1,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: Some("body".to_string()),
        comments: 0,
        updated_at: Some("2024-01-03T00:00:00Z".to_string()),
        labels: Vec::new(),
        assignees: Vec::new(),
        user: ApiUser {
            login: "dev".to_string(),
            user_type: None,
        },
        pull_request: None,
    }];
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: Some(2),
        issue_page_size: 1,
        page_etag: Some("etag-new".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 1);
    assert!(!stats.not_modified);

    let stored_repo = get_repo_by_slug(&conn, "acme", "blippy")
        .expect("lookup")
        .expect("repo");
    assert_eq!(
        stored_repo.updated_at.as_deref(),
        Some("2024-01-01T00:00:00Z")
    );
    assert_eq!(stored_repo.etag.as_deref(), Some("etag-old"));

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_keeps_partial_when_only_pull_requests_seen_before_failure() {
    let dir = unique_temp_dir("sync-pr-only-partial");
    let db_path = dir.join("blippy.db");
    let conn = open_db_at(&db_path).expect("open db");

    let repo = ApiRepo {
        id: 1,
        name: "blippy".to_string(),
        owner: ApiUser {
            login: "acme".to_string(),
            user_type: None,
        },
        permissions: None,
    };
    let issues = vec![ApiIssue {
        id: 11,
        number: 2,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: None,
        comments: 0,
        updated_at: None,
        labels: Vec::new(),
        assignees: Vec::new(),
        user: ApiUser {
            login: "dev".to_string(),
            user_type: None,
        },
        pull_request: Some(serde_json::json!({"url": "x"})),
    }];
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: Some(2),
        issue_page_size: 1,
        page_etag: Some("etag-pr-only".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 1);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("blippy-sync-{}-{}", label, nanos));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
