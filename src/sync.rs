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
}

pub fn map_repo_to_row(repo: &ApiRepo) -> RepoRow {
    RepoRow {
        id: repo.id,
        owner: repo.owner.login.clone(),
        name: repo.name.clone(),
        updated_at: None,
        etag: None,
    }
}

pub fn map_issue_to_row(repo_id: i64, issue: &ApiIssue) -> Option<IssueRow> {
    let labels = issue
        .labels
        .iter()
        .map(|label| label.name.as_str())
        .collect::<Vec<&str>>()
        .join(",");
    let assignees = issue
        .assignees
        .iter()
        .map(|user| user.login.as_str())
        .collect::<Vec<&str>>()
        .join(",");
    Some(IssueRow {
        id: issue.id,
        repo_id,
        number: issue.number,
        state: issue.state.clone(),
        title: issue.title.clone(),
        body: issue.body.clone().unwrap_or_default(),
        labels,
        assignees,
        comments_count: issue.comments,
        updated_at: issue.updated_at.clone(),
        is_pr: issue.pull_request.is_some(),
    })
}

pub fn map_comment_to_row(issue_id: i64, comment: &ApiComment) -> CommentRow {
    CommentRow {
        id: comment.id,
        issue_id,
        author: comment.user.login.clone(),
        body: comment.body.clone().unwrap_or_default(),
        created_at: comment.created_at.clone(),
        last_accessed_at: Some(crate::store::comment_now_epoch()),
    }
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
                && issue_updated_at < cursor
            {
                reached_previous_cursor = true;
                break;
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
mod tests;
