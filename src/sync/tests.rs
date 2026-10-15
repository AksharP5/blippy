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
    sync_repo_with_progress(client, conn, owner, repo, |_page, _stats| {})
        .await
        .map(|result| result.stats)
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
        user: Some(ApiUser {
            login: "dev".to_string(),
            user_type: None,
        }),
        pull_request: Some(serde_json::json!({"url": "x"})),
    };
    let row = map_issue_to_row(1, &issue);
    assert!(row.is_pr);
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
        user: Some(ApiUser {
            login: "dev".to_string(),
            user_type: None,
        }),
        pull_request: Some(serde_json::json!({
            "url": "x",
            "merged_at": "2024-02-01T12:00:00Z"
        })),
    };

    let row = map_issue_to_row(1, &issue);
    assert!(row.is_pr);
    assert_eq!(row.state, "merged");
}

#[test]
fn map_issue_to_row_preserves_labels_and_assignees() {
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
        user: Some(ApiUser {
            login: "dev".to_string(),
            user_type: None,
        }),
        pull_request: None,
    };
    let row = map_issue_to_row(1, &issue);
    assert_eq!(row.labels, vec!["bug"]);
    assert_eq!(row.assignees, "dev");
    assert_eq!(row.comments_count, 3);
}

#[test]
fn map_comment_to_row_copies_author() {
    let comment = ApiComment {
        id: 50,
        body: Some("hello".to_string()),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        user: Some(ApiUser {
            login: "dev".to_string(),
            user_type: None,
        }),
    };
    let row = map_comment_to_row(99, &comment);
    assert_eq!(row.issue_id, 99);
    assert_eq!(row.author, "dev");
    assert_eq!(row.body, "hello");
}

#[test]
fn map_comment_to_row_uses_unknown_for_unavailable_authors() {
    let comment: ApiComment = serde_json::from_value(serde_json::json!({
        "id": 50,
        "body": "Preserved comment",
        "user": null,
    }))
    .expect("parse comment");
    let row = map_comment_to_row(99, &comment);
    assert_eq!(row.author, "unknown");
    assert_eq!(row.body, "Preserved comment");
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
            user: Some(ApiUser {
                login: "dev".to_string(),
                user_type: None,
            }),
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
            user: Some(ApiUser {
                login: "dev".to_string(),
                user_type: None,
            }),
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
    assert_eq!(stats.incomplete_reason, None);
    assert_eq!(stats.comments, 0);

    let rows = list_issues(&conn, 1).expect("list issues");
    assert_eq!(rows.len(), 2);
    let comments = comments_for_issue(&conn, 10).expect("comments");
    assert_eq!(comments.len(), 0);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

fn issue_fixture(number: i64, updated_at: Option<&str>) -> ApiIssue {
    ApiIssue {
        id: number + 1_000,
        number,
        state: "open".to_string(),
        title: format!("Issue {number}"),
        body: None,
        comments: 0,
        updated_at: updated_at.map(ToString::to_string),
        labels: Vec::new(),
        assignees: Vec::new(),
        user: None,
        pull_request: None,
    }
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
        owner: &str,
        repo: &str,
        page: u32,
        if_none_match: Option<&str>,
        _since: Option<&str>,
    ) -> anyhow::Result<ApiIssuesPageResult> {
        assert_eq!(owner, self.repo.owner.login);
        assert_eq!(repo, self.repo.name);
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
    let issues = (1..=201)
        .map(|number| issue_fixture(number, Some("2024-01-01T00:00:00Z")))
        .collect();
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: Some(3),
        issue_page_size: 100,
        page_etag: Some("etag-partial".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 200);
    assert_eq!(stats.incomplete_reason.as_deref(), Some("rate limit"));

    let rows = list_issues(&conn, 1).expect("list issues");
    assert_eq!(rows.len(), 200);

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
    let issues = (1..=101)
        .map(|number| issue_fixture(number, Some("2024-01-01T00:00:00Z")))
        .collect();
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: Some("etag-progress".to_string()),
        not_modified_when_etag_matches: false,
    };

    let mut progress = Vec::new();
    let result = sync_repo_with_progress(&client, &conn, "acme", "blippy", |page, stats| {
        progress.push((page, stats.issues));
    })
    .await
    .expect("sync");

    assert_eq!(result.stats.issues, 101);
    assert_eq!(progress, vec![(1, 100), (2, 101)]);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_finishes_a_short_page_without_another_request() {
    let conn = open_db_at(std::path::Path::new(":memory:")).expect("open db");
    let client = FakeGitHub {
        repo: serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "blippy",
            "owner": {"login": "acme"},
        }))
        .expect("API repo"),
        issues: vec![issue_fixture(1, Some("2024-01-05T00:00:00Z"))],
        fail_get_repo: false,
        fail_issue_page: Some(2),
        issue_page_size: 100,
        page_etag: Some("short-page".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync complete first page");
    assert_eq!(stats.issues, 1);
    assert_eq!(stats.incomplete_reason, None);
    let repo = get_repo_by_slug(&conn, "acme", "blippy")
        .expect("lookup")
        .expect("repo");
    assert_eq!(repo.updated_at.as_deref(), Some("2024-01-05T00:00:00Z"));
    assert_eq!(repo.etag, None);
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
            user: Some(ApiUser {
                login: "dev".to_string(),
                user_type: None,
            }),
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
            user: Some(ApiUser {
                login: "dev".to_string(),
                user_type: None,
            }),
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
    assert_eq!(stored_repo.etag, None);

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sync_repo_reports_no_changes_after_etag_and_later_pages_are_checked() {
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
async fn sync_repo_preserves_state_when_a_later_page_fails_after_not_modified() {
    let conn = open_db_at(std::path::Path::new(":memory:")).expect("open db");
    let existing = crate::store::RepoRow {
        id: 1,
        owner: "acme".to_string(),
        name: "blippy".to_string(),
        updated_at: Some("2024-01-05T00:00:00Z".to_string()),
        etag: Some("unchanged-first-page".to_string()),
    };
    crate::store::upsert_repo(&conn, &existing).expect("seed repo state");
    let client = FakeGitHub {
        repo: serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "blippy",
            "owner": {"login": "acme"},
        }))
        .expect("API repo"),
        issues: Vec::new(),
        fail_get_repo: false,
        fail_issue_page: Some(2),
        issue_page_size: 100,
        page_etag: existing.etag.clone(),
        not_modified_when_etag_matches: true,
    };

    let result = sync_repo_with_progress(&client, &conn, "acme", "blippy", |_, _| {})
        .await
        .expect("return incomplete sync");
    assert_eq!(result.stats.issues, 0);
    assert!(!result.stats.not_modified);
    assert_eq!(
        result.stats.incomplete_reason.as_deref(),
        Some("rate limit")
    );
    assert_eq!(result.repo, existing);
    assert_eq!(
        get_repo_by_slug(&conn, "acme", "blippy").expect("stored state"),
        Some(existing)
    );
}

