use super::*;

#[test]
fn enter_toggles_pull_request_diff_expanded_mode() {
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
    assert!(!app.pull_request_diff_expanded());

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.pull_request_diff_expanded());

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.pull_request_diff_expanded());
}

#[test]
fn gg_keeps_pull_request_diff_expanded_mode() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 3,
            deletions: 0,
            patch: Some("@@ -1,1 +1,4 @@\n old\n+one\n+two\n+three".to_string()),
        }],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.pull_request_diff_expanded());

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));

    assert!(app.pull_request_diff_expanded());
    assert_eq!(app.selected_pull_request_diff_line(), 0);
}

#[test]
fn question_mark_toggles_help_overlay() {
    let mut app = App::new(Config::default());
    assert!(!app.help_overlay_visible());

    app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
    assert!(app.help_overlay_visible());

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.help_overlay_visible());
}

#[test]
fn visual_mode_creates_multiline_review_target() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 2,
            deletions: 0,
            patch: Some("@@ -1,1 +1,3 @@\n old\n+new\n+more".to_string()),
        }],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT));
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));

    let target = app.selected_pull_request_review_target().expect("target");
    assert_eq!(target.side, ReviewSide::Right);
    assert_eq!(target.start_line, Some(1));
    assert_eq!(target.line, 2);
}

#[test]
fn l_sets_review_side_to_new_on_context_row() {
    let mut app = App::new(Config::default());
    app.set_view(View::PullRequestFiles);
    app.set_pull_request_files(
        1,
        vec![PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1,2 +1,2 @@\n old\n-old2\n+new2".to_string()),
        }],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    let right_target = app
        .selected_pull_request_review_target()
        .expect("right target");
    assert_eq!(right_target.side, ReviewSide::Right);
}

#[test]
fn e_triggers_edit_comment_action_in_comments_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::IssueComments);
    app.set_comments(vec![CommentRow {
        id: 300,
        issue_id: 20,
        author: "dev".to_string(),
        body: "hello".to_string(),
        created_at: Some("2024-01-02T01:00:00Z".to_string()),
        last_accessed_at: None,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::EditIssueComment));
}

#[test]
fn x_triggers_delete_comment_action_in_comments_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::IssueComments);
    app.set_comments(vec![CommentRow {
        id: 301,
        issue_id: 20,
        author: "dev".to_string(),
        body: "hello".to_string(),
        created_at: Some("2024-01-02T01:00:00Z".to_string()),
        last_accessed_at: None,
    }]);

    app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::DeleteIssueComment));
}

#[test]
fn j_and_k_navigate_comments_in_full_comments_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::IssueComments);
    app.set_comments(vec![
        CommentRow {
            id: 401,
            issue_id: 20,
            author: "dev".to_string(),
            body: "one".to_string(),
            created_at: Some("2024-01-02T01:00:00Z".to_string()),
            last_accessed_at: None,
        },
        CommentRow {
            id: 402,
            issue_id: 20,
            author: "dev".to_string(),
            body: "two".to_string(),
            created_at: Some("2024-01-02T01:01:00Z".to_string()),
            last_accessed_at: None,
        },
    ]);

    assert_eq!(app.selected_comment(), 0);
    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.selected_comment(), 1);

    app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(app.selected_comment(), 0);
}

#[test]
fn enter_submits_edited_comment_editor() {
    let mut app = App::new(Config::default());
    app.open_comment_edit_editor(View::IssueComments, 99, "existing");

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitEditedComment));
}

#[test]
fn slash_search_supports_qualifier_tokens() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 11,
            state: "open".to_string(),
            title: "Auth".to_string(),
            body: String::new(),
            labels: "bug,security".to_string(),
            assignees: "alex".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 22,
            state: "closed".to_string(),
            title: "Docs".to_string(),
            body: String::new(),
            labels: "docs".to_string(),
            assignees: "sam".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);
    app.set_issue_filter(IssueFilter::Closed);

    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for ch in "is:closed label:docs".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }

    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(22));
}

#[test]
fn assignee_qualifier_matches_exact_user() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 11,
            state: "open".to_string(),
            title: "One".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: "alex,sam".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 12,
            state: "open".to_string(),
            title: "Two".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: "samiam".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);

    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for ch in "assignee:sam".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }

    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(11));
}

#[test]
fn is_pr_query_matches_pull_requests() {
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
            number: 12,
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

    app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

    app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for ch in "is:pr".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }

    assert_eq!(app.issues_for_view().len(), 1);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(12));
}

#[test]
fn enter_submits_comment_editor() {
    let mut app = App::new(Config::default());
    app.open_issue_comment_editor(View::Issues);

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitIssueComment));
}

#[test]
fn shift_enter_adds_newline_in_comment_editor() {
    let mut app = App::new(Config::default());
    app.open_issue_comment_editor(View::Issues);
    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));

    assert_eq!(app.editor().text(), "a\nb");
    assert_eq!(app.take_action(), None);
}

#[test]
fn ctrl_j_adds_newline_in_comment_editor() {
    let mut app = App::new(Config::default());
    app.open_issue_comment_editor(View::Issues);
    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
    app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));

    assert_eq!(app.editor().text(), "a\nb");
    assert_eq!(app.take_action(), None);
}

#[test]
fn set_issues_preserves_selected_issue_when_still_present() {
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
            title: "Two".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
    ]);

    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));

    app.set_issues(vec![
        IssueRow {
            id: 10,
            repo_id: 1,
            number: 2,
            state: "open".to_string(),
            title: "Two refreshed".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        },
        IssueRow {
            id: 11,
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

    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));
}

#[test]
fn update_issue_state_rebuilds_filtered_view() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 1,
        repo_id: 1,
        number: 10,
        state: "open".to_string(),
        title: "One".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    assert_eq!(app.issues_for_view().len(), 1);
    app.update_issue_state_by_number(10, "closed");
    assert_eq!(app.issues_for_view().len(), 0);
}

#[test]
fn closed_filter_sorts_by_recently_closed() {
    let mut app = App::new(Config::default());
    app.set_view(View::Issues);
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 10,
            state: "closed".to_string(),
            title: "older close".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            is_pr: false,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 11,
            state: "closed".to_string(),
            title: "newer close".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
            is_pr: false,
        },
    ]);

    app.set_issue_filter(IssueFilter::Closed);
    assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(11));
}

#[test]
fn repo_picker_keeps_distinct_rows() {
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
            remote_name: "upstream".to_string(),
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            url: "https://github.com/acme/blippy.git".to_string(),
            last_seen: None,
            last_scanned: None,
        },
        LocalRepoRow {
            path: "/tmp/three".to_string(),
            remote_name: "origin".to_string(),
            owner: "other".to_string(),
            repo: "core".to_string(),
            url: "https://github.com/other/core.git".to_string(),
            last_seen: None,
            last_scanned: None,
        },
    ]);

    assert_eq!(app.filtered_repo_rows().len(), 3);
    assert_eq!(app.filtered_repo_rows()[0].owner, "acme");
    assert_eq!(app.filtered_repo_rows()[0].repo, "blippy");
    assert_eq!(app.filtered_repo_rows()[1].remote_name, "upstream");
}
