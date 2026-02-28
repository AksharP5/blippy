use std::collections::{HashMap, HashSet};

use super::*;

impl GitHubClient {
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

    pub async fn pull_request_file_view_state(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<(Option<String>, HashSet<String>)> {
        let query = r#"
            query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
              repository(owner: $owner, name: $repo) {
                pullRequest(number: $number) {
                  id
                  files(first: 100, after: $cursor) {
                    pageInfo {
                      hasNextPage
                      endCursor
                    }
                    nodes {
                      path
                      viewerViewedState
                    }
                  }
                }
              }
            }
        "#;
        let id_only_query = r#"
            query($owner: String!, $repo: String!, $number: Int!) {
              repository(owner: $owner, name: $repo) {
                pullRequest(number: $number) {
                  id
                }
              }
            }
        "#;

        let mut cursor: Option<String> = None;
        let mut pull_request_id: Option<String> = None;
        let mut viewed_files = HashSet::new();

        loop {
            let payload = serde_json::json!({
                "owner": owner,
                "repo": repo,
                "number": pull_number,
                "cursor": cursor,
            });
            let response = match self.graphql(query, payload).await {
                Ok(response) => response,
                Err(_) => {
                    let fallback = self
                        .graphql(
                            id_only_query,
                            serde_json::json!({
                                "owner": owner,
                                "repo": repo,
                                "number": pull_number,
                            }),
                        )
                        .await?;
                    let pull_request_id = fallback["data"]["repository"]["pullRequest"]
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string);
                    return Ok((pull_request_id, HashSet::new()));
                }
            };
            let pull_request = &response["data"]["repository"]["pullRequest"];
            if pull_request.is_null() {
                return Ok((None, HashSet::new()));
            }

            if pull_request_id.is_none() {
                pull_request_id = pull_request
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string);
            }

            let files = pull_request["files"]["nodes"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for file in files {
                let path = match file.get("path").and_then(serde_json::Value::as_str) {
                    Some(path) => path,
                    None => continue,
                };
                let viewed = file
                    .get("viewerViewedState")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|state| state.eq_ignore_ascii_case("VIEWED"));
                if viewed {
                    viewed_files.insert(path.to_string());
                }
            }

