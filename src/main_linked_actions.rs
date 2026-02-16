use super::*;

pub(super) fn maybe_probe_visible_linked_items(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) {
    if app.view() != View::Issues {
        return;
    }
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => return,
    };

    let visible = app
        .issues_for_view()
        .iter()
        .take(20)
        .map(|issue| (issue.number, issue.is_pr))
        .collect::<Vec<(i64, bool)>>();

    for (number, is_pr) in visible {
        if is_pr {
            if !app.begin_linked_issue_lookup(number) {
                continue;
            }
            start_linked_issue_lookup(
                owner.clone(),
                repo.clone(),
                number,
                token.to_string(),
                event_tx.clone(),
                LinkedIssueTarget::Probe,
            );
            continue;
        }

        if !app.begin_linked_pull_request_lookup(number) {
            continue;
        }
        start_linked_pull_request_lookup(
            owner.clone(),
            repo.clone(),
            number,
            token.to_string(),
            event_tx.clone(),
            LinkedPullRequestTarget::Probe,
        );
    }
}

pub(super) fn try_open_cached_linked_pull_request(
    app: &mut App,
    conn: &rusqlite::Connection,
    target: LinkedPullRequestTarget,
) -> Result<bool> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => return Ok(false),
    };
    if issue.is_pr {
        return Ok(false);
    }

    let pull_number = match app.linked_pull_request_for_issue(issue.number) {
        Some(pull_number) => pull_number,
        None => return Ok(false),
    };

    if target == LinkedPullRequestTarget::Tui {
        app.capture_linked_navigation_origin();
        refresh_current_repo_issues(app, conn)?;
        if open_pull_request_in_tui(app, conn, pull_number)? {
            app.set_status(format!(
                "Opened linked pull request #{} in TUI",
                pull_number
            ));
            return Ok(true);
        }
        app.clear_linked_navigation_origin();
        app.set_status(format!(
            "Linked PR #{} not cached in TUI yet; press r then Shift+P",
            pull_number
        ));
        return Ok(true);
    }

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner, repo),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(true);
        }
    };
    let url = format!("https://github.com/{}/{}/pull/{}", owner, repo, pull_number);
    if let Err(error) = open_url(url.as_str()) {
        app.set_status(format!("Open linked PR failed: {}", error));
        return Ok(true);
    }
    app.set_status(format!(
        "Opened linked pull request #{} in browser",
        pull_number
    ));
    Ok(true)
}

pub(super) fn try_open_cached_linked_issue(
    app: &mut App,
    conn: &rusqlite::Connection,
    target: LinkedIssueTarget,
) -> Result<bool> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => return Ok(false),
    };
    if !issue.is_pr {
        return Ok(false);
    }

    let issue_number = match app.linked_issue_for_pull_request(issue.number) {
        Some(issue_number) => issue_number,
        None => return Ok(false),
    };

    if target == LinkedIssueTarget::Tui {
        app.capture_linked_navigation_origin();
        refresh_current_repo_issues(app, conn)?;
        if open_issue_in_tui(app, conn, issue_number)? {
            app.set_status(format!("Opened linked issue #{} in TUI", issue_number));
            return Ok(true);
        }
        app.clear_linked_navigation_origin();
        app.set_status(format!(
            "Linked issue #{} not cached in TUI yet; press r then Shift+P",
            issue_number
        ));
        return Ok(true);
    }

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner, repo),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(true);
        }
    };
    let url = format!(
        "https://github.com/{}/{}/issues/{}",
        owner, repo, issue_number
    );
    if let Err(error) = open_url(url.as_str()) {
        app.set_status(format!("Open linked issue failed: {}", error));
        return Ok(true);
    }
    app.set_status(format!("Opened linked issue #{} in browser", issue_number));
    Ok(true)
}

pub(super) fn open_linked_pull_request(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    target: LinkedPullRequestTarget,
) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };
    if issue.is_pr {
        app.set_status("Selected item is already a pull request".to_string());
        return Ok(());
    }

    let issue_number = issue.number;
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_linked_pull_request_lookup(
        owner,
        repo,
        issue_number,
        token.to_string(),
        event_tx,
        target,
    );
    if target == LinkedPullRequestTarget::Tui {
        app.set_status("Looking up linked pull request for TUI".to_string());
        return Ok(());
    }
    app.set_status("Looking up linked pull request for browser".to_string());
    Ok(())
}

