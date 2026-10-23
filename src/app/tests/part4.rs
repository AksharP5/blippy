use super::*;

#[test]
fn refresh_preserves_preview_scroll_for_the_same_issue() {
    let mut app = App::new(Config::default());
    let issue = IssueRow {
        id: 1,
        repo_id: 1,
        number: 1,
        state: "open".into(),
        title: "Long issue".into(),
        body: "body\n".repeat(80),
        labels: vec![],
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    };
    app.set_issues(vec![issue.clone()]);
    app.set_view(View::Issues);
    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    app.set_issues_preview_max_scroll(60);
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.issues_preview_scroll(), 1);

    app.set_issues(vec![issue.clone()]);
    assert_eq!(app.issues_preview_scroll(), 1);
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.issues_preview_scroll(), 2);
    app.set_issues_preview_max_scroll(1);
    assert_eq!(app.issues_preview_scroll(), 1);

    app.set_issues(vec![IssueRow {
        id: 2,
        repo_id: 2,
        ..issue
    }]);
    assert_eq!(
        app.issues_preview_scroll(),
        0,
        "new repository resets the preview even for the same issue number"
    );
}

#[test]
fn refresh_preserves_comment_reading_positions() {
    let mut app = App::new(Config::default());
    let mut comment = CommentRow {
        id: 1,
        issue_id: 10,
        author: "dev".into(),
        body: "body\n".repeat(80),
        created_at: None,
        last_accessed_at: Some(1),
    };
    app.set_current_issue(10, 1);
    app.set_comments(vec![comment.clone()]);
    app.set_view(View::IssueDetail);
    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    app.set_issue_recent_comments_max_scroll(60);
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.set_view(View::IssueComments);
    app.set_issue_comments_max_scroll(60);
    app.set_issue_comment_offsets(vec![0]);
    app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
    assert_eq!(app.issue_comments_scroll(), 60);
    assert_eq!(app.issue_recent_comments_scroll(), 1);

    comment.last_accessed_at = Some(2);
    app.set_comments(vec![comment.clone()]);
    app.set_issue_comments_max_scroll(60);
    app.set_issue_comment_offsets(vec![0]);
    assert_eq!(app.issue_comments_scroll(), 60);
    assert_eq!(app.issue_recent_comments_scroll(), 1);
    app.set_issue_comments_max_scroll(10);
    assert_eq!(app.issue_comments_scroll(), 10);

    app.set_current_issue(11, 2);
    app.set_comments(vec![CommentRow {
        id: 2,
        issue_id: 11,
        ..comment
    }]);
    assert_eq!(app.issue_comments_scroll(), 0);
    assert_eq!(app.issue_recent_comments_scroll(), 0);
}

#[test]
fn refresh_keeps_position_inside_surviving_comment_after_predecessor_changes() {
    let mut positions = Vec::new();
    let mut expected_positions = Vec::new();
    for (name, ids, offsets, max_scroll, expected_scroll) in [
        ("deletion", vec![2, 3], vec![0, 80], 140, 25),
        ("insertion", vec![1, 4, 2, 3], vec![0, 20, 40, 120], 180, 65),
        (
            "predecessor grows",
            vec![1, 2, 3],
            vec![0, 60, 140],
            200,
            85,
        ),
        (
            "predecessor shrinks",
            vec![1, 2, 3],
            vec![0, 10, 90],
            150,
            35,
        ),
    ] {
        let mut app = App::new(Config::default());
        let comment = |id| CommentRow {
            id,
            issue_id: 10,
            author: "dev".into(),
            body: "body\n".repeat(80),
            created_at: None,
            last_accessed_at: None,
        };
        app.set_current_issue(10, 1);
        app.set_view(View::IssueComments);
        app.set_comments(vec![comment(1), comment(2), comment(3)]);
        app.set_issue_comments_max_scroll(160);
        app.set_issue_comment_offsets(vec![0, 20, 100]);
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.navigation.issue_comments_scroll = 45;

        let next_comment_offset = *offsets.last().expect("next comment offset");
        app.set_comments(ids.into_iter().map(comment).collect());
        app.set_issue_comments_max_scroll(max_scroll);
        app.set_issue_comment_offsets(offsets);

        assert_eq!(app.selected_comment_row().map(|row| row.id), Some(2));
        positions.push((name, app.issue_comments_scroll()));
        expected_positions.push((name, expected_scroll));
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.selected_comment_row().map(|row| row.id), Some(3));
        assert_eq!(app.issue_comments_scroll(), next_comment_offset, "{name}");
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.issue_comments_scroll(), expected_scroll - 25, "{name}");

        app.navigation.issue_comments_scroll = expected_scroll;
        app.set_comments(app.comments().to_vec());
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        let mut new_offsets = app.navigation.issue_comment_offsets.clone();
        *new_offsets.last_mut().expect("last comment offset") += 20;
        app.set_issue_comments_max_scroll(max_scroll + 20);
        app.set_issue_comment_offsets(new_offsets);
        assert_eq!(app.selected_comment_row().map(|row| row.id), Some(3));
        assert_eq!(
            app.issue_comments_scroll(),
            next_comment_offset + 20,
            "explicit selection during {name} refresh"
        );
    }
    assert_eq!(positions, expected_positions);
}

