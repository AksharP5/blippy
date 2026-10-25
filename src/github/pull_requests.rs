use std::collections::{HashMap, HashSet};

use super::*;

impl GitHubClient {
    pub async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<(String, Vec<ApiPullRequestFile>)> {
        let commits = self.pull_request_commits(owner, repo, pull_number).await?;
        let mut page = 1;
        let mut files = Vec::new();
        loop {
            let url = format!(
                "{}/repos/{}/{}/pulls/{}/files",
                self.api_base, owner, repo, pull_number
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
        if self.pull_request_commits(owner, repo, pull_number).await? != commits {
            return Err(anyhow!(
                "Pull request changed while loading files; refresh to try again"
            ));
        }
        Ok((commits.head.sha, files))
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

    async fn pull_request_commits(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<ApiPullRequestSummary> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.api_base, owner, repo, pull_number
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json::<ApiPullRequestSummary>().await?)
    }

    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
        head_sha: Option<&str>,
    ) -> Result<()> {
        let repo_details = self
            .client
            .get(format!("{}/repos/{}/{}", self.api_base, owner, repo))
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
            self.api_base, owner, repo, pull_number
        );
        let mut last_error = String::new();
        for merge_method in merge_methods {
            let mut payload = serde_json::json!({ "merge_method": merge_method });
            if let Some(head_sha) = head_sha {
                payload["sha"] = serde_json::json!(head_sha);
            }
            let response = self
                .client
                .put(merge_url.as_str())
                .bearer_auth(&self.token)
                .json(&payload)
                .send()
                .await?;
            let status = response.status();
            if status == reqwest::StatusCode::CONFLICT && head_sha.is_some() {
                return Err(anyhow!(
                    "Pull request head changed; refresh the diff and try merging again"
                ));
            }
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
                self.api_base, owner, repo, pull_number
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
            self.api_base, owner, repo, pull_number
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
            self.api_base, owner, repo, comment_id
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
            self.api_base, owner, repo, comment_id
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
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    fn serve_responses(
        responses: Vec<(&'static str, &'static str, serde_json::Value)>,
    ) -> (GitHubClient, thread::JoinHandle<Vec<serde_json::Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock GitHub");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let api_base = format!("http://{}", listener.local_addr().expect("local address"));
        let server = thread::spawn(move || {
            let mut bodies = Vec::new();
            for (request_line, status, response) in responses {
                let deadline = Instant::now() + Duration::from_secs(5);
                let stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "missing request: {request_line}");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept request: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("read timeout");
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("request line");
                assert_eq!(line.trim_end(), format!("{request_line} HTTP/1.1"));
                let mut length = 0;
                loop {
                    line.clear();
                    assert!(reader.read_line(&mut line).expect("header") > 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().expect("content length");
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).expect("request body");
                bodies.push(if body.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_slice(&body).expect("JSON body")
                });
                let body = response.to_string();
                write!(
                    reader.get_mut(),
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(), body
                ).expect("mock response");
            }
            bodies
        });
        let client = GitHubClient {
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("test client"),
            token: "test-token".to_string(),
            api_base,
        };
        (client, server)
    }

    #[tokio::test]
    async fn merge_request_pins_the_displayed_head() {
        let mut app = crate::app::App::new(crate::config::Config::default());
        app.set_current_issue(1, 7);
        app.set_pull_request_files(1, "head-a".to_string(), Vec::new());
        for (server_head, status, response) in [
            ("head-a", "200 OK", serde_json::json!({"merged": true})),
            (
                "head-b",
                "409 Conflict",
                serde_json::json!({"message": "Head branch was modified"}),
            ),
        ] {
            let (client, server) = serve_responses(vec![
                (
                    "GET /repos/acme/blippy",
                    "200 OK",
                    serde_json::json!({
                        "allow_merge_commit": true,
                        "allow_squash_merge": true,
                        "allow_rebase_merge": true,
                    }),
                ),
                ("PUT /repos/acme/blippy/pulls/7/merge", status, response),
            ]);

            let result = client
                .merge_pull_request("acme", "blippy", 7, app.pull_request_head_sha())
                .await;
            let bodies = server
                .join()
                .expect("one merge attempt, without a head fetch");
            println!(
                "Displayed head=head-a, server head={server_head}, result={result:?}, requests={bodies:?}"
            );
            assert_eq!(
                bodies,
                vec![
                    serde_json::Value::Null,
                    serde_json::json!({"merge_method": "merge", "sha": "head-a"}),
                ]
            );
            if server_head == "head-a" {
                result.expect("merge unchanged head");
            } else {
                assert_eq!(
                    result
                        .expect_err("reject changed head without trying another method")
                        .to_string(),
                    "Pull request head changed; refresh the diff and try merging again"
                );
            }
        }
    }

    #[tokio::test]
    async fn merge_method_fallback_preserves_optional_head() {
        for head_sha in [Some("head-a"), None] {
            let (client, server) = serve_responses(vec![
                (
                    "GET /repos/acme/blippy",
                    "200 OK",
                    if head_sha.is_some() {
                        serde_json::json!({
                            "allow_merge_commit": true,
                            "allow_squash_merge": true,
                        })
                    } else {
                        serde_json::json!({})
                    },
                ),
                (
                    "PUT /repos/acme/blippy/pulls/7/merge",
                    "405 Method Not Allowed",
                    serde_json::json!({"message": "Merge commits are not allowed"}),
                ),
                (
                    "PUT /repos/acme/blippy/pulls/7/merge",
                    "200 OK",
                    serde_json::json!({"merged": true}),
                ),
            ]);

            client
                .merge_pull_request("acme", "blippy", 7, head_sha)
                .await
                .expect("fallback to squash merge");
            let bodies = server.join().expect("settings and two merge attempts");
            println!("Fallback head={head_sha:?}, requests={bodies:?}");
            let expected = if head_sha.is_some() {
                vec![
                    serde_json::Value::Null,
                    serde_json::json!({"merge_method": "merge", "sha": "head-a"}),
                    serde_json::json!({"merge_method": "squash", "sha": "head-a"}),
                ]
            } else {
                vec![
                    serde_json::Value::Null,
                    serde_json::json!({"merge_method": "merge"}),
                    serde_json::json!({"merge_method": "squash"}),
                ]
            };
            assert_eq!(bodies, expected);
        }
    }

    #[tokio::test]
    async fn files_snapshot_requires_the_same_commits_before_and_after_all_pages() {
        let file = serde_json::json!({
            "filename": "src/main.rs",
            "status": "modified",
            "additions": 1,
            "deletions": 1,
            "patch": "@@ -1 +1 @@\n-old\n+new",
        });
        let first_page = (0..100)
            .map(|index| {
                let mut file = file.clone();
                file["filename"] = serde_json::json!(format!("src/file-{index:03}.rs"));
                file
            })
            .collect::<Vec<_>>();
        for (final_head, final_base) in [
            ("head-a", "base-a"),
            ("head-b", "base-a"),
            ("head-a", "base-b"),
        ] {
            let (client, server) = serve_responses(vec![
                (
                    "GET /repos/acme/blippy/pulls/7",
                    "200 OK",
                    serde_json::json!({
                        "head": {"sha": "head-a"},
                        "base": {"sha": "base-a"},
                    }),
                ),
                (
                    "GET /repos/acme/blippy/pulls/7/files?per_page=100&page=1",
                    "200 OK",
                    serde_json::json!(first_page),
                ),
                (
                    "GET /repos/acme/blippy/pulls/7/files?per_page=100&page=2",
                    "200 OK",
                    if final_base == "base-a" {
                        serde_json::json!([file.clone()])
                    } else {
                        // Retargeting to a base containing the first 100 files leaves one page.
                        serde_json::json!([])
                    },
                ),
                (
                    "GET /repos/acme/blippy/pulls/7",
                    "200 OK",
                    serde_json::json!({
                        "head": {"sha": final_head},
                        "base": {"sha": final_base},
                    }),
                ),
            ]);

            let result = client.list_pull_request_files("acme", "blippy", 7).await;
            server
                .join()
                .expect("all pages fetched between commit checks");
            if final_head == "head-a" && final_base == "base-a" {
                let (head_sha, files) = result.expect("stable snapshot");
                assert_eq!(head_sha, "head-a");
                assert_eq!(files.len(), 101);
            } else {
                let error = match result {
                    Err(error) => error,
                    Ok((_, files)) => panic!(
                        "accepted {} files after head={final_head}, base={final_base}",
                        files.len()
                    ),
                };
                assert_eq!(
                    error.to_string(),
                    "Pull request changed while loading files; refresh to try again"
                );
            }
        }
    }

    #[tokio::test]
    async fn review_request_keeps_editor_snapshot_after_files_are_replaced() {
        use crate::app::{App, PullRequestFile, PullRequestReviewFocus, ReviewSide, View};
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        for (side, multiline) in [
            (ReviewSide::Left, false),
            (ReviewSide::Right, false),
            (ReviewSide::Left, true),
            (ReviewSide::Right, true),
        ] {
            let mut app = App::new(crate::config::Config::default());
            app.set_current_issue(1, 7);
            app.set_view(View::PullRequestFiles);
            let conn = rusqlite::Connection::open_in_memory().expect("conn");
            let (event_tx, event_rx) = std::sync::mpsc::channel();
            let snapshot = |head: &str, patch: &str| crate::AppEvent::PullRequestFilesUpdated {
                issue_id: 1,
                head_sha: head.to_string(),
                files: vec![PullRequestFile {
                    filename: "src/main.rs".to_string(),
                    status: "modified".to_string(),
                    additions: 0,
                    deletions: 0,
                    patch: Some(patch.to_string()),
                }],
                pull_request_id: None,
                viewed_files: HashSet::new(),
                view_state_error: None,
            };
            event_tx
                .send(snapshot("head-a", "@@ -10,2 +20,2 @@\n first\n second"))
                .expect("snapshot A");
            crate::main_events::handle_events(&mut app, &conn, &event_rx).expect("display A");
            app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
            app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
            if side == ReviewSide::Left {
                app.on_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
            }
            if multiline {
                app.on_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT));
                app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
            }
            let target = app.selected_pull_request_review_target().expect("A target");
            let start = if side == ReviewSide::Left { 10 } else { 20 };
            let end = start + i64::from(multiline);
            assert_eq!(target.commit_id, "head-a");
            assert_eq!(target.line, end);
            assert_eq!(target.side, side);
            assert_eq!(target.start_line, multiline.then_some(start));
            app.open_pull_request_review_comment_editor(View::PullRequestFiles, target.clone());

            event_tx
                .send(snapshot("head-b", "@@ -110,2 +120,2 @@\n first\n second"))
                .expect("snapshot B");
            crate::main_events::handle_events(&mut app, &conn, &event_rx).expect("display B");
            assert_eq!(app.view(), View::CommentEditor);
            assert!(
                app.pull_request_files()[0]
                    .patch
                    .as_deref()
                    .unwrap()
                    .contains("120")
            );
            let pending = app.take_pending_review_target().expect("editor target");
            assert_eq!(pending, target);

            let (client, server) = serve_responses(vec![(
                "POST /repos/acme/blippy/pulls/7/comments",
                "200 OK",
                serde_json::json!({}),
            )]);
            client
                .create_pull_request_review_comment(
                    "acme",
                    "blippy",
                    7,
                    &pending.commit_id,
                    &pending.path,
                    pending.line,
                    pending.side.as_api_side(),
                    pending.start_line,
                    pending.start_side.map(ReviewSide::as_api_side),
                    "Review A",
                )
                .await
                .expect("submit captured snapshot");
            let bodies = server.join().expect("only a comment POST, no head fetch");
            let mut expected = serde_json::json!({
                "body": "Review A",
                "commit_id": "head-a",
                "path": "src/main.rs",
                "line": end,
                "side": side.as_api_side(),
            });
            if multiline {
                expected["start_line"] = serde_json::json!(start);
                expected["start_side"] = serde_json::json!(side.as_api_side());
            }
            assert_eq!(bodies, vec![expected]);
        }
    }

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
