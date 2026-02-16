use super::*;

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
        labels: "bug".to_string(),
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
        labels: String::new(),
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
fn labels_picker_enter_submits() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec!["bug".to_string(), "triage".to_string()],
        "bug,triage",
    );

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
}

#[test]
fn labels_picker_enter_selects_highlighted_when_none_selected() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec!["bug".to_string(), "docs".to_string()],
        "",
    );

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
    assert_eq!(app.selected_labels(), vec!["bug".to_string()]);
}

#[test]
fn labels_picker_enter_adds_highlighted_when_existing_labels_present() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec!["bug".to_string(), "docs".to_string()],
        "bug",
    );

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
    assert_eq!(
        app.selected_labels(),
        vec!["bug".to_string(), "docs".to_string()]
    );
}

#[test]
fn assignee_picker_enter_selects_highlighted_when_none_selected() {
    let mut app = App::new(Config::default());
    app.open_assignee_picker(
        View::Issues,
        vec!["alex".to_string(), "sam".to_string()],
        "",
    );

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
    assert_eq!(app.selected_assignees(), vec!["alex".to_string()]);
}

#[test]
fn assignee_picker_enter_adds_highlighted_when_existing_assignees_present() {
    let mut app = App::new(Config::default());
    app.open_assignee_picker(
        View::Issues,
        vec!["alex".to_string(), "sam".to_string()],
        "alex",
    );

    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
    assert_eq!(
        app.selected_assignees(),
        vec!["alex".to_string(), "sam".to_string()]
    );
}

#[test]
fn labels_picker_enter_removes_highlighted_when_already_selected() {
    let mut app = App::new(Config::default());
    app.open_label_picker(
        View::Issues,
        vec!["bug".to_string(), "docs".to_string()],
        "bug,docs",
    );

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
    assert_eq!(app.selected_labels(), vec!["docs".to_string()]);
}

#[test]
fn assignee_picker_enter_removes_highlighted_when_already_selected() {
    let mut app = App::new(Config::default());
    app.open_assignee_picker(
        View::Issues,
        vec!["alex".to_string(), "sam".to_string()],
        "alex,sam",
    );

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
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
        "",
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
        "",
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
fn back_from_expanded_diff_returns_to_split_review() {
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
    app.set_linked_pull_requests(7, vec![42, 84]);

    assert_eq!(app.linked_pull_request_for_issue(7), Some(42));
    assert_eq!(app.linked_pull_requests_for_issue(7), vec![42, 84]);
}

#[test]
fn linked_issue_multi_preserves_all_candidates() {
    let mut app = App::new(Config::default());
    app.set_linked_issues_for_pull_request(42, vec![7, 9]);

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
        labels: String::new(),
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
        labels: String::new(),
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
            labels: String::new(),
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
            labels: String::new(),
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
