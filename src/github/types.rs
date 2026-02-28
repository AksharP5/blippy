use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ApiUser {
    pub login: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub user_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiLabel {
    pub name: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ApiRepoPermissions {
    #[allow(dead_code)]
    #[serde(default)]
    pub pull: bool,
    #[serde(default)]
    pub triage: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub maintain: bool,
    #[serde(default)]
    pub admin: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiIssue {
    pub id: i64,
    pub number: i64,
    pub state: String,
    pub title: String,
    pub body: Option<String>,
    pub comments: i64,
    pub updated_at: Option<String>,
    pub labels: Vec<ApiLabel>,
    pub assignees: Vec<ApiUser>,
    #[allow(dead_code)]
    pub user: ApiUser,
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiComment {
    pub id: i64,
    pub body: Option<String>,
    pub created_at: Option<String>,
    pub user: ApiUser,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiRepo {
    pub id: i64,
    pub name: String,
    pub owner: ApiUser,
    #[serde(default)]
    pub permissions: Option<ApiRepoPermissions>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPullRequestFile {
    pub filename: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
    pub patch: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPullRequestHead {
    pub sha: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPullRequestSummary {
    pub head: ApiPullRequestHead,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPullRequestDetails {
    #[serde(default)]
    pub merge_commit_allowed: bool,
    #[serde(default)]
    pub squash_merge_allowed: bool,
    #[serde(default)]
    pub rebase_merge_allowed: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ApiPullRequestMergeResponse {
    #[serde(default)]
    pub merged: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPullRequestReviewComment {
    pub id: i64,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub is_resolved: bool,
    pub path: String,
    pub line: Option<i64>,
    pub original_line: Option<i64>,
    pub side: Option<String>,
    pub in_reply_to_id: Option<i64>,
    pub body: Option<String>,
    pub created_at: Option<String>,
    pub user: ApiUser,
}

#[derive(Debug, Clone)]
pub struct ApiIssuesPage {
    pub issues: Vec<ApiIssue>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ApiIssuesPageResult {
    NotModified,
    Page(ApiIssuesPage),
}
