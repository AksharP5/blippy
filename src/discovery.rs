use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRepo {
    pub path: PathBuf,
}

pub fn quick_scan(
    cwd: &Path,
    max_depth: usize,
    parent_depth: usize,
) -> Result<Vec<DiscoveredRepo>> {
    let mut roots = Vec::new();
    for (idx, ancestor) in cwd.ancestors().enumerate() {
        if idx > parent_depth {
            break;
        }
        roots.push(ancestor.to_path_buf());
    }

    let excluded = excluded_dirs();
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let repos = scan_repos_in_dir(&root, max_depth, &excluded)?;
        for repo in repos {
            let key = canonical_key(&repo.path);
            if seen.insert(key) {
                results.push(repo);
            }
        }
    }

    Ok(results)
}

pub fn full_scan(home: &Path) -> Result<Vec<DiscoveredRepo>> {
    let excluded = excluded_dirs();
    scan_repos_in_dir(home, usize::MAX, &excluded)
}

pub fn home_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return Some(PathBuf::from(home));
    }

    if let Ok(home) = std::env::var("USERPROFILE")
        && !home.is_empty()
    {
        return Some(PathBuf::from(home));
    }

    None
}

fn scan_repos_in_dir(
    root: &Path,
    max_depth: usize,
    excluded: &HashSet<&'static str>,
) -> Result<Vec<DiscoveredRepo>> {
    let mut repos = Vec::new();
    if !root.exists() {
        return Ok(repos);
    }

    let mut stack = Vec::new();
    stack.push((root.to_path_buf(), 0usize));

    while let Some((path, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }

        if is_excluded(&path, excluded) {
            continue;
        }

        if is_git_repo(&path) {
            repos.push(DiscoveredRepo { path: path.clone() });
            // Git knows its linked worktrees, so ordinary repository contents need no traversal.
            repos.extend(
                nested_worktrees(&path, max_depth - depth, excluded)
                    .into_iter()
                    .map(|path| DiscoveredRepo { path }),
            );
            continue;
        }

        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if !file_type.is_dir() {
                continue;
            }

            if depth == usize::MAX {
                continue;
            }

            stack.push((entry_path, depth + 1));
        }
    }

    Ok(repos)
}

fn nested_worktrees(
    repo: &Path,
    max_depth: usize,
    excluded: &HashSet<&'static str>,
) -> Vec<PathBuf> {
    if max_depth == 0 {
        return Vec::new();
    }
    let Ok(root) = std::fs::canonicalize(repo) else {
        return Vec::new();
    };
    let Ok(output) = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    output
        .stdout
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let worktree = record.strip_prefix(b"worktree ")?;
            #[cfg(unix)]
            let worktree = {
                use std::os::unix::ffi::OsStrExt;
                PathBuf::from(std::ffi::OsStr::from_bytes(worktree))
            };
            #[cfg(not(unix))]
            let worktree = PathBuf::from(std::str::from_utf8(worktree).ok()?);
            let worktree = std::fs::canonicalize(worktree).ok()?;
            let relative = worktree.strip_prefix(&root).ok()?;
            if relative.as_os_str().is_empty()
                || relative.components().count() > max_depth
                || relative
                    .ancestors()
                    .any(|ancestor| is_excluded(ancestor, excluded))
                || !is_git_repo(&worktree)
            {
                return None;
            }
            Some(repo.join(relative))
        })
        .collect()
}

pub fn is_git_repo(path: &Path) -> bool {
    let git_entry = path.join(".git");
    git_entry.is_dir() || git_entry.is_file()
}

fn excluded_dirs() -> HashSet<&'static str> {
    let names = [
        ".git",
        ".cache",
        "node_modules",
        "target",
        "vendor",
        "Library",
        "Applications",
        "AppData",
        "Program Files",
        "Program Files (x86)",
    ];
    names.iter().copied().collect()
}

