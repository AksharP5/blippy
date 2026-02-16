use super::*;

impl GitHubClient {
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
}