#[tokio::test]
async fn sync_repo_fetches_later_same_timestamp_changes_despite_first_page_etag() {
    for cached_count in [1, 99, 100, 101] {
        let conn = open_db_at(std::path::Path::new(":memory:")).expect("open db");
        let existing = crate::store::RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: Some("2024-01-05T00:00:00Z".to_string()),
            etag: Some("unchanged-first-page".to_string()),
        };
        crate::store::upsert_repo(&conn, &existing).expect("seed repo state");
        let mut client = FakeGitHub {
            repo: serde_json::from_value(serde_json::json!({
                "id": 1,
                "name": "blippy",
                "owner": {"login": "acme"},
            }))
            .expect("API repo"),
            issues: (1..=101)
                .rev()
                .map(|number| issue_fixture(number, existing.updated_at.as_deref()))
                .collect(),
            fail_get_repo: false,
            fail_issue_page: None,
            issue_page_size: 100,
            page_etag: Some("unchanged-first-page".to_string()),
            not_modified_when_etag_matches: true,
        };
        for issue in &client.issues[..cached_count] {
            crate::store::upsert_issue(&conn, &map_issue_to_row(1, issue))
                .expect("seed cached issue");
        }
        client.issues[100].title = "Changed on the second page".to_string();

        let stats = sync_repo(&client, &conn, "acme", "blippy")
            .await
            .expect("sync same-second change");
        let changed = list_issues(&conn, 1)
            .expect("cached issues")
            .into_iter()
            .find(|issue| issue.number == 1)
            .expect("second-page issue");
        assert_eq!(changed.title, "Changed on the second page");
        assert!(!stats.not_modified);
    }
}

#[tokio::test]
async fn sync_repo_preserves_cache_and_cursor_when_repository_is_renamed_or_transferred() {
    for (owner, name) in [("old-owner", "old-name"), ("new-owner", "new-name")] {
        let conn = open_db_at(std::path::Path::new(":memory:")).expect("open db");
        let existing = crate::store::RepoRow {
            id: 1,
            owner: "old-owner".to_string(),
            name: "old-name".to_string(),
            updated_at: Some("2024-01-05T00:00:00Z".to_string()),
            etag: Some("etag-stable".to_string()),
        };
        crate::store::upsert_repo(&conn, &existing).expect("seed repo state");
        let cached_issue = map_issue_to_row(1, &issue_fixture(1, existing.updated_at.as_deref()));
        crate::store::upsert_issue(&conn, &cached_issue).expect("seed cached issue");
        let client = FakeGitHub {
            repo: serde_json::from_value(serde_json::json!({
                "id": 1,
                "name": "new-name",
                "owner": {"login": "new-owner"},
            }))
            .expect("API repo"),
            issues: Vec::new(),
            fail_get_repo: false,
            fail_issue_page: None,
            issue_page_size: 100,
            page_etag: existing.etag.clone(),
            not_modified_when_etag_matches: true,
        };

        let result = sync_repo_with_progress(&client, &conn, owner, name, |_, _| {})
            .await
            .expect("sync renamed repository");
        assert!(result.stats.not_modified);
        assert_eq!(result.repo.owner, "new-owner");
        assert_eq!(result.repo.name, "new-name");
        assert_eq!(result.repo.updated_at, existing.updated_at);
        assert_eq!(result.repo.etag, existing.etag);
        assert_eq!(
            get_repo_by_slug(&conn, "new-owner", "new-name").expect("canonical repo"),
            Some(result.repo)
        );
        assert!(
            get_repo_by_slug(&conn, "old-owner", "old-name")
                .expect("old repo")
                .is_none()
        );
        assert_eq!(
            list_issues(&conn, 1).expect("cached issues"),
            vec![cached_issue]
        );
    }
}