fn is_excluded(path: &Path, excluded: &HashSet<&'static str>) -> bool {
    let name = match path.file_name() {
        Some(name) => name,
        None => return false,
    };
    let name = match name.to_str() {
        Some(name) => name,
        None => return false,
    };
    excluded.contains(name)
}

fn canonical_key(path: &Path) -> String {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical.to_string_lossy().to_string();
    }

    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::{DiscoveredRepo, excluded_dirs, full_scan, scan_repos_in_dir};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scan_repos_in_dir_finds_git_dirs() {
        let root = unique_temp_dir("scan");
        let repo_path = root.join("work").join("repo");
        fs::create_dir_all(repo_path.join(".git")).expect("create .git");

        let repos = scan_repos_in_dir(&root, 4, &excluded_dirs()).expect("scan");
        assert_eq!(repos, vec![DiscoveredRepo { path: repo_path }]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_repos_in_dir_finds_git_files_used_by_worktrees() {
        let root = unique_temp_dir("worktree");
        let repo_path = root.join("work").join("repo");
        fs::create_dir_all(&repo_path).expect("create repo");
        fs::write(repo_path.join(".git"), "gitdir: /tmp/example.git").expect("create .git");

        let repos = scan_repos_in_dir(&root, 4, &excluded_dirs()).expect("scan");
        assert_eq!(repos, vec![DiscoveredRepo { path: repo_path }]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scans_find_registered_worktrees_without_traversing_repository_contents() {
        let root = unique_temp_dir("nested-worktree");
        let repo = root.join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        run_git(&repo, &["init"]);
        run_git(
            &repo,
            &[
                "-c",
                "user.name=Blippy Tests",
                "-c",
                "user.email=blippy@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "Initial commit",
            ],
        );
        let worktree = repo.join("reviews").join("branches").join("feature one");
        let excluded = repo.join("target").join("ignored");
        let outside = root.join("outside");
        for path in [&worktree, &excluded, &outside] {
            run_git(
                &repo,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    path.to_str().expect("worktree path"),
                ],
            );
        }
        let embedded = repo.join("src").join("generated").join("unrelated");
        fs::create_dir_all(&embedded).expect("create embedded repo");
        run_git(&embedded, &["init"]);

        let repos = full_scan(&repo).expect("scan");
        assert_eq!(
            repos,
            vec![
                DiscoveredRepo { path: repo.clone() },
                DiscoveredRepo {
                    path: worktree.clone()
                }
            ],
        );
        let shallow = scan_repos_in_dir(&repo, 2, &excluded_dirs()).expect("shallow scan");
        assert_eq!(shallow, vec![DiscoveredRepo { path: repo.clone() }]);
        let bounded = scan_repos_in_dir(&repo, 3, &excluded_dirs()).expect("bounded scan");
        assert_eq!(bounded, repos);
        assert_eq!(
            full_scan(&worktree).expect("scan linked worktree"),
            vec![DiscoveredRepo { path: worktree }],
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_repos_in_dir_skips_excluded_dirs() {
        let root = unique_temp_dir("excluded");
        let repo_path = root.join("node_modules").join("repo");
        fs::create_dir_all(repo_path.join(".git")).expect("create .git");

        let repos = scan_repos_in_dir(&root, 4, &excluded_dirs()).expect("scan");
        assert!(repos.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_repos_in_dir_respects_depth_limit() {
        let root = unique_temp_dir("depth");
        let repo_path = root.join("a").join("b").join("c").join("repo");
        fs::create_dir_all(repo_path.join(".git")).expect("create .git");

        let shallow = scan_repos_in_dir(&root, 2, &excluded_dirs()).expect("scan");
        assert!(shallow.is_empty());

        let deep = scan_repos_in_dir(&root, 5, &excluded_dirs()).expect("scan");
        assert_eq!(deep, vec![DiscoveredRepo { path: repo_path }]);

        let _ = fs::remove_dir_all(&root);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("blippy-scan-{}-{}", label, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn run_git(path: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
