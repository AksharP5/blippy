use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRepo {
    pub path: PathBuf,
}

pub fn quick_scan(
    _cwd: &Path,
    _max_depth: usize,
    _parent_depth: usize,
) -> Result<Vec<DiscoveredRepo>> {
    let mut roots = Vec::new();
    for (idx, ancestor) in _cwd.ancestors().enumerate() {
        if idx > _parent_depth {
            break;
        }
        roots.push(ancestor.to_path_buf());
    }

    let excluded = excluded_dirs();
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let repos = scan_repos_in_dir(&root, _max_depth, &excluded)?;
        for repo in repos {
            let key = canonical_key(&repo.path);
            if seen.insert(key) {
                results.push(repo);
            }
        }
    }

    Ok(results)
}

pub fn full_scan(_home: &Path) -> Result<Vec<DiscoveredRepo>> {
    let excluded = excluded_dirs();
    scan_repos_in_dir(_home, usize::MAX, &excluded)
}

pub fn home_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }

    None
}

fn scan_repos_in_dir(
    _root: &Path,
    _max_depth: usize,
    _excluded: &HashSet<&'static str>,
) -> Result<Vec<DiscoveredRepo>> {
    let mut repos = Vec::new();
    if !_root.exists() {
        return Ok(repos);
    }

    let mut stack = Vec::new();
    stack.push((_root.to_path_buf(), 0usize));

    while let Some((path, depth)) = stack.pop() {
        if depth > _max_depth {
            continue;
        }

        if is_excluded(&path, _excluded) {
            continue;
        }

        let git_dir = path.join(".git");
        if git_dir.is_dir() {
            repos.push(DiscoveredRepo { path });
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
    use super::{DiscoveredRepo, excluded_dirs, scan_repos_in_dir};
    use std::fs;
    use std::path::PathBuf;
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
}
