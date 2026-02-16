use super::*;

pub(crate) fn submit_pull_request_review_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Review comment cannot be empty".to_string());
        return Ok(());
    }

    let target = match app.take_pending_review_target() {
        Some(target) => target,
        None => {
            app.set_status("No review target selected".to_string());
            return Ok(());
        }
    };

    let pull_number = match issue_number(app) {
        Some(pull_number) => pull_number,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_create_pull_request_review_comment(
        owner,
        repo,
        issue_id,
        pull_number,
        target.path,
        target.line,
        target.side,
        target.start_line,
        target.start_side,
        token.to_string(),
        body,
        event_tx,
    );
    app.set_view(app.editor_cancel_view());
    app.set_status("Submitting review comment".to_string());
    Ok(())
}

pub(crate) fn update_pull_request_review_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Review comment cannot be empty".to_string());
        return Ok(());
    }

    let comment_id = match app.take_editing_pull_request_review_comment_id() {
        Some(comment_id) => comment_id,
        None => {
            app.set_status("No review comment selected".to_string());
            return Ok(());
        }
    };

    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_update_pull_request_review_comment(
        owner,
        repo,
        issue_id,
        comment_id,
        token.to_string(),
        body,
        event_tx,
    );
    app.set_view(app.editor_cancel_view());
    app.set_status("Updating review comment".to_string());
    Ok(())
}

pub(crate) fn delete_pull_request_review_comment(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let comment = match app.selected_pull_request_review_comment() {
        Some(comment) => comment,
        None => {
            app.set_status("No review comment selected".to_string());
            return Ok(());
        }
    };

    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_delete_pull_request_review_comment(
        owner,
        repo,
        issue_id,
        comment.id,
        token.to_string(),
        event_tx,
    );
    app.set_status("Deleting review comment".to_string());
    Ok(())
}

pub(crate) fn resolve_pull_request_review_comment(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let comment = match app.selected_pull_request_review_comment() {
        Some(comment) => comment,
        None => {
            app.set_status("No review comment selected".to_string());
            return Ok(());
        }
    };
    let thread_id = match comment.thread_id.clone() {
        Some(thread_id) => thread_id,
        None => {
            app.set_status("Selected comment has no resolvable thread".to_string());
            return Ok(());
        }
    };
    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    let resolve = !comment.resolved;
    start_toggle_pull_request_review_thread_resolution(
        owner,
        repo,
        issue_id,
        thread_id,
        resolve,
        token.to_string(),
        event_tx,
    );
    if resolve {
        app.set_status("Resolving review thread".to_string());
        return Ok(());
    }
    app.set_status("Reopening review thread".to_string());
    Ok(())
}

pub(crate) fn toggle_pull_request_file_viewed(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let (path, viewed) = match app.selected_pull_request_file_view_toggle() {
        Some(toggle) => toggle,
        None => {
            app.set_status("No changed file selected".to_string());
            return Ok(());
        }
    };
    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let pull_request_id = match app.pull_request_id() {
        Some(pull_request_id) => pull_request_id.to_string(),
        None => {
            app.request_pull_request_files_sync();
            app.set_status("Loading pull request metadata".to_string());
            return Ok(());
        }
    };
    if !matches!(
        (app.current_owner(), app.current_repo()),
        (Some(_), Some(_))
    ) {
        app.set_status("No repo selected".to_string());
        return Ok(());
    }

    app.set_pull_request_file_viewed(path.as_str(), viewed);
    start_set_pull_request_file_viewed(
        issue_id,
        pull_request_id,
        path.clone(),
        viewed,
        token.to_string(),
        event_tx,
    );
    if viewed {
        app.set_status(format!("Marking {} viewed on GitHub", path));
        return Ok(());
    }
    app.set_status(format!("Marking {} unviewed on GitHub", path));
    Ok(())
}
