use super::*;

#[test]
fn ctrl_a_resets_assignee_filter_to_all() {
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
    ]);

    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.assignee_filter_label(), "alex");

    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert_eq!(app.assignee_filter_label(), "all");
    assert_eq!(app.issues_for_view().len(), 2);
}

#[test]
fn slash_search_filters_and_escape_clears() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 101,
            state: "open".to_string(),
            title: "Login bug".to_string(),
            body: "Fails for SSO users".to_string(),
            labels: "bug,auth".to_string(),
            assignees: "alex".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 202,
            state: "open".to_string(),
            title: "Docs polish".to_string(),
            body: "Update README".to_string(),
            labels: "docs".to_string(),
            assignees: "sam".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);

    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert!(app.issue_search_mode());

    app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));

    assert_eq!(app.issue_query(), "bug");
    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(
        app.selected_issue_row().map(|issue| issue.number),
        Some(101)
    );

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.issue_search_mode());
    assert_eq!(app.issue_query(), "");
    assert_eq!(app.issues_for_view().len(), 2);
}

#[test]
fn slash_search_matches_issue_number() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 777,
        state: "open".to_string(),
        title: "Telemetry".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('#'), KeyModifiers::SHIFT));
    app.on_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));

    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(
        app.selected_issue_row().map(|issue| issue.number),
        Some(777)
    );
}

#[test]
fn reopen_action_for_closed_issue() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 9,
        repo_id: 1,
        number: 99,
        state: "closed".to_string(),
        title: "Closed".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);
    app.set_issue_filter(IssueFilter::Closed);

    app.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    assert_eq!(app.take_action(), Some(AppAction::ReopenIssue));
}

#[test]
fn comment_action_on_issue() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 9,
        repo_id: 1,
        number: 99,
        state: "open".to_string(),
        title: "Open".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
    assert_eq!(app.take_action(), Some(AppAction::AddIssueComment));
}

#[test]
fn m_triggers_pull_request_review_comment_in_review_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1,1 +1,2 @@\n-old\n+new\n+more".to_string()),
        }],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));

    assert_eq!(
        app.take_action(),
        Some(AppAction::AddPullRequestReviewComment)
    );
    let target = app
        .selected_pull_request_review_target()
        .expect("review target");
    assert_eq!(target.path, "src/main.rs");
    assert_eq!(target.line, 1);
    assert_eq!(target.side, ReviewSide::Right);
}

#[test]
fn review_comment_editor_submit_action_is_emitted() {
    let mut app = App::new(Config::default());
    app.open_pull_request_review_comment_editor(
        View::PullRequestFiles,
        PullRequestReviewTarget {
            path: "src/main.rs".to_string(),
            line: 10,
            side: ReviewSide::Right,
            start_line: None,
            start_side: None,
        },
    );

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        app.take_action(),
        Some(AppAction::SubmitPullRequestReviewComment)
    );
}

#[test]
fn shift_r_triggers_resolve_review_comment_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);

    app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));

    assert_eq!(
        app.take_action(),
        Some(AppAction::ResolvePullRequestReviewComment)
    );
}

#[test]
fn w_emits_toggle_pull_request_file_viewed_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
        }],
    );

    app.on_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

    assert_eq!(
        app.take_action(),
        Some(AppAction::TogglePullRequestFileViewed)
    );
}

#[test]
fn custom_quit_keybinding_remaps_and_disables_default() {
    let mut config = Config::default();
    config
        .keybinds
        .insert("quit".to_string(), "ctrl+q".to_string());
    let mut app = App::new(config);

    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!app.should_quit());

    app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(app.should_quit());
}

#[test]
fn diff_horizontal_scroll_uses_keyboard_and_mouse() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
        }],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
    app.set_pull_request_diff_horizontal_max(20);
    app.register_mouse_region(MouseTarget::PullRequestDiffPane, 0, 0, 120, 40);

    app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert_eq!(app.pull_request_diff_horizontal_scroll(), 4);

    app.on_mouse(MouseEvent {
        kind: MouseEventKind::ScrollRight,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.pull_request_diff_horizontal_scroll(), 8);

    app.on_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
    assert_eq!(app.pull_request_diff_horizontal_scroll(), 0);
}

