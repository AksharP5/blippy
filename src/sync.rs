use anyhow::Result;
use async_trait::async_trait;

use crate::github::{ApiComment, ApiIssue, ApiIssuesPageResult, ApiRepo, GitHubClient};
use crate::store::{CommentRow, IssueRow, RepoRow};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncStats {
    pub issues: usize,
    pub comments: usize,
    pub not_modified: bool,
    pub incomplete_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncResult {
    pub repo: RepoRow,
    pub stats: SyncStats,
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

pub fn map_issue_to_row(repo_id: i64, issue: &ApiIssue) -> IssueRow {
    let labels = issue
        .labels
        .iter()
        .map(|label| label.name.clone())
        .collect();
    let assignees = issue
        .assignees
        .iter()
        .map(|user| user.login.as_str())
        .collect::<Vec<&str>>()
        .join(",");
    let is_pr = issue.pull_request.is_some();
    let is_merged = is_pr
        && issue
            .pull_request
            .as_ref()
            .and_then(|pull_request| pull_request.get("merged_at"))
            .and_then(serde_json::Value::as_str)
            .is_some();
    let state = if is_merged {
        "merged".to_string()
    } else {
        issue.state.clone()
    };
    IssueRow {
        id: issue.id,
        repo_id,
        number: issue.number,
        state,
        title: issue.title.clone(),
        body: issue.body.clone().unwrap_or_default(),
        labels,
        assignees,
        comments_count: issue.comments,
        updated_at: issue.updated_at.clone(),
        is_pr,
    }
}

pub fn map_comment_to_row(issue_id: i64, comment: &ApiComment) -> CommentRow {
    CommentRow {
        id: comment.id,
        issue_id,
        author: comment
            .user
            .as_ref()
            .map(|user| user.login.clone())
            .unwrap_or_else(|| "unknown".to_string()),
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
) -> Result<SyncResult>
where
    F: FnMut(u32, &SyncStats),
{
    let repo = _client.get_repo(_owner, _repo).await?;
    let stored_repo = crate::store::get_repo_by_id(_conn, repo.id)?;
    let mut repo_row = map_repo_to_row(&repo);
    if let Some(stored) = &stored_repo {
        repo_row.updated_at = stored.updated_at.clone();
        repo_row.etag = stored.etag.clone();
    }
    crate::store::upsert_repo(_conn, &repo_row)?;

    let previous_cursor = stored_repo
        .as_ref()
        .and_then(|stored_repo| stored_repo.updated_at.clone());
    let previous_etag = stored_repo
        .as_ref()
        .and_then(|stored_repo| stored_repo.etag.clone());

    // A first-page ETag cannot certify later pages at the same update timestamp.
    let use_first_page_etag = match (previous_etag.as_ref(), previous_cursor.as_deref()) {
        (Some(_), Some(cursor)) => {
            crate::store::count_issues_updated_since(_conn, repo_row.id, cursor)? < 100
        }
        _ => true,
    };

    let mut stats = SyncStats::default();
    let mut page = 1u32;
    let mut fetched_any_page = false;
    let mut latest_seen_updated_at = previous_cursor.clone();
    let mut first_page_etag = None;
    loop {
        let if_none_match = if page == 1 && use_first_page_etag {
            previous_etag.as_deref()
        } else {
            None
        };
        let page_result = _client
            .list_issues_page(
                &repo_row.owner,
                &repo_row.name,
                page,
                if_none_match,
                previous_cursor.as_deref(),
            )
            .await;
        let (issues, etag) = match page_result {
            Ok(ApiIssuesPageResult::NotModified) => {
                stats.not_modified = true;
                return Ok(SyncResult {
                    repo: repo_row,
                    stats,
                });
            }
            Ok(ApiIssuesPageResult::Page(page_result)) => {
                fetched_any_page = true;
                (page_result.issues, page_result.etag)
            }
            Err(error) => {
                if fetched_any_page {
                    stats.incomplete_reason = Some(error.to_string());
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
        let is_last_page = issues.len() < 100;
        let mut rows = Vec::new();
        let mut reached_previous_cursor = false;
        for issue in issues {
            if let (Some(cursor), Some(issue_updated_at)) =
                (previous_cursor.as_deref(), issue.updated_at.as_deref())
                && issue_updated_at < cursor
            {
                reached_previous_cursor = true;
                break;
            }

            let row = map_issue_to_row(repo_row.id, &issue);

            if let Some(updated_at) = row.updated_at.as_deref() {
                let should_replace = latest_seen_updated_at
                    .as_deref()
                    .is_none_or(|current| updated_at > current);
                if should_replace {
                    latest_seen_updated_at = Some(updated_at.to_string());
                }
            }

            rows.push(row);
        }

        let transaction = _conn.unchecked_transaction()?;
        for row in &rows {
            crate::store::upsert_issue(&transaction, row)?;
            stats.issues += 1;
        }
        transaction.commit()?;
        _on_progress(page, &stats);
        if reached_previous_cursor || is_last_page {
            break;
        }
        page += 1;
    }

    if stats.incomplete_reason.is_none() {
        let next_cursor = latest_seen_updated_at
            .as_deref()
            .or(previous_cursor.as_deref());
        let next_etag = first_page_etag.as_deref().or(previous_etag.as_deref());
        crate::store::update_repo_sync_state(_conn, repo_row.id, next_cursor, next_etag)?;
        repo_row.updated_at = next_cursor.map(ToString::to_string);
        repo_row.etag = next_etag.map(ToString::to_string);
    }

    Ok(SyncResult {
        repo: repo_row,
        stats,
    })
}

#[cfg(test)]
mod tests;
