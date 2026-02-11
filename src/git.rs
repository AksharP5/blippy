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
    repo_root_at(std::path::Path::new("."))
}

pub fn list_github_remotes() -> Result<Vec<RemoteInfo>> {
    list_github_remotes_at(std::path::Path::new("."))
}

fn repo_root_at(_path: &std::path::Path) -> Result<Option<std::path::PathBuf>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(_path)
        .args(["rev-parse", "--show-toplevel"])
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(error.into());
        }
    };

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(std::path::PathBuf::from(trimmed)))
}

pub fn list_github_remotes_at(_path: &std::path::Path) -> Result<Vec<RemoteInfo>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(_path)
        .args(["remote", "-v"])
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(Vec::new());
            }
            return Err(error.into());
        }
    };

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_remotes_output(&stdout))
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
    use super::{RemoteInfo, RepoSlug, parse_remote_url, parse_remotes_output};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_remote_url_handles_https() {
        let slug = parse_remote_url("https://github.com/acme/blippy.git").expect("slug");
        assert_eq!(slug.owner, "acme");
        assert_eq!(slug.repo, "blippy");
    }

    #[test]
    fn parse_remote_url_handles_ssh() {
        let slug = parse_remote_url("git@github.com:acme/blippy.git").expect("slug");
        assert_eq!(slug.owner, "acme");
        assert_eq!(slug.repo, "blippy");
    }

    #[test]
    fn parse_remote_url_handles_ssh_url() {
        let slug = parse_remote_url("ssh://git@github.com/acme/blippy.git").expect("slug");
        assert_eq!(slug.owner, "acme");
        assert_eq!(slug.repo, "blippy");
    }

    #[test]
    fn parse_remote_url_rejects_non_github() {
        assert!(parse_remote_url("https://gitlab.com/acme/blippy").is_none());
    }

    #[test]
    fn parse_remotes_output_collects_unique_remotes() {
        let output = "origin\thttps://github.com/acme/blippy.git (fetch)\norigin\thttps://github.com/acme/blippy.git (push)\nupstream\tgit@github.com:org/blippy.git (fetch)\n";
        let remotes = parse_remotes_output(output);

        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].slug.owner, "acme");
        assert_eq!(remotes[1].name, "upstream");
        assert_eq!(remotes[1].slug.owner, "org");
    }

    #[test]
    fn parse_remotes_output_filters_non_github() {
        let output = "origin\thttps://gitlab.com/acme/blippy.git (fetch)\n";
        let remotes = parse_remotes_output(output);
        assert!(remotes.is_empty());
    }

    #[test]
    fn parse_remotes_output_matches_expected_slug() {
        let output = "origin\tssh://git@github.com/acme/blippy.git (fetch)\n";
        let remotes = parse_remotes_output(output);

        let expected = RemoteInfo {
            name: "origin".to_string(),
            url: "ssh://git@github.com/acme/blippy.git".to_string(),
            slug: RepoSlug {
                owner: "acme".to_string(),
                repo: "blippy".to_string(),
            },
        };
        assert_eq!(remotes, vec![expected]);
    }

    #[test]
    fn repo_root_detects_git_repo() {
        let dir = unique_temp_dir("git-root");
        init_git_repo(&dir);

        let root = super::repo_root_at(&dir).expect("repo root");
        let expected = std::fs::canonicalize(&dir).expect("canonicalize dir");
        let actual = root.map(|path| std::fs::canonicalize(path).expect("canonicalize root"));
        assert_eq!(actual, Some(expected));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_github_remotes_reads_origin() {
        let dir = unique_temp_dir("git-remote");
        init_git_repo(&dir);
        run_git(
            &dir,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/acme/blippy.git",
            ],
        );

        let remotes = super::list_github_remotes_at(&dir).expect("list remotes");
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].slug.owner, "acme");

        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("blippy-git-{}-{}", label, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn init_git_repo(path: &Path) {
        run_git(path, &["init"]);
    }

    fn run_git(path: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success());
    }
}