#[test]
fn refreshing_comments_rebases_before_clamping_and_resets_removed_selection() {
    let mut app = App::new(Config::default());
    let comment = |id| CommentRow {
        id,
        issue_id: 10,
        author: "dev".into(),
        body: "body\n".repeat(100),
        created_at: None,
        last_accessed_at: None,
    };
    app.set_current_issue(10, 1);
    app.set_view(View::IssueComments);
    app.set_comments(vec![comment(1), comment(2)]);
    app.set_issue_comments_max_scroll(100);
    app.set_issue_comment_offsets(vec![0, 20]);
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.navigation.issue_comments_scroll = 90;

    app.set_comments(vec![comment(2)]);
    app.set_issue_comments_max_scroll(80);
    app.set_issue_comment_offsets(vec![0]);
    assert_eq!(app.issue_comments_scroll(), 70);

    app.set_comments(vec![CommentRow {
        body: "short".into(),
        ..comment(2)
    }]);
    app.set_issue_comments_max_scroll(5);
    app.set_issue_comment_offsets(vec![0]);
    assert_eq!(app.issue_comments_scroll(), 5);

    app.set_comments(vec![comment(3)]);
    app.set_issue_comments_max_scroll(80);
    app.set_issue_comment_offsets(vec![0]);
    assert_eq!(app.issue_comments_scroll(), 0);

    app.navigation.issue_comments_scroll = 50;
    app.set_comments(vec![comment(3)]);
    app.set_view(View::IssueDetail);
    app.reset_issue_comments_scroll();
    app.set_view(View::IssueComments);
    app.set_issue_comments_max_scroll(80);
    app.set_issue_comment_offsets(vec![0]);
    assert_eq!(app.issue_comments_scroll(), 0);

    app.navigation.issue_comments_scroll = 50;
    app.set_comments(vec![comment(3)]);
    app.set_current_issue(11, 2);
    app.set_comments(vec![CommentRow {
        issue_id: 11,
        ..comment(4)
    }]);
    app.set_issue_comments_max_scroll(80);
    app.set_issue_comment_offsets(vec![0]);
    assert_eq!(app.issue_comments_scroll(), 0);
}

#[test]
fn editing_and_deleting_predecessors_keeps_the_rendered_reading_lines() {
    use ratatui::{Terminal, backend::TestBackend};

    let mut app = App::new(Config::default());
    let predecessor = CommentRow {
        id: 1,
        issue_id: 10,
        author: "dev".into(),
        body: "earlier line\n".repeat(20),
        created_at: None,
        last_accessed_at: None,
    };
    let selected = CommentRow {
        id: 2,
        body: (0..100).map(|i| format!("reading-line-{i}\n")).collect(),
        ..predecessor.clone()
    };
    app.set_current_issue(10, 1);
    app.set_view(View::IssueComments);
    app.set_comments(vec![predecessor.clone(), selected]);
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).expect("test terminal");
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app))
        .expect("draw comments");
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.navigation.issue_comments_scroll += 25;
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app))
        .expect("draw reading position");
    let reading_lines = |terminal: &Terminal<TestBackend>| {
        terminal
            .backend()
            .buffer()
            .content
            .chunks(60)
            .map(|cells| cells.iter().map(|cell| cell.symbol()).collect::<String>())
            .filter(|row| row.contains("reading-line-"))
            .collect::<Vec<_>>()
    };
    let before = reading_lines(&terminal);
    assert!(before[0].contains("reading-line-24"));

    app.update_comment_body_by_id(1, &"earlier line\n".repeat(40));
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app))
        .expect("draw edited predecessor");
    let after_edit = reading_lines(&terminal);
    app.remove_comment_by_id(1);
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app))
        .expect("draw deleted predecessor");
    assert_eq!(app.selected_comment_row().map(|row| row.id), Some(2));
    assert_eq!(reading_lines(&terminal), before);
    assert_eq!(after_edit, before);
}

