use super::*;

impl GitHubClient {
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
}
