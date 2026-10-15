use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::discovery::is_git_repo;
use crate::git::{RemoteInfo, list_github_remotes_at};
use crate::store::{
    LocalRepoRow, delete_local_repos_at_path, list_local_repos, replace_local_repos_for_path,
};

pub fn index_repo_path(conn: &rusqlite::Connection, path: &Path) -> Result<usize> {
    let remotes = list_github_remotes_at(path)?;
    let rows = build_local_repo_rows(path, remotes);
    replace_local_repos_for_path(conn, path.to_string_lossy().as_ref(), &rows)?;
    Ok(rows.len())
}

#[derive(Debug, Default)]
pub struct RepoIndexStats {
    pub remotes: usize,
    pub failures: BTreeMap<PathBuf, String>,
}

pub fn index_repo_paths<'a>(
    conn: &rusqlite::Connection,
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<RepoIndexStats> {
    let mut stats = RepoIndexStats::default();
    for path in paths {
        match index_repo_path(conn, path) {
            Ok(remotes) => stats.remotes += remotes,
            Err(error) if error.downcast_ref::<rusqlite::Error>().is_some() => return Err(error),
            Err(error) => {
                stats.failures.insert(path.to_path_buf(), error.to_string());
            }
        }
    }
    Ok(stats)
}

pub fn prune_missing_local_repos(conn: &rusqlite::Connection) -> Result<usize> {
    let mut removed = 0usize;
    let mut checked_paths = HashSet::new();
    for repo in list_local_repos(conn)? {
        if !checked_paths.insert(repo.path.clone()) || is_git_repo(Path::new(&repo.path)) {
            continue;
        }
        delete_local_repos_at_path(conn, &repo.path)?;
        removed += 1;
    }
    Ok(removed)
}

fn build_local_repo_rows(path: &Path, remotes: Vec<RemoteInfo>) -> Vec<LocalRepoRow> {
    let now = now_epoch();
    remotes
        .into_iter()
        .map(|remote| LocalRepoRow {
            path: path.to_string_lossy().to_string(),
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
    use super::{build_local_repo_rows, index_repo_path, prune_missing_local_repos};
    use crate::git::{RemoteInfo, RepoSlug};
    use crate::store::{LocalRepoRow, list_local_repos, open_db_at, upsert_local_repo};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_local_repo_rows_includes_remote_metadata() {
        let remotes = vec![RemoteInfo {
            name: "origin".to_string(),
            url: "https://github.com/acme/blippy.git".to_string(),
            slug: RepoSlug {
                owner: "acme".to_string(),
                repo: "blippy".to_string(),
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
                "https://github.com/acme/blippy.git",
            ],
        );

        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");

        let inserted = index_repo_path(&conn, &repo_path).expect("index");
        assert_eq!(inserted, 1);

        let repos = list_local_repos(&conn).expect("list repos");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].path, repo_path.to_string_lossy().to_string());

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reindexing_replaces_removed_remotes() {
        let dir = unique_temp_dir("replace");
        let repo_path = dir.join("repo");
        fs::create_dir_all(&repo_path).expect("create repo");
        init_git_repo(&repo_path);
        run_git(
            &repo_path,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/acme/blippy.git",
            ],
        );
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");
        index_repo_path(&conn, &repo_path).expect("initial index");

        run_git(&repo_path, &["remote", "remove", "origin"]);
        index_repo_path(&conn, &repo_path).expect("reindex");

        assert!(list_local_repos(&conn).expect("list repos").is_empty());
        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_reindex_preserves_cached_remotes() {
        let dir = unique_temp_dir("failed-reindex");
        let repo_path = dir.join("repo");
        fs::create_dir_all(&repo_path).expect("create repo");
        fs::write(
            repo_path.join(".git"),
            "gitdir: /definitely/missing/blippy\n",
        )
        .expect("write broken git file");
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");
        let cached = LocalRepoRow {
            path: repo_path.to_string_lossy().to_string(),
            remote_name: "origin".to_string(),
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            url: "https://github.com/acme/blippy.git".to_string(),
            last_seen: None,
            last_scanned: None,
        };
        upsert_local_repo(&conn, &cached).expect("cache remote");

        assert!(index_repo_path(&conn, &repo_path).is_err());
        assert_eq!(list_local_repos(&conn).expect("list repos"), vec![cached]);

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn batch_index_continues_after_a_broken_repo() {
        let dir = unique_temp_dir("batch-index");
        let broken_path = dir.join("broken");
        fs::create_dir_all(&broken_path).expect("create broken repo");
        fs::write(
            broken_path.join(".git"),
            "gitdir: /definitely/missing/blippy\n",
        )
        .expect("write broken git file");
        let healthy_path = dir.join("healthy");
        fs::create_dir_all(&healthy_path).expect("create healthy repo");
        init_git_repo(&healthy_path);
        run_git(
            &healthy_path,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/acme/healthy.git",
            ],
        );
        let conn = open_db_at(&dir.join("blippy.db")).expect("open db");
        let cached = LocalRepoRow {
            path: broken_path.to_string_lossy().to_string(),
            remote_name: "origin".to_string(),
            owner: "acme".to_string(),
            repo: "broken".to_string(),
            url: "https://github.com/acme/broken.git".to_string(),
            last_seen: None,
            last_scanned: None,
        };
        upsert_local_repo(&conn, &cached).expect("cache remote");

        let missing_schema = rusqlite::Connection::open_in_memory().expect("empty db");
        assert!(super::index_repo_paths(&missing_schema, [healthy_path.as_path()]).is_err());

        let stats = super::index_repo_paths(&conn, [broken_path.as_path(), healthy_path.as_path()])
            .expect("index healthy repositories");

        assert_eq!(stats.remotes, 1);
        assert_eq!(stats.failures.len(), 1);
        assert!(stats.failures.contains_key(&broken_path));
        let repos = list_local_repos(&conn).expect("list repos");
        assert_eq!(repos.len(), 2);
        assert!(repos.contains(&cached));
        assert!(repos.iter().any(|repo| repo.repo == "healthy"));

        drop(conn);
        fs::remove_dir_all(&dir).expect("remove temp dir");
    }

    #[test]
    fn pruning_removes_deleted_repository_paths() {
        let dir = unique_temp_dir("prune");
        let repo_path = dir.join("repo");
        fs::create_dir_all(&repo_path).expect("create repo");
        init_git_repo(&repo_path);
        run_git(
            &repo_path,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/acme/blippy.git",
            ],
        );
        let db_path = dir.join("blippy.db");
        let conn = open_db_at(&db_path).expect("open db");
        index_repo_path(&conn, &repo_path).expect("index");
        fs::remove_dir_all(&repo_path).expect("remove repo");

        let removed = prune_missing_local_repos(&conn).expect("prune");

        assert_eq!(removed, 1);
        assert!(list_local_repos(&conn).expect("list repos").is_empty());
        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("blippy-index-{}-{}", label, nanos));
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
