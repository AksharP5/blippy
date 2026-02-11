use anyhow::Result;
use async_trait::async_trait;

use crate::github::{ApiComment, ApiIssue, ApiIssuesPageResult, ApiRepo, GitHubClient};
use crate::store::{CommentRow, IssueRow, RepoRow};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncStats {
    pub issues: usize,
    pub comments: usize,
    pub not_modified: bool,
}

#[async_trait]
pub trait GitHubApi {
    async fn get_repo(&self, owner: &str, repo: &str) -> Result<ApiRepo>;
    async fn list_issues_page(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
        if_none_match: Option<&str>,
        since: Option<&str>,
    ) -> Result<ApiIssuesPageResult>;
    async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Vec<ApiComment>>;
}

#[async_trait]
impl GitHubApi for GitHubClient {
    async fn get_repo(&self, owner: &str, repo: &str) -> Result<ApiRepo> {
        self.get_repo(owner, repo).await
    }

    async fn list_issues_page(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
        if_none_match: Option<&str>,
        since: Option<&str>,
    ) -> Result<ApiIssuesPageResult> {
        self.list_issues_page_conditional(owner, repo, page, if_none_match, since)
            .await
    }

    async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Vec<ApiComment>> {
        self.list_comments(owner, repo, issue_number).await
    }
}

pub fn map_repo_to_row(_repo: &ApiRepo) -> RepoRow {
    RepoRow {
        id: _repo.id,
        owner: _repo.owner.login.clone(),
        name: _repo.name.clone(),
        updated_at: None,
        etag: None,
    }
}

pub fn map_issue_to_row(_repo_id: i64, _issue: &ApiIssue) -> Option<IssueRow> {
    let labels = _issue
        .labels
        .iter()
        .map(|label| label.name.as_str())
        .collect::<Vec<&str>>()
        .join(",");
    let assignees = _issue
        .assignees
        .iter()
        .map(|user| user.login.as_str())
        .collect::<Vec<&str>>()
        .join(",");
    Some(IssueRow {
        id: _issue.id,
        repo_id: _repo_id,
        number: _issue.number,
        state: _issue.state.clone(),
        title: _issue.title.clone(),
        body: _issue.body.clone().unwrap_or_default(),
        labels,
        assignees,
        comments_count: _issue.comments,
        updated_at: _issue.updated_at.clone(),
        is_pr: _issue.pull_request.is_some(),
    })
}

pub fn map_comment_to_row(_issue_id: i64, _comment: &ApiComment) -> CommentRow {
    CommentRow {
        id: _comment.id,
        issue_id: _issue_id,
        author: _comment.user.login.clone(),
        body: _comment.body.clone().unwrap_or_default(),
        created_at: _comment.created_at.clone(),
        last_accessed_at: Some(crate::store::comment_now_epoch()),
    }
}

pub async fn sync_repo(
    _client: &dyn GitHubApi,
    _conn: &rusqlite::Connection,
    _owner: &str,
    _repo: &str,
) -> Result<SyncStats> {
    sync_repo_with_progress(_client, _conn, _owner, _repo, |_page, _stats| {}).await
}

