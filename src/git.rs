use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSlug {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    pub slug: RepoSlug,
}

pub fn parse_remote_url(_url: &str) -> Option<RepoSlug> {
    let url = _url.trim();
    if url.is_empty() {
        return None;
    }

    let cleaned = url.strip_suffix(".git").unwrap_or(url);

    if let Some(rest) = cleaned.strip_prefix("git@github.com:") {
        return split_owner_repo(rest);
    }

    if let Some(rest) = cleaned.strip_prefix("ssh://git@github.com/") {
        return split_owner_repo(rest);
    }

    if let Some(rest) = cleaned.strip_prefix("https://github.com/") {
        return split_owner_repo(rest);
    }

    if let Some(rest) = cleaned.strip_prefix("http://github.com/") {
        return split_owner_repo(rest);
    }

    None
}

pub fn parse_remotes_output(_output: &str) -> Vec<RemoteInfo> {
    let mut remotes = Vec::new();
    for line in _output.lines() {
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(value) => value,
            None => continue,
        };
        let url = match parts.next() {
            Some(value) => value,
            None => continue,
        };
        let kind = parts.next().unwrap_or("");
        if kind != "(fetch)" {
            continue;
        }
        let slug = match parse_remote_url(url) {
            Some(slug) => slug,
            None => continue,
        };

        remotes.push(RemoteInfo {
            name: name.to_string(),
            url: url.to_string(),
            slug,
        });
    }

    remotes
}

pub fn repo_root() -> Result<Option<std::path::PathBuf>> {
    todo!("Detect git repo root");
}

pub fn list_github_remotes() -> Result<Vec<RemoteInfo>> {
    todo!("List GitHub remotes for current repo");
}

fn split_owner_repo(input: &str) -> Option<RepoSlug> {
    let mut parts = input.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some(RepoSlug {
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_remote_url, parse_remotes_output, RemoteInfo, RepoSlug};

    #[test]
    fn parse_remote_url_handles_https() {
        let slug = parse_remote_url("https://github.com/acme/glyph.git").expect("slug");
        assert_eq!(slug.owner, "acme");
        assert_eq!(slug.repo, "glyph");
    }

    #[test]
    fn parse_remote_url_handles_ssh() {
        let slug = parse_remote_url("git@github.com:acme/glyph.git").expect("slug");
        assert_eq!(slug.owner, "acme");
        assert_eq!(slug.repo, "glyph");
    }

    #[test]
    fn parse_remote_url_handles_ssh_url() {
        let slug = parse_remote_url("ssh://git@github.com/acme/glyph.git").expect("slug");
        assert_eq!(slug.owner, "acme");
        assert_eq!(slug.repo, "glyph");
    }

    #[test]
    fn parse_remote_url_rejects_non_github() {
        assert!(parse_remote_url("https://gitlab.com/acme/glyph").is_none());
    }

    #[test]
    fn parse_remotes_output_collects_unique_remotes() {
        let output = "origin\thttps://github.com/acme/glyph.git (fetch)\norigin\thttps://github.com/acme/glyph.git (push)\nupstream\tgit@github.com:org/glyph.git (fetch)\n";
        let remotes = parse_remotes_output(output);

        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].slug.owner, "acme");
        assert_eq!(remotes[1].name, "upstream");
        assert_eq!(remotes[1].slug.owner, "org");
    }

    #[test]
    fn parse_remotes_output_filters_non_github() {
        let output = "origin\thttps://gitlab.com/acme/glyph.git (fetch)\n";
        let remotes = parse_remotes_output(output);
        assert!(remotes.is_empty());
    }

    #[test]
    fn parse_remotes_output_matches_expected_slug() {
        let output = "origin\tssh://git@github.com/acme/glyph.git (fetch)\n";
        let remotes = parse_remotes_output(output);

        let expected = RemoteInfo {
            name: "origin".to_string(),
            url: "ssh://git@github.com/acme/glyph.git".to_string(),
            slug: RepoSlug {
                owner: "acme".to_string(),
                repo: "glyph".to_string(),
            },
        };
        assert_eq!(remotes, vec![expected]);
    }
}
