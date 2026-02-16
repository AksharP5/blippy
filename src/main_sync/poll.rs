use super::*;

pub(crate) fn maybe_start_repo_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if app.syncing() {
        return Ok(());
    }

    if !app.take_sync_request() {
        return Ok(());
    }

    let owner = match app.current_owner() {
        Some(owner) => owner.to_string(),
        None => return Ok(()),
    };
    let repo = match app.current_repo() {
        Some(repo) => repo.to_string(),
        None => return Ok(()),
    };

    super::repo_sync::start_repo_sync(owner, repo, token.to_string(), event_tx);
    app.set_syncing(true);
    app.set_status("Syncing".to_string());
    Ok(())
}

pub(crate) fn maybe_start_repo_permissions_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) {
    if app.repo_permissions_syncing() {
        return;
    }
    if !app.take_repo_permissions_sync_request() {
        return;
    }

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => return,
    };

    super::repo_sync::start_fetch_repo_permissions(owner, repo, token.to_string(), event_tx);
    app.set_repo_permissions_syncing(true);
}

pub(crate) fn maybe_start_repo_labels_sync(app: &mut App, token: &str, event_tx: Sender<AppEvent>) {
    if app.repo_labels_syncing() {
        return;
    }
    if !app.take_repo_labels_sync_request() {
        return;
    }

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => return,
    };

    super::repo_sync::start_fetch_labels(owner, repo, token.to_string(), event_tx);
    app.set_repo_labels_syncing(true);
}

pub(crate) fn maybe_start_issue_poll(app: &mut App, last_poll: &mut Instant) {
    if !matches!(
        app.view(),
        View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return;
    }

    if last_poll.elapsed() < ISSUE_POLL_INTERVAL {
        return;
    }

    app.request_sync();
    *last_poll = Instant::now();
}

pub(crate) fn maybe_start_comment_poll(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    last_poll: &mut Instant,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }

    if app.comment_syncing() {
        return Ok(());
    }

    if !app.take_comment_sync_request() && last_poll.elapsed() < COMMENT_POLL_INTERVAL {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    super::repo_sync::start_comment_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_comment_syncing(true);
    *last_poll = Instant::now();
    Ok(())
}

pub(crate) fn maybe_start_pull_request_files_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }
    if app.pull_request_files_syncing() {
        return Ok(());
    }
    if !app.take_pull_request_files_sync_request() {
        return Ok(());
    }
    if !app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    super::pr_sync::start_pull_request_files_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_pull_request_files_syncing(true);
    app.set_status("Loading pull request changes".to_string());
    Ok(())
}

pub(crate) fn maybe_start_pull_request_review_comments_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }
    if app.pull_request_review_comments_syncing() {
        return Ok(());
    }
    if !app.take_pull_request_review_comments_sync_request() {
        return Ok(());
    }
    if !app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    super::pr_sync::start_pull_request_review_comments_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_pull_request_review_comments_syncing(true);
    Ok(())
}
