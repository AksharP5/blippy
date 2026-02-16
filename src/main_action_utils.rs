use super::*;

pub(super) fn handle_preset_selection(
    app: &mut App,
    _conn: &rusqlite::Connection,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    match app.preset_selection() {
        PresetSelection::CloseWithoutComment => {
            close_issue_with_comment(app, token, None, event_tx)?;
        }
        PresetSelection::CustomMessage => {
            app.open_close_comment_editor();
        }
        PresetSelection::Preset(index) => {
            let body = app
                .comment_defaults()
                .get(index)
                .map(|preset| preset.body.clone());
            if body.is_none() {
                app.set_status("Preset not found".to_string());
                return Ok(());
            }
            close_issue_with_comment(app, token, body, event_tx)?;
        }
        PresetSelection::AddPreset => {
            app.editor_mut().reset_for_preset_name();
            app.set_view(View::CommentPresetName);
        }
    }
    Ok(())
}

pub(super) fn save_preset_from_editor(app: &mut App) -> Result<()> {
    let name = app.editor().name().trim().to_string();
    if name.is_empty() {
        app.set_status("Preset name required".to_string());
        return Ok(());
    }
    let body = app.editor().text().to_string();
    if body.trim().is_empty() {
        app.set_status("Preset body required".to_string());
        return Ok(());
    }

    app.add_comment_default(crate::config::CommentDefault { name, body });
    app.save_config()?;
    app.set_status("Preset saved".to_string());
    Ok(())
}

