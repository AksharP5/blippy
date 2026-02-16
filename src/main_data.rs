use super::*;

pub(super) fn initialize_app(app: &mut App, conn: &rusqlite::Connection) -> Result<()> {
    let repo_root = crate::git::repo_root()?;
    if let Some(root) = repo_root {
        let remotes = list_github_remotes_at(&root)?;
        if remotes.is_empty() {
            app.set_status("No GitHub remotes found.");
            app.set_repos(load_repos(conn)?);
            app.set_view(View::RepoPicker);
            return Ok(());
        }

        if remotes.len() == 1 {
            let remote = &remotes[0];
            let root_path = root.to_string_lossy().to_string();
            load_issues_for_slug(
                app,
                conn,
                &remote.slug.owner,
                &remote.slug.repo,
                Some(root_path.as_str()),
            )?;
            app.set_view(View::Issues);
            app.request_sync();
            return Ok(());
        }

        app.set_remotes(remotes);
        app.set_view(View::RemoteChooser);
        return Ok(());
    }

    app.set_repos(load_repos(conn)?);
    app.set_view(View::RepoPicker);
    Ok(())
}

pub(super) fn load_issues_for_slug(
    app: &mut App,
    conn: &rusqlite::Connection,
    owner: &str,
    repo: &str,
    repo_path: Option<&str>,
) -> Result<()> {
    app.set_current_repo_with_path(owner, repo, repo_path);
    let repo_row = get_repo_by_slug(conn, owner, repo)?;
    let repo_row = match repo_row {
        Some(repo_row) => repo_row,
        None => {
            app.set_issues(Vec::new());
            app.set_status("No cached issues yet. Press r to sync.".to_string());
            app.request_sync();
            return Ok(());
        }
    };
    let issues = list_issues(conn, repo_row.id)?;
    app.set_issues(issues);
    app.set_status(format!("{}/{}", owner, repo));
    Ok(())
}

pub(super) fn load_repos(conn: &rusqlite::Connection) -> Result<Vec<crate::store::LocalRepoRow>> {
    list_local_repos(conn)
}

pub(super) fn maybe_start_scan(app: &App, event_tx: Sender<AppEvent>) -> Result<()> {
    if app.view() != View::RepoPicker {
        return Ok(());
    }

    let mode = if app.repos().is_empty() {
        ScanMode::QuickAndFull
    } else {
        ScanMode::QuickOnly
    };

    start_scan(event_tx, mode)
}

pub(super) fn maybe_start_rescan(app: &mut App, event_tx: Sender<AppEvent>) -> Result<()> {
    if !app.take_rescan_request() {
        return Ok(());
    }

    start_scan(event_tx, ScanMode::FullOnly)
}

pub(super) fn start_scan(event_tx: Sender<AppEvent>, mode: ScanMode) -> Result<()> {
    let cwd = env::current_dir()?;
    let home = home_dir().unwrap_or(cwd.clone());
    thread::spawn(move || {
        let conn = match crate::store::open_db() {
            Ok(conn) => conn,
            Err(_) => return,
        };

        if matches!(mode, ScanMode::QuickOnly | ScanMode::QuickAndFull) {
            let quick = quick_scan(&cwd, 4, 2).unwrap_or_default();
            for repo in &quick {
                let _ = index_repo_path(&conn, &repo.path);
            }
            let _ = event_tx.send(AppEvent::ReposUpdated);
        }

        if matches!(mode, ScanMode::FullOnly | ScanMode::QuickAndFull) {
            let full = crate::discovery::full_scan(&home).unwrap_or_default();
            for repo in &full {
                let _ = index_repo_path(&conn, &repo.path);
            }
            let _ = event_tx.send(AppEvent::ReposUpdated);
        }

        let _ = event_tx.send(AppEvent::ScanFinished);
    });

    Ok(())
}
