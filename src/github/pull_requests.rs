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
            let is_last_page = batch.len() < 100;
            files.extend(batch);
            if is_last_page {
                break;
            }
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
        let mut cursor: Option<String> = None;
        let mut pull_request_id: Option<String> = None;
        let mut viewed_files = HashSet::new();
        let mut cursors = HashSet::new();

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

            cursor = next_graphql_cursor(&pull_request["files"]["pageInfo"], &mut cursors)?;
            if cursor.is_none() {
                break;
            }
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
        let repo_details = self
            .client
            .get(format!("{}/repos/{}/{}", API_BASE, owner, repo))
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?
            .json::<ApiRepoMergeSettings>()
            .await?;
        let mut merge_methods = preferred_merge_methods(&repo_details);
        if merge_methods.is_empty() {
            merge_methods = vec!["merge", "squash", "rebase"];
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
            .await?;

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
            let is_last_page = batch.len() < 100;
            for mut comment in batch {
                apply_review_thread_metadata(&mut comment, &thread_map);
                comments.push(comment);
            }
            if is_last_page {
                break;
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
                          fullDatabaseId
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
        let mut cursors = HashSet::new();
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
                append_review_thread_metadata(&thread, &mut map)?;
            }

            cursor = next_graphql_cursor(&pull_request["reviewThreads"]["pageInfo"], &mut cursors)?;
            if cursor.is_none() {
                break;
            }
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

fn append_review_thread_metadata(
    thread: &serde_json::Value,
    map: &mut HashMap<i64, (String, bool)>,
) -> Result<()> {
    let Some(thread_id) = thread.get("id").and_then(serde_json::Value::as_str) else {
        return Ok(());
    };
    let is_resolved = thread
        .get("isResolved")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let Some(comments) = thread["comments"]["nodes"].as_array() else {
        return Ok(());
    };
    for comment in comments {
        let Some(value) = comment
            .get("fullDatabaseId")
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        let comment_id = value
            .as_str()
            .and_then(|id| id.parse::<i64>().ok())
            .ok_or_else(|| {
                anyhow!("GitHub review comment returned invalid fullDatabaseId: {value}")
            })?;
        map.insert(comment_id, (thread_id.to_string(), is_resolved));
    }
    Ok(())
}

fn apply_review_thread_metadata(
    comment: &mut ApiPullRequestReviewComment,
    thread_map: &HashMap<i64, (String, bool)>,
) {
    if let Some((thread_id, resolved)) = thread_map.get(&comment.id).or_else(|| {
        comment
            .in_reply_to_id
            .and_then(|parent_id| thread_map.get(&parent_id))
    }) {
        comment.thread_id = Some(thread_id.clone());
        comment.is_resolved = *resolved;
    }
}

fn preferred_merge_methods(repo: &ApiRepoMergeSettings) -> Vec<&'static str> {
    let mut methods = Vec::new();
    if repo.allow_merge_commit {
        methods.push("merge");
    }
    if repo.allow_squash_merge {
        methods.push("squash");
    }
    if repo.allow_rebase_merge {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_thread_metadata_accepts_full_database_ids() {
        let thread = serde_json::json!({
            "id": "thread",
            "isResolved": true,
            "comments": {"nodes": [
                {"databaseId": null, "fullDatabaseId": "2147483648"},
            ]},
        });
        let mut map = HashMap::new();
        append_review_thread_metadata(&thread, &mut map).expect("parse 64-bit comment ID");
        let mut comment: ApiPullRequestReviewComment =
            serde_json::from_value(serde_json::json!({"id": 2147483648i64, "path": "src/main.rs"}))
                .expect("parse comment");
        apply_review_thread_metadata(&mut comment, &map);
        assert_eq!(comment.thread_id.as_deref(), Some("thread"));
        assert!(comment.is_resolved);
    }

    #[test]
    fn review_thread_metadata_rejects_invalid_full_database_ids() {
        for id in [
            serde_json::json!("not-an-id"),
            serde_json::json!("9223372036854775808"),
            serde_json::json!(42),
        ] {
            let thread = serde_json::json!({
                "id": "thread",
                "comments": {"nodes": [{"fullDatabaseId": id}]},
            });
            let error = append_review_thread_metadata(&thread, &mut HashMap::new())
                .expect_err("invalid ID must not silently lose thread metadata");
            assert!(error.to_string().contains("fullDatabaseId"));
        }
    }

    #[test]
    fn review_replies_beyond_first_thread_page_inherit_metadata() {
        let thread_map = (1..=100)
            .map(|id| (id, ("thread".to_string(), true)))
            .collect::<HashMap<_, _>>();
        let mut reply: ApiPullRequestReviewComment = serde_json::from_value(serde_json::json!({
            "id": 101,
            "path": "src/main.rs",
            "in_reply_to_id": 1,
            "user": {"login": "dev"},
        }))
        .expect("parse reply");

        apply_review_thread_metadata(&mut reply, &thread_map);

        assert_eq!(reply.thread_id.as_deref(), Some("thread"));
        assert!(reply.is_resolved);

        let mut direct_map = thread_map;
        direct_map.insert(reply.id, ("direct-thread".to_string(), false));
        apply_review_thread_metadata(&mut reply, &direct_map);
        assert_eq!(reply.thread_id.as_deref(), Some("direct-thread"));
        assert!(!reply.is_resolved);
    }
}
