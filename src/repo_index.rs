use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::git::{RemoteInfo, list_github_remotes_at};
use crate::store::{LocalRepoRow, upsert_local_repo};

pub fn index_repo_path(_conn: &rusqlite::Connection, _path: &Path) -> Result<usize> {
    let remotes = list_github_remotes_at(_path)?;
    let rows = build_local_repo_rows(_path, remotes);
    for row in &rows {
        upsert_local_repo(_conn, row)?;
    }
    Ok(rows.len())
}

fn build_local_repo_rows(_path: &Path, _remotes: Vec<RemoteInfo>) -> Vec<LocalRepoRow> {
    let now = now_epoch();
    _remotes
        .into_iter()
        .map(|remote| LocalRepoRow {
            path: _path.to_string_lossy().to_string(),
            remote_name: remote.name,
            owner: remote.slug.owner,
            repo: remote.slug.repo,
            url: remote.url,
            last_seen: Some(now.clone()),
            last_scanned: Some(now.clone()),
        })
        .collect()
}

fn now_epoch() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.to_string()
}

#[cfg(test)]
mod tests {
    use super::{build_local_repo_rows, index_repo_path};
    use crate::git::{RemoteInfo, RepoSlug};
    use crate::store::{list_local_repos, open_db_at};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_local_repo_rows_includes_remote_metadata() {
        let remotes = vec![RemoteInfo {
            name: "origin".to_string(),
            url: "https://github.com/acme/glyph.git".to_string(),
            slug: RepoSlug {
                owner: "acme".to_string(),
                repo: "glyph".to_string(),
            },
        }];
        let path = Path::new("/tmp/repo");
        let rows = build_local_repo_rows(path, remotes);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "/tmp/repo");
        assert_eq!(rows[0].remote_name, "origin");
        assert_eq!(rows[0].owner, "acme");
    }

    #[test]
    fn index_repo_path_inserts_local_repo() {
        let dir = unique_temp_dir("index");
        let repo_path = dir.join("repo");
        fs::create_dir_all(repo_path.join(".git")).expect("create .git");
        init_git_repo(&repo_path);
        run_git(
            &repo_path,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/acme/glyph.git",
            ],
        );

        let db_path = dir.join("glyph.db");
        let conn = open_db_at(&db_path).expect("open db");

        let inserted = index_repo_path(&conn, &repo_path).expect("index");
        assert_eq!(inserted, 1);

        let repos = list_local_repos(&conn).expect("list repos");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].path, repo_path.to_string_lossy().to_string());

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("glyph-index-{}-{}", label, nanos));
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