#[test]
fn a_mouse_click_toggles_a_label_once() {
    let mut app = App::new(Config::default());
    app.open_label_picker(View::Issues, vec!["bug".to_string()], &[]);
    app.register_mouse_region(MouseTarget::LabelOption(0), 1, 1, 10, 1);

    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        app.on_mouse(MouseEvent {
            kind,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
    }

    assert_eq!(app.selected_labels(), vec!["bug"]);
}

#[test]
fn repo_picker_search_filters_entries() {
    let mut app = App::new(Config::default());
    app.set_repos(vec![
        LocalRepoRow {
            path: "/tmp/one".to_string(),
            remote_name: "origin".to_string(),
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            url: "https://github.com/acme/blippy.git".to_string(),
            last_seen: None,
            last_scanned: None,
        },
        LocalRepoRow {
            path: "/tmp/two".to_string(),
            remote_name: "origin".to_string(),
            owner: "other".to_string(),
            repo: "core".to_string(),
            url: "https://github.com/other/core.git".to_string(),
            last_seen: None,
            last_scanned: None,
        },
    ]);

    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for ch in "acme".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }

    assert_eq!(app.filtered_repo_rows().len(), 1);
    assert_eq!(app.filtered_repo_rows()[0].owner, "acme");

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.filtered_repo_rows().len(), 2);
}

#[test]
fn repeated_rescan_keys_do_not_start_overlapping_scans() {
    let mut app = App::new(Config::default());
    let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    app.on_key(key);
    assert!(app.scanning());
    assert!(app.take_rescan_request());

    app.on_key(KeyEvent::new_with_kind(
        KeyCode::Char('r'),
        KeyModifiers::CONTROL,
        crossterm::event::KeyEventKind::Repeat,
    ));
    assert!(!app.take_rescan_request());

    app.set_scanning(false);
    app.on_key(key);
    assert!(app.take_rescan_request());
}

#[test]
fn in_flight_syncs_follow_their_targets_across_navigation() {
    let mut app = App::new(Config::default());
    app.set_current_repo_with_path("acme", "one", None);
    app.set_current_issue(10, 7);
    app.begin_repo_sync();
    app.begin_repo_permissions_sync();
    app.begin_repo_labels_sync();
    app.begin_comment_sync();
    app.begin_pull_request_files_sync();
    app.begin_pull_request_review_comments_sync();
    app.set_pending_issue_action(7, super::super::PendingIssueAction::Closing);

    app.set_current_repo_with_path("acme", "two", None);

    assert!(!app.syncing());
    assert!(!app.repo_permissions_syncing());
    assert!(!app.repo_labels_syncing());
    assert!(!app.comment_syncing());
    assert!(!app.pull_request_files_syncing());
    assert!(!app.pull_request_review_comments_syncing());
    assert_eq!(app.pending_issue_badge(7), None);

    app.set_current_repo_with_path("ACME", "One", None);
    app.set_current_issue(10, 7);
    assert!(app.syncing());
    assert!(app.repo_permissions_syncing());
    assert!(app.repo_labels_syncing());
    assert!(app.comment_syncing());
    assert!(app.pull_request_files_syncing());
    assert!(app.pull_request_review_comments_syncing());
}

