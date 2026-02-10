use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ETAG, IF_NONE_MATCH, USER_AGENT};
use serde::Deserialize;

const API_BASE: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";

#[derive(Debug, Deserialize, Clone)]
pub struct ApiUser {
    pub login: String,
    #[serde(rename = "type")]
    pub user_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiLabel {
    pub name: String,
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPullRequestFile {
    pub filename: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
    pub patch: Option<String>,
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

pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("glyph"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(API_VERSION),
        );

        let client = reqwest::Client::builder().default_headers(headers).build()?;
        Ok(Self {
            client,
            token: token.to_string(),
        })
    }

    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<ApiRepo> {
        let url = format!("{}/repos/{}/{}", API_BASE, owner, repo);
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json::<ApiRepo>().await?)
    }

    pub async fn list_issues_page(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
    ) -> Result<Vec<ApiIssue>> {
        let result = self
            .list_issues_page_conditional(owner, repo, page, None, None)
            .await?;
        match result {
            ApiIssuesPageResult::NotModified => Ok(Vec::new()),
            ApiIssuesPageResult::Page(page) => Ok(page.issues),
        }
    }

    pub async fn list_issues_page_conditional(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
        if_none_match: Option<&str>,
        since: Option<&str>,
    ) -> Result<ApiIssuesPageResult> {
        let url = format!("{}/repos/{}/{}/issues", API_BASE, owner, repo);
        let mut request = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .query(&[
                ("state", "all"),
                ("sort", "updated"),
                ("direction", "desc"),
                ("per_page", "100"),
                ("page", &page.to_string()),
            ]);
        if let Some(value) = if_none_match {
            request = request.header(IF_NONE_MATCH, value);
        }
        if let Some(value) = since {
            request = request.query(&[("since", value)]);
        }

        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(ApiIssuesPageResult::NotModified);
        }

        let response = response.error_for_status()?;
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let issues = response.json::<Vec<ApiIssue>>().await?;
        Ok(ApiIssuesPageResult::Page(ApiIssuesPage { issues, etag }))
    }

    pub async fn list_issues(&self, owner: &str, repo: &str) -> Result<Vec<ApiIssue>> {
        let mut page = 1u32;
        let mut issues = Vec::new();
        loop {
            let batch = self.list_issues_page(owner, repo, page).await?;
            if batch.is_empty() {
                break;
            }
            issues.extend(batch);
            page += 1;
        }
        Ok(issues)
    }

    pub async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Vec<ApiComment>> {
        let mut page = 1;
        let mut comments = Vec::new();
        loop {
            let url = format!(
                "{}/repos/{}/{}/issues/{}/comments",
                API_BASE, owner, repo, issue_number
            );
            let response = self
                .client
                .get(url)
                .bearer_auth(&self.token)
                .query(&[("per_page", "100"), ("page", &page.to_string())])
                .send()
                .await?
                .error_for_status()?;
            let batch = response.json::<Vec<ApiComment>>().await?;
            if batch.is_empty() {
                break;
            }
            comments.extend(batch);
            page += 1;
        }
        Ok(comments)
    }

    pub async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<Vec<ApiPullRequestFile>> {
        let mut page = 1;
        let mut files = Vec::new();
        loop {
            let url = format!(
                "{}/repos/{}/{}/pulls/{}/files",
                API_BASE, owner, repo, pull_number
            );
            let response = self
                .client
                .get(url)
                .bearer_auth(&self.token)
                .query(&[("per_page", "100"), ("page", &page.to_string())])
                .send()
                .await?
                .error_for_status()?;
            let batch = response.json::<Vec<ApiPullRequestFile>>().await?;
            if batch.is_empty() {
                break;
            }
            files.extend(batch);
            page += 1;
        }
        Ok(files)
    }

    pub async fn find_linked_pull_request_url(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Option<String>> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/timeline",
            API_BASE, owner, repo, issue_number
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .query(&[("per_page", "100")])
            .send()
            .await?
            .error_for_status()?;
        let events = response.json::<Vec<serde_json::Value>>().await?;

        for event in events {
            let issue = match event.get("source").and_then(|value| value.get("issue")) {
                Some(issue) => issue,
                None => continue,
            };
            if issue.get("pull_request").is_none() {
                continue;
            }
            let html_url = match issue.get("html_url").and_then(serde_json::Value::as_str) {
                Some(html_url) => html_url,
                None => continue,
            };
            if !html_url.contains("/pull/") {
                continue;
            }
            return Ok(Some(html_url.to_string()));
        }

        Ok(None)
    }

    pub async fn create_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        body: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            API_BASE, owner, repo, issue_number
        );
        self.client
            .post(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"body": body}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn update_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        body: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/comments/{}",
            API_BASE, owner, repo, comment_id
        );
        self.client
            .patch(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"body": body}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn delete_comment(&self, owner: &str, repo: &str, comment_id: i64) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/comments/{}",
            API_BASE, owner, repo, comment_id
        );
        self.client
            .delete(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn close_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            API_BASE, owner, repo, issue_number
        );
        self.client
            .patch(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"state": "closed"}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn reopen_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            API_BASE, owner, repo, issue_number
        );
        self.client
            .patch(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"state": "open"}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn update_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        labels: &[String],
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/labels",
            API_BASE, owner, repo, issue_number
        );
        self.client
            .put(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"labels": labels}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn update_issue_assignees(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        assignees: &[String],
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            API_BASE, owner, repo, issue_number
        );
        self.client
            .patch(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"assignees": assignees}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<String>> {
        let mut page = 1u32;
        let mut labels = Vec::new();
        loop {
            let url = format!("{}/repos/{}/{}/labels", API_BASE, owner, repo);
            let response = self
                .client
                .get(url)
                .bearer_auth(&self.token)
                .query(&[("per_page", "100"), ("page", &page.to_string())])
                .send()
                .await?
                .error_for_status()?;
            let batch = response.json::<Vec<ApiLabel>>().await?;
            if batch.is_empty() {
                break;
            }
            for label in batch {
                labels.push(label.name);
            }
            page += 1;
        }
        Ok(labels)
    }
}