#[tokio::test]
async fn sync_repo_resolves_a_cold_redirect_and_reuses_its_cursor_on_the_next_sync() {
    let conn = open_db_at(std::path::Path::new(":memory:")).expect("open db");
    let mut client = FakeGitHub {
        repo: serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "new-name",
            "owner": {"login": "new-owner"},
        }))
        .expect("API repo"),
        issues: vec![issue_fixture(1, Some("2024-01-05T00:00:00Z"))],
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: Some("etag-stable".to_string()),
        not_modified_when_etag_matches: false,
    };

    let first = sync_repo_with_progress(&client, &conn, "old-owner", "old-name", |_, _| {})
        .await
        .expect("sync redirected repository");
    assert_eq!(first.stats.issues, 1);
    assert_eq!(first.repo.owner, "new-owner");
    assert_eq!(first.repo.name, "new-name");
    assert_eq!(
        first.repo.updated_at.as_deref(),
        Some("2024-01-05T00:00:00Z")
    );
    assert_eq!(first.repo.etag, None);

    client.not_modified_when_etag_matches = true;
    let second = sync_repo_with_progress(&client, &conn, "old-owner", "old-name", |_, _| {})
        .await
        .expect("repeat sync through the old remote");
    assert!(!second.stats.not_modified);
    assert_eq!(second.stats.issues, 1);
    assert_eq!(second.repo.updated_at, first.repo.updated_at);
    assert_eq!(second.repo.etag, client.page_etag);

    let third = sync_repo_with_progress(&client, &conn, "old-owner", "old-name", |_, _| {})
        .await
        .expect("reuse ETag for the same cursor");
    assert!(third.stats.not_modified);
    assert_eq!(third.repo, second.repo);
    assert_eq!(list_issues(&conn, 1).expect("cached issues").len(), 1);
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
    let issues = (1..=101)
        .map(|number| issue_fixture(number, Some("2024-01-03T00:00:00Z")))
        .collect();
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: Some(2),
        issue_page_size: 100,
        page_etag: Some("etag-new".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 100);
    assert_eq!(stats.incomplete_reason.as_deref(), Some("rate limit"));
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
    let issues = (1..=101)
        .map(|number| ApiIssue {
            pull_request: Some(serde_json::json!({"url": "x"})),
            ..issue_fixture(number, None)
        })
        .collect();
    let client = FakeGitHub {
        repo,
        issues,
        fail_get_repo: false,
        fail_issue_page: Some(2),
        issue_page_size: 100,
        page_etag: Some("etag-pr-only".to_string()),
        not_modified_when_etag_matches: false,
    };

    let stats = sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync");
    assert_eq!(stats.issues, 100);
    assert_eq!(stats.incomplete_reason.as_deref(), Some("rate limit"));

    drop(conn);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn labels_round_trip_from_api_through_cache_and_picker() {
    use crate::app::{App, View};
    use crate::config::Config;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let dir = unique_temp_dir("label-round-trip");
    let conn = open_db_at(&dir.join("blippy.db")).expect("open db");
    let names = vec!["api, v2".to_string(), r#"["quoted"]"#.to_string()];
    let issue: ApiIssue = serde_json::from_value(serde_json::json!({
        "id": 10,
        "number": 1,
        "state": "open",
        "title": "Preserve label names",
        "body": null,
        "comments": 0,
        "updated_at": "2024-01-01T00:00:00Z",
        "labels": names.iter().map(|name| serde_json::json!({"name": name})).collect::<Vec<_>>(),
        "assignees": [],
        "user": {"login": "dev"},
        "pull_request": null,
    }))
    .expect("parse API issue");
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
        issues: vec![issue],
        fail_get_repo: false,
        fail_issue_page: None,
        issue_page_size: 100,
        page_etag: None,
        not_modified_when_etag_matches: false,
    };
    sync_repo(&client, &conn, "acme", "blippy")
        .await
        .expect("sync issue");
    let issues = list_issues(&conn, 1).expect("read cached issue");
    assert_eq!(issues[0].labels, names);

    let mut options = names.clone();
    options.push("bug".to_string());
    let mut app = App::new(Config::default());
    app.open_label_picker(View::Issues, options, &issues[0].labels);
    assert!(app.label_option_selected("api, v2"));
    assert!(app.label_option_selected(r#"["quoted"]"#));
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert_eq!(
        app.selected_labels(),
        vec![r#"["quoted"]"#, "api, v2", "bug"]
    );

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