#[test]
fn pending_issue_actions_survive_navigation_and_clear_for_the_original_repo() {
    let mut app = App::new(Config::default());
    app.set_current_repo_with_path("Acme", "One", None);
    app.set_pending_issue_action(7, super::super::PendingIssueAction::Closing);

    app.set_current_repo_with_path("Other", "One", None);
    assert_eq!(app.pending_issue_badge(7), None);
    app.set_pending_issue_action(7, super::super::PendingIssueAction::UpdatingLabels);

    app.set_current_repo_with_path("aCME", "ONE", None);
    assert_eq!(app.pending_issue_badge(7), Some("closing"));

    app.set_current_repo_with_path("Other", "One", None);
    app.clear_pending_issue_action("ACME", "one", 7);
    assert_eq!(app.pending_issue_badge(7), Some("updating labels"));

    app.set_current_repo_with_path("Acme", "One", None);
    assert_eq!(app.pending_issue_badge(7), None);
}

#[test]
fn repository_aliases_follow_repeated_renames_without_cycles() {
    let mut app = App::new(Config::default());
    app.set_current_repo_with_path("acme", "one", None);
    app.begin_repo_permissions_sync();
    app.set_pending_issue_action(7, super::super::PendingIssueAction::Closing);
    for repo in ["two", "three", "one"] {
        let requested = app.current_repo().expect("repo").to_string();
        app.resolve_repository_alias(
            "acme",
            &requested,
            &crate::store::RepoRow {
                id: 1,
                owner: "acme".into(),
                name: repo.into(),
                updated_at: None,
                etag: None,
            },
        );
        assert_eq!(app.current_repo(), Some(repo));
        assert!(app.repo_permissions_syncing());
        assert_eq!(app.pending_issue_badge(7), Some("closing"));
    }
    for alias in ["one", "two", "three"] {
        assert!(app.repository_is_current("ACME", alias));
    }
    app.finish_repo_permissions_sync("acme", "one");
    app.clear_pending_issue_action("acme", "one", 7);
    assert!(!app.repo_permissions_syncing());
    assert_eq!(app.pending_issue_badge(7), None);
}

#[test]
fn aliased_requests_keep_independent_completion_guards() {
    let mut app = App::new(Config::default());
    for repo in ["old", "new"] {
        app.set_current_repo_with_path("acme", repo, None);
        app.begin_repo_sync();
        app.begin_repo_permissions_sync();
        app.begin_repo_labels_sync();
        app.set_pending_issue_action(7, super::super::PendingIssueAction::Closing);
    }
    app.set_current_repo_with_path("acme", "old", None);
    app.resolve_repository_alias(
        "acme",
        "old",
        &crate::store::RepoRow {
            id: 1,
            owner: "acme".into(),
            name: "new".into(),
            updated_at: None,
            etag: None,
        },
    );
    app.finish_repo_sync("acme", "old");
    app.finish_repo_permissions_sync("acme", "old");
    app.finish_repo_labels_sync("acme", "old");
    app.clear_pending_issue_action("acme", "old", 7);
    assert!(app.syncing());
    assert!(app.repo_permissions_syncing());
    assert!(app.repo_labels_syncing());
    assert_eq!(app.pending_issue_badge(7), Some("closing"));
    app.finish_repo_sync("acme", "new");
    app.finish_repo_permissions_sync("acme", "new");
    app.finish_repo_labels_sync("acme", "new");
    app.clear_pending_issue_action("acme", "new", 7);
    assert!(!app.syncing());
    assert!(!app.repo_permissions_syncing());
    assert!(!app.repo_labels_syncing());
    assert_eq!(app.pending_issue_badge(7), None);
}

#[test]
fn ctrl_g_resets_repo_picker_query_when_reopened() {
    let mut app = App::new(Config::default());
    app.set_repos(vec![
        LocalRepoRow {
            path: "/tmp/one".to_string(),
            remote_name: "origin".to_string(),
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            url: "https://github.com/acme/blippy.git".to_string(),
            last_seen: None,
            last_scanned: None,
        },
        LocalRepoRow {
            path: "/tmp/two".to_string(),
            remote_name: "origin".to_string(),
            owner: "other".to_string(),
            repo: "core".to_string(),
            url: "https://github.com/other/core.git".to_string(),
            last_seen: None,
            last_scanned: None,
        },
    ]);

    app.set_view(View::Issues);
    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for ch in "acme".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
    assert_eq!(app.repo_query(), "acme");
    assert_eq!(app.filtered_repo_rows().len(), 1);

    app.set_view(View::Issues);
    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));

    assert_eq!(app.repo_query(), "");
    assert_eq!(app.filtered_repo_rows().len(), 2);
    assert!(!app.repo_search_mode());
}