pub(super) fn open_linked_issue(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    target: LinkedIssueTarget,
) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };
    if !issue.is_pr {
        app.set_status("Selected item is not a pull request".to_string());
        return Ok(());
    }

    let pull_number = issue.number;
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_linked_issue_lookup(
        owner,
        repo,
        pull_number,
        token.to_string(),
        event_tx,
        target,
    );
    if target == LinkedIssueTarget::Tui {
        app.set_status("Looking up linked issue for TUI".to_string());
        return Ok(());
    }
    app.set_status("Looking up linked issue for browser".to_string());
    Ok(())
}

pub(super) fn open_pull_request_in_tui(
    app: &mut App,
    conn: &rusqlite::Connection,
    pull_number: i64,
) -> Result<bool> {
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);

    let try_filters = [IssueFilter::Open, IssueFilter::Closed];
    for filter in try_filters {
        app.set_issue_filter(filter);
        if !app.select_issue_by_number(pull_number) {
            continue;
        }

        let (issue_id, issue_number) = match app.selected_issue_row() {
            Some(issue) => (issue.id, issue.number),
            None => return Ok(false),
        };
        app.set_current_issue(issue_id, issue_number);
        app.reset_issue_detail_scroll();
        load_comments_for_issue(app, conn, issue_id)?;
        app.set_view(View::IssueDetail);
        app.set_comment_syncing(false);
        app.request_comment_sync();
        app.request_pull_request_files_sync();
        app.request_pull_request_review_comments_sync();
        return Ok(true);
    }

    Ok(false)
}

pub(super) fn open_issue_in_tui(
    app: &mut App,
    conn: &rusqlite::Connection,
    issue_number: i64,
) -> Result<bool> {
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::Issues);

    let try_filters = [IssueFilter::Open, IssueFilter::Closed];
    for filter in try_filters {
        app.set_issue_filter(filter);
        if !app.select_issue_by_number(issue_number) {
            continue;
        }

        let (issue_id, issue_number) = match app.selected_issue_row() {
            Some(issue) => (issue.id, issue.number),
            None => return Ok(false),
        };
        app.set_current_issue(issue_id, issue_number);
        app.reset_issue_detail_scroll();
        load_comments_for_issue(app, conn, issue_id)?;
        app.set_view(View::IssueDetail);
        app.set_comment_syncing(false);
        app.request_comment_sync();
        return Ok(true);
    }

    Ok(false)
}

pub(super) fn start_linked_pull_request_lookup(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
    target: LinkedPullRequestTarget,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::LinkedPullRequestLookupFailed {
            issue_number,
            message,
            target,
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .find_linked_pull_request(&owner, &repo, issue_number)
                    .await
            });

            match result {
                Ok(linked) => {
                    let (pull_number, url) = match linked {
                        Some((pull_number, url)) => (Some(pull_number), Some(url)),
                        None => (None, None),
                    };
                    let _ = event_tx.send(AppEvent::LinkedPullRequestResolved {
                        issue_number,
                        pull_number,
                        url,
                        target,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::LinkedPullRequestLookupFailed {
                        issue_number,
                        message: error.to_string(),
                        target,
                    });
                }
            }
        },
    );
}

pub(super) fn start_linked_issue_lookup(
    owner: String,
    repo: String,
    pull_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
    target: LinkedIssueTarget,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::LinkedIssueLookupFailed {
            pull_number,
            message,
            target,
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .find_linked_issue_for_pull_request(&owner, &repo, pull_number)
                    .await
            });

            match result {
                Ok(linked) => {
                    let (issue_number, url) = match linked {
                        Some((issue_number, url)) => (Some(issue_number), Some(url)),
                        None => (None, None),
                    };
                    let _ = event_tx.send(AppEvent::LinkedIssueResolved {
                        pull_number,
                        issue_number,
                        url,
                        target,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::LinkedIssueLookupFailed {
                        pull_number,
                        message: error.to_string(),
                        target,
                    });
                }
            }
        },
    );
}

pub(super) fn open_url(url: &str) -> Result<()> {
    if cfg!(target_os = "macos") {
        return run_silent_command(std::process::Command::new("open").arg(url));
    }

    if cfg!(target_os = "windows") {
        return run_silent_command(std::process::Command::new("cmd").args(["/C", "start", url]));
    }

    run_silent_command(std::process::Command::new("xdg-open").arg(url))
}

pub(super) fn run_silent_command(command: &mut std::process::Command) -> Result<()> {
    let status = command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        return Ok(());
    }
    anyhow::bail!("command exited with status {}", status)
}