            let has_next_page = pull_request["files"]["pageInfo"]["hasNextPage"]
                .as_bool()
                .unwrap_or(false);
            if !has_next_page {
                break;
            }
            cursor = pull_request["files"]["pageInfo"]["endCursor"]
                .as_str()
                .map(ToString::to_string);
        }

        Ok((pull_request_id, viewed_files))
    }

    pub async fn set_pull_request_file_viewed(
        &self,
        pull_request_id: &str,
        path: &str,
        viewed: bool,
    ) -> Result<()> {
        let mutation = if viewed {
            "mutation($pullRequestId: ID!, $path: String!) { markFileAsViewed(input: { pullRequestId: $pullRequestId, path: $path }) { clientMutationId } }"
        } else {
            "mutation($pullRequestId: ID!, $path: String!) { unmarkFileAsViewed(input: { pullRequestId: $pullRequestId, path: $path }) { clientMutationId } }"
        };
        self.graphql(
            mutation,
            serde_json::json!({
                "pullRequestId": pull_request_id,
                "path": path,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn pull_request_head_sha(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<String> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            API_BASE, owner, repo, pull_number
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?;
        let pull = response.json::<ApiPullRequestSummary>().await?;
        Ok(pull.head.sha)
    }

    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<()> {
        let details_url = format!(
            "{}/repos/{}/{}/pulls/{}",
            API_BASE, owner, repo, pull_number
        );
        let details = self
            .client
            .get(details_url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?
            .json::<ApiPullRequestDetails>()
            .await?;

        let merge_methods = preferred_merge_methods(&details);
        if merge_methods.is_empty() {
            return Err(anyhow::anyhow!(
                "no merge methods are enabled in this repository"
            ));
        }

        let merge_url = format!(
            "{}/repos/{}/{}/pulls/{}/merge",
            API_BASE, owner, repo, pull_number
        );
        let mut last_error = String::new();
        for merge_method in merge_methods {
            let response = self
                .client
                .put(merge_url.as_str())
                .bearer_auth(&self.token)
                .json(&serde_json::json!({ "merge_method": merge_method }))
                .send()
                .await?;
            let status = response.status();
            let payload_text = response.text().await.unwrap_or_default();

            if status.is_success() {
                let payload =
                    serde_json::from_str::<ApiPullRequestMergeResponse>(payload_text.as_str())
                        .unwrap_or_default();
                if payload.merged {
                    return Ok(());
                }
                if !payload.message.is_empty() {
                    last_error = payload.message;
                    continue;
                }
                last_error = format!("GitHub merge endpoint returned {}", status);
                continue;
            }

            let api_error = parse_api_error_message(payload_text.as_str())
                .unwrap_or_else(|| payload_text.trim().to_string());
            if !api_error.is_empty() {
                last_error = api_error;
            } else {
                last_error = format!("GitHub merge endpoint returned {}", status);
            }
        }

        if last_error.is_empty() {
            return Err(anyhow::anyhow!("merge failed"));
        }
        Err(anyhow::anyhow!(last_error))
    }

    pub async fn list_pull_request_review_comments(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<Vec<ApiPullRequestReviewComment>> {
        let thread_map = self
            .list_pull_request_review_thread_map(owner, repo, pull_number)
            .await
            .unwrap_or_default();

        let mut page = 1;
        let mut comments = Vec::new();
        loop {
            let url = format!(
                "{}/repos/{}/{}/pulls/{}/comments",
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
            let batch = response.json::<Vec<ApiPullRequestReviewComment>>().await?;
            if batch.is_empty() {
                break;
            }
            for mut comment in batch {
                if let Some((thread_id, resolved)) = thread_map.get(&comment.id) {
                    comment.thread_id = Some(thread_id.clone());
                    comment.is_resolved = *resolved;
                }
                comments.push(comment);
            }
            page += 1;
        }
        Ok(comments)
    }

    async fn list_pull_request_review_thread_map(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<HashMap<i64, (String, bool)>> {
        let query = r#"
            query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
              repository(owner: $owner, name: $repo) {
                pullRequest(number: $number) {
                  reviewThreads(first: 100, after: $cursor) {
                    pageInfo {
                      hasNextPage
                      endCursor
                    }
                    nodes {
                      id
                      isResolved
                      comments(first: 100) {
                        nodes {
                          databaseId
                        }
                      }
                    }
                  }
                }
              }
            }
        "#;

        let mut cursor: Option<String> = None;
        let mut map = HashMap::new();
        loop {
            let payload = serde_json::json!({
                "owner": owner,
                "repo": repo,
                "number": pull_number,
                "cursor": cursor,
            });
            let response = self.graphql(query, payload).await?;
            let pull_request = &response["data"]["repository"]["pullRequest"];
            if pull_request.is_null() {
                break;
            }
            let threads = pull_request["reviewThreads"]["nodes"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for thread in threads {
                let thread_id = match thread
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
                {
                    Some(thread_id) => thread_id,
                    None => continue,
                };
                let is_resolved = thread
                    .get("isResolved")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let thread_comments = thread["comments"]["nodes"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                for comment in thread_comments {
                    let comment_id = match comment
                        .get("databaseId")
                        .and_then(serde_json::Value::as_i64)
                    {
                        Some(comment_id) => comment_id,
                        None => continue,
                    };
                    map.insert(comment_id, (thread_id.clone(), is_resolved));
                }
            }

            let has_next_page = pull_request["reviewThreads"]["pageInfo"]["hasNextPage"]
                .as_bool()
                .unwrap_or(false);
            if !has_next_page {
                break;
            }
            cursor = pull_request["reviewThreads"]["pageInfo"]["endCursor"]
                .as_str()
                .map(ToString::to_string);
        }
        Ok(map)
    }

    pub async fn set_pull_request_review_thread_resolved(
        &self,
        _owner: &str,
        _repo: &str,
        thread_id: &str,
        resolved: bool,
    ) -> Result<()> {
        let mutation = if resolved {
            "mutation($threadId: ID!) { resolveReviewThread(input: { threadId: $threadId }) { thread { id isResolved } } }"
        } else {
            "mutation($threadId: ID!) { unresolveReviewThread(input: { threadId: $threadId }) { thread { id isResolved } } }"
        };
        self.graphql(
            mutation,
            serde_json::json!({
                "threadId": thread_id,
            }),
        )
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
        commit_id: &str,
        path: &str,
        line: i64,
        side: &str,
        start_line: Option<i64>,
        start_side: Option<&str>,
        body: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/comments",
            API_BASE, owner, repo, pull_number
        );
        let mut payload = serde_json::json!({
            "body": body,
            "commit_id": commit_id,
            "path": path,
            "line": line,
            "side": side,
        });
        if let Some(start_line) = start_line {
            payload["start_line"] = serde_json::json!(start_line);
        }
        if let Some(start_side) = start_side {
            payload["start_side"] = serde_json::json!(start_side);
        }

        self.client
            .post(url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn update_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        body: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/pulls/comments/{}",
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

    pub async fn delete_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/pulls/comments/{}",
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
}

fn preferred_merge_methods(details: &ApiPullRequestDetails) -> Vec<&'static str> {
    let mut methods = Vec::new();
    if details.merge_commit_allowed {
        methods.push("merge");
    }
    if details.squash_merge_allowed {
        methods.push("squash");
    }
    if details.rebase_merge_allowed {
        methods.push("rebase");
    }
    methods
}

fn parse_api_error_message(payload: &str) -> Option<String> {
    let parsed = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    parsed
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}