pub(super) fn close_issue_with_comment(
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

pub(super) fn post_issue_comment(
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

pub(super) fn update_issue_comment(
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

pub(super) fn submit_pull_request_review_comment(
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

pub(super) fn update_pull_request_review_comment(
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

pub(super) fn delete_pull_request_review_comment(
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

pub(super) fn resolve_pull_request_review_comment(
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

pub(super) fn toggle_pull_request_file_viewed(
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

pub(super) fn delete_issue_comment(
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

pub(super) fn update_issue_labels(
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

pub(super) fn update_issue_assignees(
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

pub(super) fn reopen_issue(app: &mut App, token: &str, event_tx: Sender<AppEvent>) -> Result<()> {
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

pub(super) fn ensure_can_edit_issue_metadata(app: &mut App) -> bool {
    if app.repo_issue_metadata_editable() == Some(true) {
        return true;
    }
    if app.repo_issue_metadata_editable() == Some(false) {
        app.set_status("No permission to edit labels/assignees in this repo".to_string());
        return false;
    }

    app.request_repo_permissions_sync();
    if app.repo_permissions_syncing() {
        app.set_status("Checking repo permissions".to_string());
        return false;
    }
    app.set_status("Checking repo permissions, try again in a moment".to_string());
    false
}

pub(super) fn selected_issue_for_action(app: &App) -> Option<(i64, i64, Option<String>)> {
    if app.view() == View::Issues {
        return app
            .selected_issue_row()
            .map(|issue| (issue.id, issue.number, Some(issue.state.clone())));
    }

    if matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        if let Some(issue) = app.current_issue_row() {
            return Some((issue.id, issue.number, Some(issue.state.clone())));
        }
        if let (Some(issue_id), Some(issue_number)) =
            (app.current_issue_id(), app.current_issue_number())
        {
            return Some((issue_id, issue_number, None));
        }
    }

    None
}

pub(super) fn selected_issue_labels(app: &App) -> Option<String> {
    if app.view() == View::Issues {
        return app.selected_issue_row().map(|issue| issue.labels.clone());
    }
    if matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles | View::CommentEditor
    ) {
        return app.current_issue_row().map(|issue| issue.labels.clone());
    }
    None
}

pub(super) fn selected_issue_assignees(app: &App) -> Option<String> {
    if app.view() == View::Issues {
        return app
            .selected_issue_row()
            .map(|issue| issue.assignees.clone());
    }
    if matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles | View::CommentEditor
    ) {
        return app.current_issue_row().map(|issue| issue.assignees.clone());
    }
    None
}

pub(super) fn label_options_for_repo(app: &App) -> Vec<String> {
    let mut labels = app
        .issues()
        .iter()
        .flat_map(|issue| issue.labels.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<String>>();
    labels.sort_by_key(|value| value.to_ascii_lowercase());
    labels.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    labels
}

pub(super) fn assignee_options_for_repo(app: &App) -> Vec<String> {
    let mut assignees = app
        .issues()
        .iter()
        .flat_map(|issue| issue.assignees.split(','))
        .map(str::trim)
        .map(|value| value.trim_start_matches('@'))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<String>>();
    assignees.sort_by_key(|value| value.to_ascii_lowercase());
    assignees.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    assignees
}

pub(super) fn issue_number(app: &App) -> Option<i64> {
    match app.view() {
        View::IssueDetail
        | View::IssueComments
        | View::PullRequestFiles
        | View::LabelPicker
        | View::AssigneePicker
        | View::CommentPresetPicker
        | View::CommentPresetName
        | View::CommentEditor => app.current_issue_number(),
        View::Issues => app.selected_issue_row().map(|issue| issue.number),
        _ => None,
    }
}

pub(super) fn issue_url(app: &App) -> Option<String> {
    let owner = app.current_owner()?;
    let repo = app.current_repo()?;
    let issue = app.current_or_selected_issue()?;
    let issue_number = issue.number;
    let route = if issue.is_pr { "pull" } else { "issues" };

    Some(format!(
        "https://github.com/{}/{}/{}/{}",
        owner, repo, route, issue_number
    ))
}

pub(super) fn checkout_pull_request(app: &mut App) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    if !issue.is_pr {
        app.set_status("Selected item is not a pull request".to_string());
        return Ok(());
    }

    let working_dir = app.current_repo_path().unwrap_or(".").to_string();
    let issue_number = issue.number;
    let number = issue_number.to_string();
    let before_branch = current_git_branch(working_dir.as_str());
    let before_head = current_git_head(working_dir.as_str());

    let output = std::process::Command::new("gh")
        .args(["pr", "checkout", number.as_str()])
        .current_dir(working_dir.as_str())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            app.set_status(format!("PR checkout failed: {}", error));
            return Ok(());
        }
    };

    if output.status.success() {
        return finalize_checkout_status(
            app,
            working_dir.as_str(),
            issue_number,
            before_branch,
            before_head,
        );
    }

    let detached_output = std::process::Command::new("gh")
        .args(["pr", "checkout", number.as_str(), "--detach"])
        .current_dir(working_dir.as_str())
        .output();

    if detached_output
        .as_ref()
        .is_ok_and(|out| out.status.success())
    {
        return finalize_checkout_status(
            app,
            working_dir.as_str(),
            issue_number,
            before_branch,
            before_head,
        );
    }

    let primary_message = command_error_message(&output);
    let detached_message = detached_output
        .as_ref()
        .map(command_error_message)
        .unwrap_or_else(|error| error.to_string());
    let combined = if detached_message.is_empty() || detached_message == primary_message {
        primary_message
    } else if primary_message.is_empty() {
        detached_message
    } else {
        format!("{}; fallback failed: {}", primary_message, detached_message)
    };

    if combined.is_empty() {
        app.set_status(format!("PR checkout failed for #{}", issue_number));
        return Ok(());
    }

    app.set_status(format!("PR checkout failed: {}", combined));
    Ok(())
}

pub(super) fn finalize_checkout_status(
    app: &mut App,
    working_dir: &str,
    issue_number: i64,
    before_branch: Option<String>,
    before_head: Option<String>,
) -> Result<()> {
    let after_branch = current_git_branch(working_dir);
    let after_head = current_git_head(working_dir);

    if before_branch == after_branch && before_head == after_head {
        if let Some(branch) = after_branch {
            app.set_status(format!(
                "PR #{} already active on {} (no checkout changes)",
                issue_number, branch
            ));
            return Ok(());
        }
        app.set_status(format!(
            "PR #{} already active (no checkout changes)",
            issue_number
        ));
        return Ok(());
    }

    if let Some(branch) = after_branch {
        app.set_status(format!("Checked out PR #{} on {}", issue_number, branch));
        return Ok(());
    }

    app.set_status(format!("Checked out PR #{}", issue_number));
    Ok(())
}

pub(super) fn command_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(output.stderr.as_slice())
        .trim()
        .to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string()
}

pub(super) fn current_git_branch(working_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

pub(super) fn current_git_head(working_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}
