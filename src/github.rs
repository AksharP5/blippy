use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
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

    pub async fn list_issues(&self, owner: &str, repo: &str) -> Result<Vec<ApiIssue>> {
        let mut page = 1;
        let mut issues = Vec::new();
        loop {
            let url = format!("{}/repos/{}/{}/issues", API_BASE, owner, repo);
            let response = self
                .client
                .get(url)
                .bearer_auth(&self.token)
                .query(&[("state", "open"), ("per_page", "100"), ("page", &page.to_string())])
                .send()
                .await?
                .error_for_status()?;
            let batch = response.json::<Vec<ApiIssue>>().await?;
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
}
