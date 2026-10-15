use super::*;

fn app_with_items(view: View) -> App {
    let mut app = App::new(Config::default());
    app.set_issues(
        (1..=2)
            .map(|id| IssueRow {
                id,
                repo_id: 1,
                number: id,
                state: "open".to_string(),
                title: format!("Pull request {id}"),
                body: String::new(),
                labels: vec![],
                assignees: String::new(),
                comments_count: 2,
                updated_at: None,
                is_pr: true,
            })
            .collect(),
    );
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_current_issue(1, 1);
    app.set_comments(
        (1..=2)
            .map(|id| CommentRow {
                id,
                issue_id: 1,
                author: "alex".to_string(),
                body: format!("Comment {id}"),
                created_at: None,
                last_accessed_at: None,
            })
            .collect(),
    );
    app.set_view(view);
    app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    app
}

#[test]
fn help_consumes_actions_navigation_and_focus_keys() {
    for view in [
        View::RepoPicker,
        View::Issues,
        View::IssueDetail,
        View::IssueComments,
        View::PullRequestFiles,
    ] {
        let mut app = app_with_items(view);
        let focus = app.focus;
        let review_focus = app.pull_request_review_focus();
        for (code, modifiers) in [
            (KeyCode::Char('j'), KeyModifiers::NONE),
            (KeyCode::Down, KeyModifiers::NONE),
            (KeyCode::Char('l'), KeyModifiers::CONTROL),
            (KeyCode::Char('d'), KeyModifiers::NONE),
            (KeyCode::Char('d'), KeyModifiers::NONE),
            (KeyCode::Char('x'), KeyModifiers::NONE),
            (KeyCode::Char('M'), KeyModifiers::SHIFT),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char('/'), KeyModifiers::NONE),
            (KeyCode::Char('p'), KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::Char('g'), KeyModifiers::CONTROL),
            (KeyCode::Char('r'), KeyModifiers::CONTROL),
        ] {
            app.on_key(KeyEvent::new(code, modifiers));
            assert!(app.help_overlay_visible(), "{view:?}: {code:?}");
            assert_eq!(app.take_action(), None, "{view:?}: {code:?}");
            assert_eq!(app.view(), view);
            assert_eq!(app.focus, focus);
            assert_eq!(app.pull_request_review_focus(), review_focus);
            assert_eq!(app.navigation.selected_issue, 0);
            assert_eq!(app.navigation.selected_comment, 0);
            assert_eq!(app.issue_filter, IssueFilter::Open);
            assert_eq!(app.work_item_mode(), WorkItemMode::PullRequests);
            assert!(!app.issue_search_mode());
            assert!(!app.repo_search_mode());
            assert!(!app.take_rescan_request());
        }
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.help_overlay_visible());
        app.on_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::SHIFT));
        assert_eq!(
            app.take_action(),
            (view != View::RepoPicker).then_some(AppAction::MergePullRequest)
        );
    }
}

#[test]
fn help_consumes_mouse_targets_and_scrolls_only_help() {
    for (view, target) in [
        (View::Issues, MouseTarget::IssueRow(1)),
        (View::Issues, MouseTarget::IssueTabClosed),
        (View::Issues, MouseTarget::IssuesPreviewPane),
        (View::IssueComments, MouseTarget::CommentRow(1)),
        (View::IssueComments, MouseTarget::RepoPicker),
        (View::PullRequestFiles, MouseTarget::Back),
        (View::PullRequestFiles, MouseTarget::PullRequestDiffPane),
    ] {
        let mut app = app_with_items(view);
        let focus = app.focus;
        app.set_help_max_scroll(20);
        app.register_mouse_region(target, 0, 0, 80, 24);
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::ScrollRight,
            MouseEventKind::ScrollDown,
        ] {
            app.on_mouse(MouseEvent {
                kind,
                column: 1,
                row: 1,
                modifiers: KeyModifiers::NONE,
            });
            assert_eq!(app.view(), view);
            assert_eq!(app.focus, focus);
            assert_eq!(app.navigation.selected_issue, 0);
            assert_eq!(app.navigation.selected_comment, 0);
            assert_eq!(app.issue_filter, IssueFilter::Open);
            assert_eq!(
                app.pull_request_review_focus(),
                PullRequestReviewFocus::Files
            );
            assert_eq!(app.pull_request_diff_horizontal_scroll(), 0);
            assert_eq!(app.take_action(), None);
        }
        assert_eq!(app.help_scroll(), 1);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.help_scroll(), 0);
    }
}

#[test]
fn help_uses_configured_movement_escape_and_quit_bindings() {
    let config = Config {
        keybinds: [
            ("move_down", "ctrl+n"),
            ("move_up", "ctrl+p"),
            ("back_escape", "ctrl+e"),
            ("quit", "ctrl+q"),
        ]
        .into_iter()
        .map(|(action, key)| (action.to_string(), key.to_string()))
        .collect(),
        ..Config::default()
    };
    let mut app = App::new(config);
    app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    app.set_help_max_scroll(20);
    for (code, modifiers) in [
        (KeyCode::Esc, KeyModifiers::NONE),
        (KeyCode::Char('c'), KeyModifiers::CONTROL),
        (KeyCode::Char('j'), KeyModifiers::NONE),
    ] {
        app.on_key(KeyEvent::new(code, modifiers));
    }
    assert!(app.help_overlay_visible());
    assert!(!app.should_quit());
    assert_eq!(app.help_scroll(), 0);
    app.on_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(app.help_scroll(), 1);
    app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert_eq!(app.help_scroll(), 0);
    app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.help_scroll(), 20);
    app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert!(!app.help_overlay_visible());
    app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert_eq!(app.help_scroll(), 0);
    app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(app.should_quit());
}
