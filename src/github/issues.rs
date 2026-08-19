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
        let mut linked = Vec::new();
        let mut seen = HashSet::new();
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
                if !item_url_matches_repo(html_url, owner, repo, "issues", issue_number)
                    || !seen.insert(issue_number)
                {
                    continue;
                }
                linked.push((issue_number, html_url.to_string()));
            }

            if events.len() < 100 {
                break;
            }
            page += 1;
        }

        Ok(linked)
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
    use super::item_url_matches_repo;

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
