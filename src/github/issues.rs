use reqwest::header::{ETAG, IF_NONE_MATCH};
use std::collections::HashSet;

use super::*;

impl GitHubClient {
    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
    ) -> Result<ApiIssue> {
        let url = format!("{}/repos/{}/{}/issues", API_BASE, owner, repo);
        let mut payload = serde_json::json!({ "title": title });
        if let Some(body) = body {
            payload["body"] = serde_json::Value::String(body.to_string());
        }

        let response = self
            .client
            .post(url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json::<ApiIssue>().await?)
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
        let mut request = self.client.get(url).bearer_auth(&self.token).query(&[
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

    pub async fn find_linked_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Vec<(i64, String)>> {
        let resolved = self.get_repo(owner, repo).await?;
        let owner = resolved.owner.login.as_str();
        let repo = resolved.name.as_str();
        let mut linked = Vec::new();
        let mut seen = HashSet::new();
        let mut page = 1u32;
        loop {
            let url = format!(
                "{}/repos/{}/{}/issues/{}/timeline",
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
            let events = response.json::<Vec<serde_json::Value>>().await?;
            if events.is_empty() {
                break;
            }

            for event in &events {
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
                let pull_number = match issue.get("number").and_then(serde_json::Value::as_i64) {
                    Some(pull_number) => pull_number,
                    None => continue,
                };
                if !item_url_matches_repo(html_url, owner, repo, "pull", pull_number)
                    || !seen.insert(pull_number)
                {
                    continue;
                }
                linked.push((pull_number, html_url.to_string()));
            }

            if events.len() < 100 {
                break;
            }
            page += 1;
        }

        Ok(linked)
    }

    pub async fn find_linked_issues_for_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<Vec<(i64, String)>> {
        let resolved = self.get_repo(owner, repo).await?;
        let owner = resolved.owner.login.as_str();
        let repo = resolved.name.as_str();
        discover_linked_issues(
            async |linked, seen| {
                let query = r#"
                    query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
                      repository(owner: $owner, name: $repo) {
                        pullRequest(number: $number) {
                          closingIssuesReferences(first: 100, after: $cursor) {
                            pageInfo { hasNextPage endCursor }
                            nodes { number url }
                          }
                        }
                      }
                    }
                "#;
                let mut cursor: Option<String> = None;
                let mut cursors = HashSet::new();
                loop {
                    let response = self
                        .graphql(
                            query,
                            serde_json::json!({
                                "owner": owner,
                                "repo": repo,
                                "number": pull_number,
                                "cursor": cursor,
                            }),
                        )
                        .await?;
                    let issues =
                        &response["data"]["repository"]["pullRequest"]["closingIssuesReferences"];
                    if let Some(nodes) = issues["nodes"].as_array() {
                        for issue in nodes {
                            append_linked_issue(owner, repo, issue, linked, seen);
                        }
                    }
                    cursor = next_graphql_cursor(&issues["pageInfo"], &mut cursors)?;
                    if cursor.is_none() {
                        break;
                    }
                }
                Ok(())
            },
            async |linked, seen| {
                let mut page = 1u32;
                loop {
                    let url = format!(
                        "{}/repos/{}/{}/issues/{}/timeline",
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
                    let events = response.json::<Vec<serde_json::Value>>().await?;
                    if events.is_empty() {
                        break;
                    }

                    for event in &events {
                        let issue = match event.get("source").and_then(|value| value.get("issue")) {
                            Some(issue) => issue,
                            None => continue,
                        };
                        append_linked_issue(owner, repo, issue, linked, seen);
                    }

                    if events.len() < 100 {
                        break;
                    }
                    page += 1;
                }
                Ok(())
            },
        )
        .await
    }

    pub async fn close_issue(&self, owner: &str, repo: &str, issue_number: i64) -> Result<()> {
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

    pub async fn reopen_issue(&self, owner: &str, repo: &str, issue_number: i64) -> Result<()> {
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

    pub async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<ApiLabel>> {
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
            let is_last_page = batch.len() < 100;
            labels.extend(batch);
            if is_last_page {
                break;
            }
            page += 1;
        }
        Ok(labels)
    }

    pub async fn list_assignees(&self, owner: &str, repo: &str) -> Result<Vec<String>> {
        let mut page = 1u32;
        let mut assignees = Vec::new();
        loop {
            let url = format!("{}/repos/{}/{}/assignees", API_BASE, owner, repo);
            let response = self
                .client
                .get(url)
                .bearer_auth(&self.token)
                .query(&[("per_page", "100"), ("page", &page.to_string())])
                .send()
                .await?
                .error_for_status()?;
            let batch = response.json::<Vec<ApiUser>>().await?;
            let is_last_page = batch.len() < 100;
            for user in batch {
                assignees.push(user.login);
            }
            if is_last_page {
                break;
            }
            page += 1;
        }
        assignees.sort_by_key(|value| value.to_ascii_lowercase());
        assignees.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        Ok(assignees)
    }
}

async fn discover_linked_issues(
    closing_issues: impl AsyncFnOnce(&mut Vec<(i64, String)>, &mut HashSet<i64>) -> Result<()>,
    timeline: impl AsyncFnOnce(&mut Vec<(i64, String)>, &mut HashSet<i64>) -> Result<()>,
) -> Result<Vec<(i64, String)>> {
    let mut linked = Vec::new();
    let mut seen = HashSet::new();
    let closing_result = closing_issues(&mut linked, &mut seen).await;
    let timeline_result = timeline(&mut linked, &mut seen).await;

    match (closing_result, timeline_result) {
        (Err(closing_error), Err(timeline_error)) => Err(anyhow!(
            "Closing-issue lookup failed: {closing_error:#}; timeline lookup failed: {timeline_error:#}"
        )),
        (Err(error), _) if linked.is_empty() => Err(error.context("Closing-issue lookup failed")),
        (_, Err(error)) if linked.is_empty() => Err(error.context("Timeline lookup failed")),
        _ => Ok(linked),
    }
}

fn append_linked_issue(
    owner: &str,
    repo: &str,
    issue: &serde_json::Value,
    linked: &mut Vec<(i64, String)>,
    seen: &mut HashSet<i64>,
) {
    if issue.get("pull_request").is_some() {
        return;
    }
    let Some(url) = issue
        .get("html_url")
        .or_else(|| issue.get("url"))
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let Some(number) = issue.get("number").and_then(serde_json::Value::as_i64) else {
        return;
    };
    if !item_url_matches_repo(url, owner, repo, "issues", number) || !seen.insert(number) {
        return;
    }
    linked.push((number, url.to_string()));
}

fn item_url_matches_repo(
    html_url: &str,
    owner: &str,
    repo: &str,
    route: &str,
    number: i64,
) -> bool {
    let expected = format!("https://github.com/{}/{}/{}/{}", owner, repo, route, number);
    html_url
        .trim_end_matches('/')
        .eq_ignore_ascii_case(&expected)
}

#[cfg(test)]
mod tests {
    use super::{append_linked_issue, discover_linked_issues, item_url_matches_repo};
    use std::collections::HashSet;

    #[tokio::test]
    async fn graphql_failure_still_discovers_timeline_links() {
        let linked = discover_linked_issues(
            async |_, _| Err(anyhow::anyhow!("GraphQL unavailable")),
            async |linked, seen| {
                append_linked_issue(
                    "acme",
                    "blippy",
                    &serde_json::json!({
                        "number": 21,
                        "html_url": "https://github.com/acme/blippy/issues/21",
                    }),
                    linked,
                    seen,
                );
                Ok(())
            },
        )
        .await
        .expect("timeline links remain usable");

        assert_eq!(
            linked,
            vec![(21, "https://github.com/acme/blippy/issues/21".to_string())]
        );
    }

    #[tokio::test]
    async fn timeline_failure_keeps_unique_closing_links() {
        let issue = serde_json::json!({
            "number": 20,
            "url": "https://github.com/acme/blippy/issues/20",
        });
        let linked = discover_linked_issues(
            async |linked, seen| {
                append_linked_issue("acme", "blippy", &issue, linked, seen);
                Ok(())
            },
            async |linked, seen| {
                append_linked_issue("acme", "blippy", &issue, linked, seen);
                Err(anyhow::anyhow!("timeline unavailable"))
            },
        )
        .await
        .expect("closing links remain usable");

        assert_eq!(
            linked,
            vec![(20, "https://github.com/acme/blippy/issues/20".to_string())]
        );
    }

    #[tokio::test]
    async fn failed_link_sources_are_reported_when_no_links_are_available() {
        for (closing_error, timeline_error, partial_link) in [
            (Some("GraphQL unavailable"), None, false),
            (None, Some("timeline unavailable"), false),
            (
                Some("GraphQL unavailable"),
                Some("timeline unavailable"),
                false,
            ),
            (
                Some("GraphQL unavailable"),
                Some("timeline unavailable"),
                true,
            ),
        ] {
            let error = discover_linked_issues(
                async |linked, _| {
                    if partial_link {
                        linked.push((20, "https://github.com/acme/blippy/issues/20".to_string()));
                    }
                    match closing_error {
                        Some(error) => Err(anyhow::anyhow!(error)),
                        None => Ok(()),
                    }
                },
                async |_, _| match timeline_error {
                    Some(error) => Err(anyhow::anyhow!(error)),
                    None => Ok(()),
                },
            )
            .await
            .expect_err("failed discovery must not look like an empty result");
            let message = format!("{error:#}");
            for expected in [closing_error, timeline_error].into_iter().flatten() {
                assert!(message.contains(expected), "{message}");
            }
        }

        assert!(
            discover_linked_issues(async |_, _| Ok(()), async |_, _| Ok(()))
                .await
                .expect("successful empty discovery")
                .is_empty()
        );
    }

    #[test]
    fn closing_and_incoming_issue_links_preserve_unique_current_repo_targets() {
        let candidates = [
            serde_json::json!({"number": 20, "url": "https://github.com/acme/blippy/issues/20"}),
            serde_json::json!({"number": 21, "html_url": "https://github.com/acme/blippy/issues/21"}),
            serde_json::json!({"number": 21, "url": "https://github.com/acme/blippy/issues/21"}),
            serde_json::json!({"number": 22, "url": "https://github.com/other/blippy/issues/22"}),
        ];
        let mut linked = Vec::new();
        let mut seen = HashSet::new();
        for issue in &candidates {
            append_linked_issue("acme", "blippy", issue, &mut linked, &mut seen);
        }
        assert_eq!(
            linked,
            vec![
                (20, "https://github.com/acme/blippy/issues/20".to_string()),
                (21, "https://github.com/acme/blippy/issues/21".to_string()),
            ]
        );
    }

    #[test]
    fn unrelated_timeline_items_do_not_create_issue_links() {
        let mut linked = Vec::new();
        let mut seen = HashSet::new();
        for issue in [
            serde_json::Value::Null,
            serde_json::json!({
                "number": 20,
                "html_url": "https://github.com/acme/blippy/pull/20",
                "pull_request": {"url": "https://api.github.com/repos/acme/blippy/pulls/20"},
            }),
        ] {
            append_linked_issue("acme", "blippy", &issue, &mut linked, &mut seen);
        }
        assert!(linked.is_empty());
    }

    #[test]
    fn linked_item_url_must_match_the_current_repo() {
        assert!(item_url_matches_repo(
            "https://github.com/acme/blippy/pull/20",
            "Acme",
            "Blippy",
            "pull",
            20,
        ));
        assert!(!item_url_matches_repo(
            "https://github.com/other/blippy/pull/20",
            "acme",
            "blippy",
            "pull",
            20,
        ));
    }
}