#[test]
fn l_triggers_edit_labels_action() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 1,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: vec!["bug".to_string()],
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    assert_eq!(app.take_action(), Some(AppAction::EditLabels));
}

#[test]
fn shift_a_triggers_edit_assignees_action_in_detail() {
    let mut app = App::new(Config::default());
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 1,
        state: "open".to_string(),
        title: "Issue".to_string(),
        body: String::new(),
        labels: Vec::new(),
        assignees: "alex".to_string(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);
    app.set_current_issue(1, 1);
    app.set_view(View::IssueDetail);

    app.on_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
    assert_eq!(app.take_action(), Some(AppAction::EditAssignees));
}

#[test]
fn shift_a_triggers_edit_assignees_action_in_issues() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
    assert_eq!(app.take_action(), Some(AppAction::EditAssignees));
}

#[test]
fn label_picker_enter_submits_selection_unchanged() {
    for selected in [vec![], vec!["bug".to_string()]] {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "docs".to_string()],
            &selected,
        );
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
        assert_eq!(app.selected_labels(), selected);
    }
}

#[test]
fn assignee_picker_enter_submits_selection_unchanged() {
    for selected in ["", "alex"] {
        let mut app = App::new(Config::default());
        app.open_assignee_picker(
            View::Issues,
            vec!["alex".to_string(), "sam".to_string()],
            selected,
        );
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
        assert_eq!(app.selected_assignees_csv(), selected);
    }
}

#[test]
fn label_picker_space_then_enter_preserves_additions_and_removals() {
    for (selected, expected) in [
        (vec![], vec!["bug".to_string()]),
        (
            vec!["bug".to_string(), "docs".to_string()],
            vec!["docs".to_string()],
        ),
    ] {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "docs".to_string()],
            &selected,
        );
        app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
        assert_eq!(app.selected_labels(), expected);
    }
}

#[test]
fn assignee_picker_space_then_enter_preserves_additions_and_removals() {
    for (selected, expected) in [("", "alex"), ("alex,sam", "sam")] {
        let mut app = App::new(Config::default());
        app.open_assignee_picker(
            View::Issues,
            vec!["alex".to_string(), "sam".to_string()],
            selected,
        );
        app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
        assert_eq!(app.selected_assignees_csv(), expected);
    }
}

#[test]
fn label_picker_space_toggles_highlighted_option() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec!["bug".to_string(), "docs".to_string()],
        &[],
    );

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

    assert_eq!(app.label_query(), "");
    assert_eq!(app.selected_label_option(), 1);
    assert_eq!(app.selected_labels(), vec!["docs".to_string()]);
}

#[test]
fn assignee_picker_space_toggles_highlighted_option() {
    let mut app = App::new(Config::default());
    app.open_assignee_picker(
        View::Issues,
        vec!["alex".to_string(), "sam".to_string()],
        "",
    );

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

    assert_eq!(app.assignee_query(), "");
    assert_eq!(app.selected_assignee_option(), 1);
    assert_eq!(app.selected_assignees(), vec!["sam".to_string()]);
}

#[test]
fn label_picker_type_filter_can_match_c_prefix() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec![
            "bug".to_string(),
            "customer".to_string(),
            "docs".to_string(),
        ],
        &[],
    );

    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

    assert_eq!(app.label_query(), "c");
    assert_eq!(app.filtered_label_indices().len(), 2);
    assert_eq!(app.selected_label_option(), app.filtered_label_indices()[0]);
}

#[test]
fn merge_label_options_dedupes_case_insensitive() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec!["bug".to_string(), "Docs".to_string()],
        &[],
    );

    app.merge_label_options(vec![
        "docs".to_string(),
        "enhancement".to_string(),
        "BUG".to_string(),
    ]);

    assert_eq!(app.label_options(), &["bug", "Docs", "enhancement"]);
}

#[test]
fn merge_assignee_options_dedupes_case_insensitive() {
    let mut app = App::new(Config::default());
    app.open_assignee_picker(
        View::Issues,
        vec!["alex".to_string(), "Sam".to_string()],
        "",
    );

    app.merge_assignee_options(vec![
        "sam".to_string(),
        "jordan".to_string(),
        "ALEX".to_string(),
    ]);

    assert_eq!(app.assignee_options(), &["alex", "jordan", "Sam"]);
}