pub async fn sync_repo_with_progress<F>(
    _client: &dyn GitHubApi,
    _conn: &rusqlite::Connection,
    _owner: &str,
    _repo: &str,
    mut _on_progress: F,
) -> Result<SyncStats>
where
    F: FnMut(u32, &SyncStats),
{
    let stored_repo = crate::store::get_repo_by_slug(_conn, _owner, _repo)?;
    let repo_row = match stored_repo.as_ref() {
        Some(repo_row) => repo_row.clone(),
        None => {
            let repo = _client.get_repo(_owner, _repo).await?;
            let repo_row = map_repo_to_row(&repo);
            crate::store::upsert_repo(_conn, &repo_row)?;
            repo_row
        }
    };

    let previous_cursor = stored_repo
        .as_ref()
        .and_then(|stored_repo| stored_repo.updated_at.clone());
    let previous_etag = stored_repo
        .as_ref()
        .and_then(|stored_repo| stored_repo.etag.clone());

    let mut stats = SyncStats::default();
    let mut page = 1u32;
    let mut fetched_any_page = false;
    let mut sync_completed = true;
    let mut latest_seen_updated_at = previous_cursor.clone();
    let mut first_page_etag = None;
    const PROGRESS_BATCH: usize = 10;
    loop {
        let if_none_match = if page == 1 {
            previous_etag.as_deref()
        } else {
            None
        };
        let page_result = _client
            .list_issues_page(
                _owner,
                _repo,
                page,
                if_none_match,
                previous_cursor.as_deref(),
            )
            .await;
        let (issues, etag) = match page_result {
            Ok(ApiIssuesPageResult::NotModified) => {
                stats.not_modified = true;
                return Ok(stats);
            }
            Ok(ApiIssuesPageResult::Page(page_result)) => {
                fetched_any_page = true;
                (page_result.issues, page_result.etag)
            }
            Err(error) => {
                if fetched_any_page {
                    sync_completed = false;
                    break;
                }
                return Err(error);
            }
        };
        if page == 1 {
            first_page_etag = etag;
        }
        if issues.is_empty() {
            break;
        }
        let mut persisted_since_update = 0usize;
        let mut emitted_for_page = false;
        let mut reached_previous_cursor = false;
        for issue in issues {
            if let (Some(cursor), Some(issue_updated_at)) =
                (previous_cursor.as_deref(), issue.updated_at.as_deref())
            {
                if issue_updated_at < cursor {
                    reached_previous_cursor = true;
                    break;
                }
            }

            let row = match map_issue_to_row(repo_row.id, &issue) {
                Some(row) => row,
                None => continue,
            };

            if let Some(updated_at) = row.updated_at.as_deref() {
                let should_replace = latest_seen_updated_at
                    .as_deref()
                    .is_none_or(|current| updated_at > current);
                if should_replace {
                    latest_seen_updated_at = Some(updated_at.to_string());
                }
            }

            crate::store::upsert_issue(_conn, &row)?;
            stats.issues += 1;
            persisted_since_update += 1;
            if persisted_since_update >= PROGRESS_BATCH {
                _on_progress(page, &stats);
                emitted_for_page = true;
                persisted_since_update = 0;
            }
        }
        if persisted_since_update > 0 || !emitted_for_page {
            _on_progress(page, &stats);
        }
        if reached_previous_cursor {
            break;
        }
        page += 1;
    }

    if sync_completed {
        let next_cursor = latest_seen_updated_at
            .as_deref()
            .or(previous_cursor.as_deref());
        let next_etag = first_page_etag.as_deref().or(previous_etag.as_deref());
        crate::store::update_repo_sync_state(_conn, repo_row.id, next_cursor, next_etag)?;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::{
        GitHubApi, map_comment_to_row, map_issue_to_row, map_repo_to_row, sync_repo,
        sync_repo_with_progress,
    };
    use crate::github::{ApiComment, ApiIssue, ApiIssuesPageResult, ApiLabel, ApiRepo, ApiUser};
    use crate::store::{comments_for_issue, get_repo_by_slug, list_issues, open_db_at};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn map_repo_to_row_copies_owner_and_name() {
        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
            owner: ApiUser {
                login: "acme".to_string(),
                user_type: None,
            },
            permissions: None,
        };
        let row = map_repo_to_row(&repo);
        assert_eq!(row.id, 1);
        assert_eq!(row.owner, "acme");
        assert_eq!(row.name, "glyph");
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
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
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
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: None,
            issue_page_size: 100,
            page_etag: Some("etag-sync".to_string()),
            not_modified_when_etag_matches: false,
        };

        let stats = sync_repo(&client, &conn, "acme", "glyph")
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
        comments: HashMap<i64, Vec<ApiComment>>,
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

        async fn list_comments(
            &self,
            _owner: &str,
            _repo: &str,
            issue_number: i64,
        ) -> anyhow::Result<Vec<ApiComment>> {
            Ok(self
                .comments
                .get(&issue_number)
                .cloned()
                .unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn sync_repo_persists_partial_when_later_page_fails() {
        let dir = unique_temp_dir("sync-partial");
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
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
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: Some(3),
            issue_page_size: 1,
            page_etag: Some("etag-partial".to_string()),
            not_modified_when_etag_matches: false,
        };

        let stats = sync_repo(&client, &conn, "acme", "glyph")
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
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
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
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: None,
            issue_page_size: 1,
            page_etag: Some("etag-progress".to_string()),
            not_modified_when_etag_matches: false,
        };

        let mut progress = Vec::new();
        let stats = sync_repo_with_progress(&client, &conn, "acme", "glyph", |page, stats| {
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
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
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
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: None,
            issue_page_size: 100,
            page_etag: Some("etag-cursor".to_string()),
            not_modified_when_etag_matches: false,
        };

        sync_repo(&client, &conn, "acme", "glyph")
            .await
            .expect("sync");

        let stored_repo = get_repo_by_slug(&conn, "acme", "glyph")
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
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let existing = crate::store::RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: Some("2024-01-05T00:00:00Z".to_string()),
            etag: Some("etag-stable".to_string()),
        };
        crate::store::upsert_repo(&conn, &existing).expect("seed repo state");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
            owner: ApiUser {
                login: "acme".to_string(),
                user_type: None,
            },
            permissions: None,
        };
        let client = FakeGitHub {
            repo,
            issues: Vec::new(),
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: None,
            issue_page_size: 100,
            page_etag: Some("etag-stable".to_string()),
            not_modified_when_etag_matches: true,
        };

        let stats = sync_repo(&client, &conn, "acme", "glyph")
            .await
            .expect("sync");
        assert!(stats.not_modified);
        assert_eq!(stats.issues, 0);

        let stored_repo = get_repo_by_slug(&conn, "acme", "glyph")
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
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let existing = crate::store::RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: Some("2024-01-05T00:00:00Z".to_string()),
            etag: Some("etag-stable".to_string()),
        };
        crate::store::upsert_repo(&conn, &existing).expect("seed repo state");

        let client = FakeGitHub {
            repo: ApiRepo {
                id: 1,
                name: "glyph".to_string(),
                owner: ApiUser {
                    login: "acme".to_string(),
                    user_type: None,
                },
                permissions: None,
            },
            issues: Vec::new(),
            comments: HashMap::new(),
            fail_get_repo: true,
            fail_issue_page: None,
            issue_page_size: 100,
            page_etag: Some("etag-stable".to_string()),
            not_modified_when_etag_matches: true,
        };

        let stats = sync_repo(&client, &conn, "acme", "glyph")
            .await
            .expect("sync");
        assert!(stats.not_modified);

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn sync_repo_does_not_advance_cursor_on_partial_failure() {
        let dir = unique_temp_dir("sync-cursor-partial");
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let existing = crate::store::RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "glyph".to_string(),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            etag: Some("etag-old".to_string()),
        };
        crate::store::upsert_repo(&conn, &existing).expect("seed repo state");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
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
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: Some(2),
            issue_page_size: 1,
            page_etag: Some("etag-new".to_string()),
            not_modified_when_etag_matches: false,
        };

        let stats = sync_repo(&client, &conn, "acme", "glyph")
            .await
            .expect("sync");
        assert_eq!(stats.issues, 1);
        assert!(!stats.not_modified);

        let stored_repo = get_repo_by_slug(&conn, "acme", "glyph")
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
        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
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
            comments: HashMap::new(),
            fail_get_repo: false,
            fail_issue_page: Some(2),
            issue_page_size: 1,
            page_etag: Some("etag-pr-only".to_string()),
            not_modified_when_etag_matches: false,
        };

        let stats = sync_repo(&client, &conn, "acme", "glyph")
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
        let dir = std::env::temp_dir().join(format!("glyph-sync-{}-{}", label, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
