use super::main_actions::issue_url;
use crate::app::{EditorMode, PendingIssueAction, View, WorkItemMode};
use crate::config::Config;
use crate::store::IssueRow;
use std::sync::mpsc::channel;

fn parse_csv_values(input: &str, strip_at: bool) -> Vec<String> {
    let mut values = Vec::new();
    for raw in input.split(',') {
        let mut value = raw.trim().to_string();
        if strip_at {
            value = value.trim_start_matches('@').to_string();
        }
        if value.is_empty() {
            continue;
        }
        if values
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(value.as_str()))
        {
            continue;
        }
        values.push(value);
    }
    values
}

#[test]
fn parse_csv_values_trims_dedupes_and_strips_at() {
    let values = parse_csv_values(" @alex,alex, sam , ,@Sam", true);
    assert_eq!(values, vec!["alex".to_string(), "sam".to_string()]);
}

#[test]
fn parse_csv_values_keeps_label_case() {
    let values = parse_csv_values("bug,needs-triage,BUG", false);
    assert_eq!(values, vec!["bug".to_string(), "needs-triage".to_string()]);
}

#[test]
fn issue_url_uses_pull_route_for_pull_requests() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 10,
        repo_id: 1,
        number: 42,
        state: "open".to_string(),
        title: "Improve docs".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_current_issue(10, 42);
    app.set_view(View::IssueDetail);

    let url = issue_url(&app).expect("url");

    assert_eq!(url, "https://github.com/acme/blippy/pull/42");
}

#[test]
fn issue_url_uses_issue_route_for_issues() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 11,
        repo_id: 1,
        number: 7,
        state: "open".to_string(),
        title: "Bug".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    let url = issue_url(&app).expect("url");

    assert_eq!(url, "https://github.com/acme/blippy/issues/7");
}

#[test]
fn linked_pull_request_action_opens_picker_when_multiple_cached() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 12,
        repo_id: 1,
        number: 7,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);
    app.set_linked_pull_requests(7, vec![42, 43]);

    let handled = super::main_linked_actions::try_open_cached_linked_pull_request(
        &mut app,
        &conn,
        super::LinkedPullRequestTarget::Tui,
    )
    .expect("handled");

    assert!(handled);
    assert_eq!(app.view(), View::LinkedPicker);
    assert_eq!(app.linked_picker_numbers(), vec![42, 43]);
}

#[test]
fn create_issue_action_opens_create_issue_editor() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.on_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('N'),
        crossterm::event::KeyModifiers::SHIFT,
    ));

    let (event_tx, _event_rx) = channel();
    super::main_actions::handle_actions(&mut app, &conn, "token", event_tx).expect("handled");

    assert_eq!(app.view(), View::CommentEditor);
    assert_eq!(app.editor_mode(), EditorMode::CreateIssue);
}

#[test]
fn linked_issue_action_opens_picker_when_multiple_cached() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 21,
        repo_id: 1,
        number: 9,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_linked_issues_for_pull_request(9, vec![100, 101]);

    let handled = super::main_linked_actions::try_open_cached_linked_issue(
        &mut app,
        &conn,
        super::LinkedIssueTarget::Browser,
    )
    .expect("handled");

    assert!(handled);
    assert_eq!(app.view(), View::LinkedPicker);
    assert_eq!(app.linked_picker_numbers(), vec![100, 101]);
}

#[test]
fn reopen_issue_blocks_merged_pull_requests() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issue_filter(crate::app::IssueFilter::Closed);
    app.set_issues(vec![IssueRow {
        id: 30,
        repo_id: 1,
        number: 88,
        state: "merged".to_string(),
        title: "Merged PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);

    let (event_tx, _event_rx) = channel();
    super::main_action_utils::reopen_issue(&mut app, "token", event_tx).expect("reopen helper");

    assert_eq!(app.status(), "Merged pull requests cannot be reopened");
    assert_eq!(app.pending_issue_badge(88), None);
}

#[test]
fn merge_pull_request_blocks_non_pr_items() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 31,
        repo_id: 1,
        number: 90,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    let (event_tx, _event_rx) = channel();
    super::main_action_utils::merge_pull_request(&mut app, "token", event_tx)
        .expect("merge helper");

    assert_eq!(app.status(), "Selected item is not a pull request");
    assert_eq!(app.pending_issue_badge(90), None);
}

#[test]
fn merge_pull_request_checks_permissions() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_repo_pull_request_mergeable(Some(false));
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 32,
        repo_id: 1,
        number: 91,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);

    let (event_tx, _event_rx) = channel();
    super::main_action_utils::merge_pull_request(&mut app, "token", event_tx)
        .expect("merge helper");

    assert_eq!(
        app.status(),
        "No permission to merge pull requests in this repo"
    );
    assert_eq!(app.pending_issue_badge(91), None);
}

#[test]
fn issue_updated_marks_pull_request_merged() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 33,
        repo_id: 1,
        number: 92,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_pending_issue_action(92, PendingIssueAction::Merging);

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueUpdated {
            issue_number: 92,
            message: "merged".to_string(),
        })
        .expect("send event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle events");

    assert_eq!(app.pending_issue_badge(92), None);
    let merged_state = app
        .issues()
        .iter()
        .find(|issue| issue.number == 92)
        .map(|issue| issue.state.as_str());
    assert_eq!(merged_state, Some("merged"));
}

#[test]
fn submit_created_issue_requires_non_empty_title() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.open_create_issue_editor(View::Issues);
    app.on_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));

    let (event_tx, _event_rx) = channel();
    super::main_actions::handle_actions(&mut app, &conn, "token", event_tx).expect("handled");

    assert_eq!(app.status(), "Issue title required");
    assert_eq!(app.view(), View::CommentEditor);
}