#[test]
fn merge_label_options_preserves_highlight() {
    for query in ["", "n"] {
        for labels in [
            vec!["bug", "enhancement"],
            vec!["BUG", "ENHANCEMENT", "announcement"],
        ] {
            let mut app = App::new(Config::default());
            app.open_label_picker(
                View::Issues,
                vec!["bug".into(), "enhancement".into()],
                &["bug".into()],
            );
            for ch in query.chars() {
                app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            }
            app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            assert_eq!(
                app.label_options()[app.selected_label_option()],
                "enhancement"
            );

            app.merge_label_options(labels.into_iter().map(String::from).collect());

            assert_eq!(app.label_query(), query);
            assert_eq!(app.selected_labels(), vec!["bug"]);
            assert_eq!(
                app.label_options()[app.selected_label_option()],
                "enhancement"
            );
            app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
            assert_eq!(app.selected_labels(), vec!["bug", "enhancement"]);
        }
    }
}

#[test]
fn merge_assignee_options_preserves_highlight() {
    for query in ["", "a"] {
        for assignees in [vec!["alex", "sam"], vec!["ALEX", "SAM", "aaron"]] {
            let mut app = App::new(Config::default());
            app.open_assignee_picker(View::Issues, vec!["alex".into(), "sam".into()], "alex");
            for ch in query.chars() {
                app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            }
            app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            assert_eq!(
                app.assignee_options()[app.selected_assignee_option()],
                "sam"
            );

            app.merge_assignee_options(assignees.into_iter().map(String::from).collect());

            assert_eq!(app.assignee_query(), query);
            assert_eq!(app.selected_assignees_csv(), "alex");
            assert_eq!(
                app.assignee_options()[app.selected_assignee_option()],
                "sam"
            );
            app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
            assert_eq!(app.selected_assignees_csv(), "alex,sam");
        }
    }
}

#[test]
fn merge_label_options_highlights_first_match_when_none_was_visible() {
    let mut app = App::new(Config::default());
    app.open_label_picker(View::Issues, vec![], &[]);
    app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    app.merge_label_options(vec!["bug".into()]);
    assert!(app.filtered_label_indices().is_empty());
    app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(app.selected_labels().is_empty());

    app.merge_label_options(vec!["documentation".into(), "Docs".into()]);

    assert_eq!(app.label_query(), "d");
    assert_eq!(app.label_options()[app.selected_label_option()], "Docs");
}

#[test]
fn merge_assignee_options_highlights_first_match_when_none_was_visible() {
    let mut app = App::new(Config::default());
    app.open_assignee_picker(View::Issues, vec![], "");
    app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    app.merge_assignee_options(vec!["alex".into()]);
    assert!(app.filtered_assignee_indices().is_empty());
    app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(app.selected_assignees().is_empty());

    app.merge_assignee_options(vec!["sara".into(), "Sam".into()]);

    assert_eq!(app.assignee_query(), "s");
    assert_eq!(
        app.assignee_options()[app.selected_assignee_option()],
        "Sam"
    );
}

#[test]
fn back_from_expanded_diff_returns_to_split_review() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        "head-a".to_string(),
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
        }],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.pull_request_diff_expanded());

    app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    assert_eq!(app.view(), View::PullRequestFiles);
    assert!(!app.pull_request_diff_expanded());
}

#[test]
fn linked_issue_none_does_not_clear_cached_link() {
    let mut app = App::new(Config::default());
    app.set_linked_issue_for_pull_request(42, Some(7));
    app.set_linked_issue_for_pull_request(42, None);

    assert_eq!(app.linked_issue_for_pull_request(42), Some(7));
}

#[test]
fn linked_pull_request_none_does_not_clear_cached_link() {
    let mut app = App::new(Config::default());
    app.set_linked_pull_request(7, Some(42));
    app.set_linked_pull_request(7, None);

    assert_eq!(app.linked_pull_request_for_issue(7), Some(42));
}

#[test]
fn linked_pull_request_multi_preserves_all_candidates() {
    let mut app = App::new(Config::default());
    app.set_linked_pull_requests(7, vec![42, 84], true);

    assert_eq!(app.linked_pull_request_for_issue(7), Some(42));
    assert_eq!(app.linked_pull_requests_for_issue(7), vec![42, 84]);
}

