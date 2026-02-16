use anyhow::{Result, anyhow};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};

mod comments;
mod issues;
mod pull_requests;
mod repos;
mod types;

pub use types::*;

const API_BASE: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";

pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("blippy"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(API_VERSION),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self {
            client,
            token: token.to_string(),
        })
    }

    async fn graphql(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let response = self
            .client
            .post(format!("{}/graphql", API_BASE))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({
                "query": query,
                "variables": variables,
            }))
            .send()
            .await?
            .error_for_status()?;
        let payload = response.json::<serde_json::Value>().await?;
        if let Some(errors) = payload.get("errors") {
            return Err(anyhow!("graphql error: {}", errors));
        }
        Ok(payload)
    }
}
