use super::*;

pub(crate) fn ensure_can_edit_issue_metadata(app: &mut App) -> bool {
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

pub(crate) fn ensure_can_merge_pull_request(app: &mut App) -> bool {
    if app.repo_pull_request_mergeable() == Some(true) {
        return true;
    }
    if app.repo_pull_request_mergeable() == Some(false) {
        app.set_status("No permission to merge pull requests in this repo".to_string());
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

pub(crate) fn selected_issue_for_action(app: &App) -> Option<(i64, i64, Option<String>)> {
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

pub(crate) fn selected_issue_labels(app: &App) -> Option<String> {
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

pub(crate) fn selected_issue_assignees(app: &App) -> Option<String> {
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

pub(crate) fn label_options_for_repo(app: &App) -> Vec<String> {
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

pub(crate) fn assignee_options_for_repo(app: &App) -> Vec<String> {
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

pub(crate) fn issue_number(app: &App) -> Option<i64> {
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

pub(crate) fn issue_url(app: &App) -> Option<String> {
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