#[test]
fn linked_issue_multi_preserves_all_candidates() {
    let mut app = App::new(Config::default());
    app.set_linked_issues_for_pull_request(42, vec![7, 9], true);

    assert_eq!(app.linked_issue_for_pull_request(42), Some(7));
    assert_eq!(app.linked_issues_for_pull_request(42), vec![7, 9]);
}

#[test]
fn linked_picker_keyboard_flow_selects_item() {
    let mut app = App::new(Config::default());
    app.set_view(View::IssueDetail);
    app.open_linked_picker(
        View::IssueDetail,
        LinkedPickerTarget::PullRequestTui,
        vec![11, 22],
    );

    assert_eq!(app.view(), View::LinkedPicker);
    assert_eq!(app.linked_picker_numbers(), vec![11, 22]);
    assert_eq!(app.selected_linked_picker_index(), 0);

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.selected_linked_picker_index(), 1);

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.take_action(), Some(AppAction::PickLinkedItem));
}

#[test]
fn linked_picker_escape_returns_to_previous_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::IssueComments);
    app.open_linked_picker(
        View::IssueComments,
        LinkedPickerTarget::IssueBrowser,
        vec![101, 102],
    );

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(app.view(), View::IssueComments);
}

#[test]
fn linked_picker_labels_include_cached_titles() {
    let mut app = App::new(Config::default());
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 22,
        state: "open".to_string(),
        title: "Fix flaky sync test".to_string(),
        body: String::new(),
        labels: Vec::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.open_linked_picker(
        View::IssueDetail,
        LinkedPickerTarget::PullRequestTui,
        vec![22],
    );

    assert_eq!(app.linked_picker_labels(), vec!["#22  Fix flaky sync test"]);
}

#[test]
fn linked_picker_captures_origin_from_selected_pull_request() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![IssueRow {
        id: 5,
        repo_id: 1,
        number: 9,
        state: "open".to_string(),
        title: "PR source".to_string(),
        body: String::new(),
        labels: Vec::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);

    app.open_linked_picker(View::Issues, LinkedPickerTarget::IssueTui, vec![101, 102]);

    assert_eq!(
        app.linked_picker_origin(),
        Some((9, WorkItemMode::PullRequests))
    );
}

#[test]
fn linked_picker_origin_restores_pull_request_context_on_back() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);
    app.set_issues(vec![
        IssueRow {
            id: 5,
            repo_id: 1,
            number: 9,
            state: "open".to_string(),
            title: "PR source".to_string(),
            body: String::new(),
            labels: Vec::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        },
        IssueRow {
            id: 6,
            repo_id: 1,
            number: 101,
            state: "open".to_string(),
            title: "Linked issue".to_string(),
            body: String::new(),
            labels: Vec::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);

    app.open_linked_picker(View::Issues, LinkedPickerTarget::IssueTui, vec![101, 102]);
    app.apply_linked_picker_navigation_origin();
    app.clear_linked_picker_state();

    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::Issues);
    assert!(app.select_issue_by_number(101));
    let (issue_id, issue_number) = app
        .selected_issue_row()
        .map(|issue| (issue.id, issue.number))
        .expect("selected issue");
    app.set_current_issue(issue_id, issue_number);
    app.set_view(View::IssueDetail);

    app.back_from_issue_detail();

    assert_eq!(app.view(), View::Issues);
    assert_eq!(app.work_item_mode(), WorkItemMode::PullRequests);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(9));
}

#[test]
fn transient_status_clears_after_expiry() {
    let mut app = App::new(Config::default());

    app.set_transient_status(
        "Opened in browser".to_string(),
        std::time::Duration::from_millis(0),
    );
    assert_eq!(app.status(), "Opened in browser");

    assert!(app.clear_status_if_expired());
    assert_eq!(app.status(), "");
    assert!(!app.clear_status_if_expired());
}

#[test]
fn persistent_status_does_not_clear_from_expiry_check() {
    let mut app = App::new(Config::default());

    app.set_status("Sync failed".to_string());
    assert!(!app.clear_status_if_expired());

    assert_eq!(app.status(), "Sync failed");
}
