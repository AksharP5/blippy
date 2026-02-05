use anyhow::Result;

use crate::github::{ApiComment, ApiIssue, ApiRepo, GitHubClient};
use crate::store::{CommentRow, IssueRow, RepoRow};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncStats {
    pub issues: usize,
    pub comments: usize,
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
    if _issue.pull_request.is_some() {
        return None;
    }

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
        updated_at: _issue.updated_at.clone(),
        is_pr: false,
    })
}

pub fn map_comment_to_row(_issue_id: i64, _comment: &ApiComment) -> CommentRow {
    CommentRow {
        id: _comment.id,
        issue_id: _issue_id,
        author: _comment.user.login.clone(),
        body: _comment.body.clone().unwrap_or_default(),
        created_at: _comment.created_at.clone(),
    }
}

pub async fn sync_repo(
    _client: &GitHubClient,
    _conn: &rusqlite::Connection,
    _owner: &str,
    _repo: &str,
) -> Result<SyncStats> {
    todo!("Sync issues and comments for repo");
}

#[cfg(test)]
mod tests {
    use super::{map_comment_to_row, map_issue_to_row, map_repo_to_row};
    use crate::github::{ApiComment, ApiIssue, ApiLabel, ApiRepo, ApiUser};

    #[test]
    fn map_repo_to_row_copies_owner_and_name() {
        let repo = ApiRepo {
            id: 1,
            name: "glyph".to_string(),
            owner: ApiUser {
                login: "acme".to_string(),
                user_type: None,
            },
        };
        let row = map_repo_to_row(&repo);
        assert_eq!(row.id, 1);
        assert_eq!(row.owner, "acme");
        assert_eq!(row.name, "glyph");
    }

    #[test]
    fn map_issue_to_row_skips_pull_requests() {
        let issue = ApiIssue {
            id: 10,
            number: 1,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: Some("body".to_string()),
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
        assert!(row.is_none());
    }

    #[test]
    fn map_issue_to_row_builds_label_and_assignee_strings() {
        let issue = ApiIssue {
            id: 11,
            number: 2,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: Some("body".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            labels: vec![ApiLabel {
                name: "bug".to_string(),
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
}
