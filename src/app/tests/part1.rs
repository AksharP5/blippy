use super::*;

#[test]
fn dd_triggers_close_issue_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 1,
        state: "open".to_string(),
        title: "Test".to_string(),
        body: "Body".to_string(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::CloseIssue));
}

#[test]
fn dd_triggers_close_issue_action_in_detail_view() {
    let mut app = App::new(Config::default());
    app.set_issues(vec![IssueRow {
        id: 42,
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
    app.set_current_issue(42, 7);
    app.set_view(View::IssueDetail);

    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::CloseIssue));
}

#[test]
fn enter_on_pr_changes_pane_opens_full_pr_changes_view() {
    let mut app = App::new(Config::default());
    app.set_issues(vec![IssueRow {
        id: 43,
        repo_id: 1,
        number: 8,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_current_issue(43, 8);
    app.set_view(View::IssueDetail);

    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.view(), View::PullRequestFiles);
}

#[test]
fn enter_on_issue_side_pane_opens_comments_view() {
    let mut app = App::new(Config::default());
    app.set_issues(vec![IssueRow {
        id: 44,
        repo_id: 1,
        number: 9,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);
    app.set_current_issue(44, 9);
    app.set_view(View::IssueDetail);

    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.view(), View::IssueComments);
}

#[test]
fn enter_on_pr_body_opens_comments_view() {
    let mut app = App::new(Config::default());
    app.set_issues(vec![IssueRow {
        id: 45,
        repo_id: 1,
        number: 10,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_current_issue(45, 10);
    app.set_view(View::IssueDetail);

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.view(), View::IssueComments);
}

#[test]
fn tab_cycles_issue_filter() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Open".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 2,
            state: "closed".to_string(),
            title: "Closed".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);

    assert_eq!(app.issue_filter(), IssueFilter::Open);
    assert_eq!(app.issues_for_view().len(), 1);

    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Closed);
    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));

    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Open);
    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(1));
}

#[test]
fn p_toggles_between_issue_and_pr_modes() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 2,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        },
    ]);

    assert_eq!(app.work_item_mode(), WorkItemMode::Issues);
    assert_eq!(app.issues_for_view().len(), 1);
    assert!(!app.issues_for_view()[0].is_pr);

    app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert_eq!(app.work_item_mode(), WorkItemMode::PullRequests);
    assert_eq!(app.issues_for_view().len(), 1);
    assert!(app.issues_for_view()[0].is_pr);
}

#[test]
fn select_issue_by_number_finds_item_in_filtered_mode() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 11,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 22,
            state: "closed".to_string(),
            title: "PR".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        },
    ]);

    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issue_filter(IssueFilter::Closed);

    assert!(app.select_issue_by_number(22));
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(22));
    assert!(!app.select_issue_by_number(11));
}

#[test]
fn selected_issue_has_known_linked_pr_when_cached() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 10,
        repo_id: 1,
        number: 55,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    assert!(!app.selected_issue_has_known_linked_pr());
    app.set_linked_pull_request(55, Some(99));
    assert!(app.selected_issue_has_known_linked_pr());
}

#[test]
fn v_triggers_checkout_pull_request_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::CheckoutPullRequest));
}

#[test]
fn shift_o_triggers_open_linked_pull_request_in_browser_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT));

    assert_eq!(
        app.take_action(),
        Some(AppAction::OpenLinkedPullRequestInBrowser)
    );
}

#[test]
fn shift_p_triggers_open_linked_pull_request_in_tui_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT));

    assert_eq!(
        app.take_action(),
        Some(AppAction::OpenLinkedPullRequestInTui)
    );
}

#[test]
fn shift_o_on_selected_pr_triggers_open_linked_issue_in_browser_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 42,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT));

    assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInBrowser));
}

#[test]
fn shift_p_on_selected_pr_triggers_open_linked_issue_in_tui_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 42,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT));

    assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInTui));
}

#[test]
fn r_requests_pull_request_files_sync_for_pr_detail() {
    let mut app = App::new(Config::default());
    app.set_view(View::IssueDetail);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 10,
        state: "open".to_string(),
        title: "PR".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_current_issue(1, 10);

    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

    assert!(app.take_pull_request_files_sync_request());
}

#[test]
fn issue_filter_uses_1_and_2_shortcuts() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Closed);

    app.on_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Open);
}

#[test]
fn bracket_keys_do_not_change_issue_filter() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Closed);

    app.on_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Closed);

    app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert_eq!(app.issue_filter(), IssueFilter::Closed);
}

#[test]
fn ctrl_l_moves_focus_to_preview_and_j_scrolls_preview() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 1,
        state: "open".to_string(),
        title: "Open".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    assert_eq!(app.focus(), Focus::IssuesList);
    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    assert_eq!(app.focus(), Focus::IssuesPreview);

    app.set_issues_preview_max_scroll(5);
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.issues_preview_scroll(), 1);
}

#[test]
fn a_cycles_assignee_filter() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "One".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: "alex".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 2,
            state: "open".to_string(),
            title: "Two".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: "sam".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 3,
            repo_id: 1,
            number: 3,
            state: "open".to_string(),
            title: "Three".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);

    assert_eq!(app.assignee_filter_label(), "all");
    assert_eq!(app.issues_for_view().len(), 3);

    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.assignee_filter_label(), "unassigned");
    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(3));

    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.assignee_filter_label(), "alex");
    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(1));

    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.assignee_filter_label(), "sam");

    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.assignee_filter_label(), "all");
}
