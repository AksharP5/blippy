use reqwest::header::{ETAG, IF_NONE_MATCH};

use super::*;

impl GitHubClient {
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

    pub async fn find_linked_pull_request(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Option<(i64, String)>> {
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
            let pull_number = match issue.get("number").and_then(serde_json::Value::as_i64) {
                Some(pull_number) => pull_number,
                None => continue,
            };
            if !html_url.contains("/pull/") {
                continue;
            }
            return Ok(Some((pull_number, html_url.to_string())));
        }

        Ok(None)
    }

    pub async fn find_linked_issue_for_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: i64,
    ) -> Result<Option<(i64, String)>> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/timeline",
            API_BASE, owner, repo, pull_number
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
            if issue.get("pull_request").is_some() {
                continue;
            }
            let html_url = match issue.get("html_url").and_then(serde_json::Value::as_str) {
                Some(html_url) => html_url,
                None => continue,
            };
            let issue_number = match issue.get("number").and_then(serde_json::Value::as_i64) {
                Some(issue_number) => issue_number,
                None => continue,
            };
            if !html_url.contains("/issues/") {
                continue;
            }
            return Ok(Some((issue_number, html_url.to_string())));
        }

        Ok(None)
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
            if batch.is_empty() {
                break;
            }
            labels.extend(batch);
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
            if batch.is_empty() {
                break;
            }
            for user in batch {
                assignees.push(user.login);
            }
            page += 1;
        }
        assignees.sort_by_key(|value| value.to_ascii_lowercase());
        assignees.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        Ok(assignees)
    }
}
