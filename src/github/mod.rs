use anyhow::{Result, anyhow};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use std::collections::HashSet;
use std::time::Duration;

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
            .timeout(Duration::from_secs(30))
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

fn next_graphql_cursor(
    page_info: &serde_json::Value,
    seen: &mut HashSet<String>,
) -> Result<Option<String>> {
    if !page_info["hasNextPage"].as_bool().unwrap_or(false) {
        return Ok(None);
    }
    let cursor = page_info["endCursor"]
        .as_str()
        .ok_or_else(|| anyhow!("GitHub pagination returned hasNextPage without an end cursor"))?;
    if !seen.insert(cursor.to_string()) {
        return Err(anyhow!("GitHub pagination repeated an end cursor"));
    }
    Ok(Some(cursor.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_pagination_rejects_missing_or_repeated_cursors() {
        let mut seen = HashSet::new();
        assert!(
            next_graphql_cursor(
                &serde_json::json!({
                    "hasNextPage": true,
                    "endCursor": null,
                }),
                &mut seen
            )
            .is_err()
        );
        let page_info = serde_json::json!({"hasNextPage": true, "endCursor": "next"});
        assert_eq!(
            next_graphql_cursor(&page_info, &mut seen).expect("next cursor"),
            Some("next".to_string())
        );
        assert!(next_graphql_cursor(&page_info, &mut seen).is_err());
        assert_eq!(
            next_graphql_cursor(&serde_json::json!({"hasNextPage": false}), &mut seen)
                .expect("last page"),
            None
        );
    }
}
