use super::*;

pub(crate) fn close_issue_with_comment(
    app: &mut App,
    token: &str,
    body: Option<String>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let (owner, repo, issue_number) =
        match (app.current_owner(), app.current_repo(), issue_number(app)) {
            (Some(owner), Some(repo), Some(issue_number)) => {
                (owner.to_string(), repo.to_string(), issue_number)
            }
            _ => {
                app.set_status("No issue selected".to_string());
                return Ok(());
            }
        };

    start_close_issue(owner, repo, issue_number, token.to_string(), body, event_tx);
    app.set_pending_issue_action(issue_number, PendingIssueAction::Closing);
    app.set_view(View::Issues);
    app.set_status("Closing issue".to_string());
    Ok(())
}

pub(crate) fn post_issue_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Comment cannot be empty".to_string());
        return Ok(());
    }

    let (owner, repo, issue_number) =
        match (app.current_owner(), app.current_repo(), issue_number(app)) {
            (Some(owner), Some(repo), Some(issue_number)) => {
                (owner.to_string(), repo.to_string(), issue_number)
            }
            _ => {
                app.set_status("No issue selected".to_string());
                return Ok(());
            }
        };

    start_add_comment(owner, repo, issue_number, token.to_string(), body, event_tx);
    app.set_view(app.editor_cancel_view());
    app.set_status("Posting comment".to_string());
    Ok(())
}

pub(crate) fn update_issue_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Comment cannot be empty".to_string());
        return Ok(());
    }

    let comment_id = match app.take_editing_comment_id() {
        Some(comment_id) => comment_id,
        None => {
            app.set_status("No comment selected".to_string());
            return Ok(());
        }
    };

    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
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

    start_update_comment(
        owner,
        repo,
        issue_number,
        comment_id,
        token.to_string(),
        body,
        event_tx,
    );
    app.set_view(app.editor_cancel_view());
    app.set_status("Updating comment".to_string());
    Ok(())
}

pub(crate) fn delete_issue_comment(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let comment = match app.selected_comment_row() {
        Some(comment) => comment,
        None => {
            app.set_status("No comment selected".to_string());
            return Ok(());
        }
    };
    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
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

    start_delete_comment(
        owner,
        repo,
        issue_number,
        comment.id,
        comment.issue_id,
        token.to_string(),
        event_tx,
    );
    app.set_status("Deleting comment".to_string());
    Ok(())
}

pub(crate) fn update_issue_labels(
    app: &mut App,
    token: &str,
    labels: Vec<String>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
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

    let labels_display = labels.join(",");
    start_update_labels(
        owner,
        repo,
        issue_number,
        token.to_string(),
        labels,
        event_tx,
        labels_display,
    );
    app.set_pending_issue_action(issue_number, PendingIssueAction::UpdatingLabels);
    app.set_view(app.editor_cancel_view());
    app.set_status(format!("Updating labels for #{}", issue_number));
    Ok(())
}

pub(crate) fn update_issue_assignees(
    app: &mut App,
    token: &str,
    assignees: Vec<String>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
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

    let assignees_display = assignees.join(",");
    start_update_assignees(
        owner,
        repo,
        issue_number,
        token.to_string(),
        assignees,
        event_tx,
        assignees_display,
    );
    app.set_pending_issue_action(issue_number, PendingIssueAction::UpdatingAssignees);
    app.set_view(app.editor_cancel_view());
    app.set_status(format!("Updating assignees for #{}", issue_number));
    Ok(())
}

pub(crate) fn reopen_issue(app: &mut App, token: &str, event_tx: Sender<AppEvent>) -> Result<()> {
    let (issue_id, issue_number, issue_state) = match selected_issue_for_action(app) {
        Some(issue) => issue,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };

    if issue_state
        .as_deref()
        .is_some_and(|state| state.eq_ignore_ascii_case("open"))
    {
        app.set_status("Issue is already open".to_string());
        return Ok(());
    }

    app.set_current_issue(issue_id, issue_number);
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_reopen_issue(owner, repo, issue_number, token.to_string(), event_tx);
    app.set_pending_issue_action(issue_number, PendingIssueAction::Reopening);
    app.set_status("Reopening issue".to_string());
    Ok(())
}