#[test]
fn mouse_back_click_navigates_to_previous_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.register_mouse_region(MouseTarget::Back, 0, 0, 12, 3);

    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.view(), View::IssueDetail);
}

#[test]
fn mouse_click_repo_picker_region_opens_repo_picker() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.register_mouse_region(MouseTarget::RepoPicker, 0, 0, 8, 1);

    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.view(), View::RepoPicker);
}

#[test]
fn mouse_click_issue_row_selects_and_opens_issue() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 12,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);
    app.register_mouse_region(MouseTarget::IssueRow(0), 0, 0, 50, 2);

    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.take_action(), Some(AppAction::PickIssue));
}

#[test]
fn mouse_click_linked_pr_buttons_trigger_actions() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.register_mouse_region(MouseTarget::LinkedPullRequestTuiButton, 0, 0, 16, 1);
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        app.take_action(),
        Some(AppAction::OpenLinkedPullRequestInTui)
    );

    app.register_mouse_region(MouseTarget::LinkedPullRequestWebButton, 0, 1, 10, 1);
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        app.take_action(),
        Some(AppAction::OpenLinkedPullRequestInBrowser)
    );

    app.register_mouse_region(MouseTarget::LinkedIssueTuiButton, 0, 2, 16, 1);
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 2,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInTui));

    app.register_mouse_region(MouseTarget::LinkedIssueWebButton, 0, 3, 10, 1);
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInBrowser));
}

#[test]
fn mouse_click_pr_file_row_selects_file() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![
            PullRequestFile {
                filename: "src/a.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            },
            PullRequestFile {
                filename: "src/b.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            },
        ],
    );
    app.register_mouse_region(MouseTarget::PullRequestFileRow(1), 0, 0, 50, 1);

    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.selected_pull_request_file(), 1);
}

#[test]
fn mouse_click_diff_row_sets_side_and_line() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 2,
            deletions: 1,
            patch: Some("@@ -1,1 +1,2 @@\n-old\n+new\n+more".to_string()),
        }],
    );
    app.register_mouse_region(
        MouseTarget::PullRequestDiffRow(2, ReviewSide::Left),
        0,
        0,
        50,
        1,
    );

    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(
        app.pull_request_review_focus(),
        PullRequestReviewFocus::Diff
    );
    assert_eq!(app.pull_request_review_side(), ReviewSide::Left);
    assert_eq!(app.selected_pull_request_diff_line(), 2);
}

#[test]
fn selected_pull_request_file_view_toggle_flips_current_state() {
    let mut app = App::new(Config::default());
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
        }],
    );

    let (path, viewed) = app
        .selected_pull_request_file_view_toggle()
        .expect("toggle payload");
    assert_eq!(path, "src/main.rs");
    assert!(viewed);

    app.set_pull_request_file_viewed("src/main.rs", true);
    let (_, viewed) = app
        .selected_pull_request_file_view_toggle()
        .expect("toggle payload");
    assert!(!viewed);
}

#[test]
fn c_collapses_selected_hunk_and_navigation_skips_hidden_rows() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 4,
                deletions: 2,
                patch: Some(
                    "@@ -1,1 +1,4 @@\n old\n+new-a\n+new-b\n+new-c\n@@ -10,1 +10,1 @@\n-old-two\n+new-two"
                        .to_string(),
                ),
            }],
        );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.selected_pull_request_diff_line(), 2);

    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

    assert_eq!(app.selected_pull_request_diff_line(), 0);
    assert!(app.pull_request_hunk_is_collapsed("src/main.rs", 0));

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.selected_pull_request_diff_line(), 5);

    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

    assert!(!app.pull_request_hunk_is_collapsed("src/main.rs", 0));
}
