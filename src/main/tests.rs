use super::main_actions::issue_url;
use crate::app::{EditorMode, PendingIssueAction, View, WorkItemMode};
use crate::config::Config;
use crate::store::{IssueRow, LocalRepoRow};
use std::sync::mpsc::channel;

#[test]
fn invalid_presets_keep_the_editor_open() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    for (name, body, message) in [
        ("   ", "Closing this issue", "Preset name required"),
        ("close", "   ", "Preset body required"),
    ] {
        let mut app = crate::app::App::new(Config::default());
        app.editor_mut().reset_for_preset_name();
        for ch in name.chars() {
            app.editor_mut().append_name(ch);
        }
        for ch in body.chars() {
            app.editor_mut().append_text(ch);
        }
        app.set_view(View::CommentEditor);
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let (event_tx, _) = channel();
        super::main_actions::handle_actions(
            &mut app,
            &mut crate::clipboard::SystemClipboard::default(),
            &conn,
            "token",
            event_tx,
        )
        .expect("validate preset");

        assert_eq!(app.view(), View::CommentEditor);
        assert_eq!(app.status(), message);
        assert_eq!(app.editor().name(), name);
        assert_eq!(app.editor().text(), body);
        assert!(app.comment_defaults().is_empty());
    }
}

#[test]
fn reopening_the_current_issue_preserves_in_flight_comment_sync() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    for linked in [false, true] {
        let mut app = linked_navigation_app(true);
        app.set_linked_pull_request(7, None);
        app.begin_comment_sync();
        if linked {
            super::main_linked_actions::open_linked_item_in_tui(
                &mut app,
                &conn,
                7,
                WorkItemMode::Issues,
            )
            .expect("reopen linked item");
        } else {
            app.set_view(View::Issues);
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            let (event_tx, _) = channel();
            super::main_actions::handle_actions(
                &mut app,
                &mut crate::clipboard::SystemClipboard::default(),
                &conn,
                "token",
                event_tx,
            )
            .expect("reopen issue");
        }

        assert_eq!(app.view(), View::IssueDetail);
        assert!(app.comment_syncing());
        assert!(app.take_comment_sync_request());
    }
}

#[test]
fn offscreen_sync_completions_release_only_their_original_targets() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    for (repo, id) in [("one", 10), ("two", 20)] {
        app.set_current_repo_with_path("acme", repo, None);
        app.set_current_issue(id, 7);
        app.begin_repo_sync();
        app.begin_repo_permissions_sync();
        app.begin_repo_labels_sync();
        app.begin_comment_sync();
        app.begin_pull_request_files_sync();
        app.begin_pull_request_review_comments_sync();
    }
    let (event_tx, event_rx) = channel();
    for event in [
        super::AppEvent::SyncFailed {
            owner: "ACME".into(),
            repo: "One".into(),
            message: "old".into(),
        },
        super::AppEvent::RepoPermissionsFailed {
            owner: "ACME".into(),
            repo: "One".into(),
            message: "old".into(),
        },
        super::AppEvent::RepoLabelsFailed {
            owner: "ACME".into(),
            repo: "One".into(),
            message: "old".into(),
        },
        super::AppEvent::CommentsFailed {
            issue_id: 10,
            message: "old".into(),
        },
        super::AppEvent::PullRequestFilesFailed {
            issue_id: 10,
            message: "old".into(),
        },
        super::AppEvent::PullRequestReviewCommentsFailed {
            issue_id: 10,
            message: "old".into(),
        },
    ] {
        event_tx.send(event).expect("old completion");
    }
    super::main_events::handle_events(&mut app, &conn, &event_rx)
        .expect("complete offscreen loads");
    assert!(app.syncing());
    assert!(app.repo_permissions_syncing());
    assert!(app.repo_labels_syncing());
    assert!(app.comment_syncing());
    assert!(app.pull_request_files_syncing());
    assert!(app.pull_request_review_comments_syncing());
    assert!(!app.status().contains("old"));

    app.set_current_repo_with_path("acme", "one", None);
    app.set_current_issue(10, 7);
    assert!(!app.syncing());
    assert!(!app.repo_permissions_syncing());
    assert!(!app.repo_labels_syncing());
    assert!(!app.comment_syncing());
    assert!(!app.pull_request_files_syncing());
    assert!(!app.pull_request_review_comments_syncing());
}

#[test]
fn repository_event_identity_is_case_insensitive() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("ACME", "Blippy", None);
    app.begin_repo_permissions_sync();
    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::RepoPermissionsResolved {
            owner: "acme".into(),
            repo: "blippy".into(),
            can_edit_issue_metadata: true,
            can_merge_pull_request: true,
        })
        .expect("permission event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle alias");
    assert!(!app.repo_permissions_syncing());
    assert_eq!(app.repo_issue_metadata_editable(), Some(true));
    assert_eq!(app.repo_pull_request_mergeable(), Some(true));
}

#[test]
fn redirected_repository_sync_keeps_its_cache_and_navigation() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    let mut app = linked_navigation_app(true);
    let resolved = crate::store::RepoRow {
        id: 1,
        owner: "new-acme".into(),
        name: "blippy".into(),
        updated_at: None,
        etag: None,
    };
    crate::store::upsert_repo(&conn, &resolved).expect("canonical repo");
    crate::store::upsert_issue(&conn, &app.issues()[0]).expect("canonical issue cache");
    app.begin_repo_sync();
    app.begin_repo_permissions_sync();
    app.begin_repo_labels_sync();
    app.set_pending_issue_action(7, PendingIssueAction::Closing);
    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::SyncFinished {
            owner: "acme".into(),
            repo: "blippy".into(),
            resolved_repo: resolved.clone(),
            stats: Default::default(),
        })
        .expect("redirected sync completed");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle redirect");
    assert_eq!(app.issues().len(), 1);
    assert_eq!(app.current_owner(), Some("new-acme"));
    assert_eq!(app.current_issue_id(), Some(1));
    assert_eq!(app.view(), View::IssueDetail);
    assert_eq!(app.issue_query(), "#7");
    assert!(app.has_assignee_filter());
    assert!(app.repo_permissions_syncing());
    assert!(app.repo_labels_syncing());
    assert_eq!(app.pending_issue_badge(7), Some("closing"));

    event_tx
        .send(super::AppEvent::RepoPermissionsResolved {
            owner: "acme".into(),
            repo: "blippy".into(),
            can_edit_issue_metadata: true,
            can_merge_pull_request: true,
        })
        .expect("old permission request completed");
    event_tx
        .send(super::AppEvent::RepoLabelsSuggested {
            owner: "acme".into(),
            repo: "blippy".into(),
            labels: vec![("bug".into(), "ff0000".into())],
        })
        .expect("old label request completed");
    event_tx
        .send(super::AppEvent::IssueUpdated {
            repo: super::RepoIdentity::new("acme", "blippy"),
            issue_number: 7,
            message: "closed".into(),
        })
        .expect("old mutation completed");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle old alias events");
    assert!(!app.repo_permissions_syncing());
    assert!(!app.repo_labels_syncing());
    assert_eq!(app.repo_issue_metadata_editable(), Some(true));
    assert_eq!(app.repo_label_color("bug"), Some("ff0000"));
    assert_eq!(app.pending_issue_badge(7), None);
    assert_eq!(app.current_issue_row().expect("same issue").state, "closed");
}

#[test]
fn offscreen_repository_redirect_keeps_pending_operations_visible_under_both_names() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "old", None);
    app.begin_repo_sync();
    app.begin_repo_permissions_sync();
    app.begin_repo_labels_sync();
    app.set_pending_issue_action(7, PendingIssueAction::Closing);
    app.set_current_repo_with_path("other", "repo", None);
    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::SyncFinished {
            owner: "acme".into(),
            repo: "old".into(),
            stats: Default::default(),
            resolved_repo: crate::store::RepoRow {
                id: 1,
                owner: "acme".into(),
                name: "new".into(),
                updated_at: None,
                etag: None,
            },
        })
        .expect("offscreen redirect");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("record alias");
    assert_eq!(app.current_owner(), Some("other"));
    for name in ["old", "new"] {
        app.set_current_repo_with_path("acme", name, None);
        assert!(!app.syncing());
        assert!(app.repo_permissions_syncing());
        assert!(app.repo_labels_syncing());
        assert_eq!(app.pending_issue_badge(7), Some("closing"));
    }
}

#[test]
fn review_edits_and_deletions_refresh_after_an_older_snapshot() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    for delete in [false, true] {
        let mut app = crate::app::App::new(Config::default());
        app.set_current_issue(1, 7);
        let old = crate::app::PullRequestReviewComment {
            id: 10,
            thread_id: Some("thread".into()),
            resolved: false,
            anchored: true,
            path: "src/main.rs".into(),
            line: 1,
            side: crate::app::ReviewSide::Right,
            body: "old".into(),
            author: "alex".into(),
            created_at: None,
        };
        app.set_pull_request_review_comments(vec![old.clone()]);
        app.begin_pull_request_review_comments_sync();
        let (event_tx, event_rx) = channel();
        let completed = if delete {
            super::AppEvent::PullRequestReviewCommentDeleted {
                issue_id: 1,
                comment_id: 10,
            }
        } else {
            super::AppEvent::PullRequestReviewCommentUpdated {
                issue_id: 1,
                comment_id: 10,
                body: "new".into(),
            }
        };
        event_tx.send(completed).expect("mutation completed");
        event_tx
            .send(super::AppEvent::PullRequestReviewCommentsUpdated {
                issue_id: 1,
                comments: vec![old.clone()],
            })
            .expect("older fetch completed later");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle race");
        assert!(app.take_pull_request_review_comments_sync_request());
        assert!(!app.pull_request_review_comments_syncing());

        let fresh = if delete {
            Vec::new()
        } else {
            vec![crate::app::PullRequestReviewComment {
                body: "new".into(),
                ..old
            }]
        };
        event_tx
            .send(super::AppEvent::PullRequestReviewCommentsUpdated {
                issue_id: 1,
                comments: fresh,
            })
            .expect("refresh completed");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("fresh snapshot");
        let comments = app.pull_request_comments_for_path_and_line(
            "src/main.rs",
            crate::app::ReviewSide::Right,
            1,
        );
        assert!(comments.iter().all(|comment| comment.body == "new"));
        assert_eq!(comments.len(), usize::from(!delete));
    }
}

#[test]
fn completed_file_view_changes_refresh_after_an_older_snapshot() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_issue(1, 7);
    app.begin_pull_request_files_sync();
    let file = crate::app::PullRequestFile {
        filename: "src/main.rs".into(),
        status: "modified".into(),
        additions: 1,
        deletions: 1,
        patch: None,
    };
    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::PullRequestFileViewedUpdated {
            issue_id: 1,
            path: file.filename.clone(),
            viewed: true,
        })
        .expect("view mutation completed");
    event_tx
        .send(super::AppEvent::PullRequestFilesUpdated {
            issue_id: 1,
            files: vec![file.clone()],
            pull_request_id: Some("PR".into()),
            viewed_files: Default::default(),
            view_state_error: None,
        })
        .expect("older fetch completed later");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle race");
    assert!(app.take_pull_request_files_sync_request());
    assert!(!app.pull_request_files_syncing());

    event_tx
        .send(super::AppEvent::PullRequestFilesUpdated {
            issue_id: 1,
            files: vec![file],
            pull_request_id: Some("PR".into()),
            viewed_files: ["src/main.rs".to_string()].into(),
            view_state_error: None,
        })
        .expect("refresh completed");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("fresh snapshot");
    assert!(app.pull_request_file_is_viewed("src/main.rs"));
}

#[test]
fn pending_issue_actions_block_conflicting_updates() {
    for action in [
        crate::app::AppAction::SubmitComment,
        crate::app::AppAction::ReopenIssue,
        crate::app::AppAction::MergePullRequest,
        crate::app::AppAction::SubmitLabels,
        crate::app::AppAction::SubmitAssignees,
    ] {
        let mut app = linked_navigation_app(true);
        app.set_pending_issue_action(7, PendingIssueAction::Closing);
        let (event_tx, event_rx) = channel();
        let result = match action {
            crate::app::AppAction::SubmitComment => {
                super::main_action_utils::close_issue_with_comment(
                    &mut app,
                    "token",
                    Some("Closing".to_string()),
                    event_tx,
                )
            }
            crate::app::AppAction::ReopenIssue => {
                super::main_action_utils::reopen_issue(&mut app, "token", event_tx)
            }
            crate::app::AppAction::MergePullRequest => {
                super::main_action_utils::merge_pull_request(&mut app, "token", event_tx)
            }
            crate::app::AppAction::SubmitLabels => super::main_action_utils::update_issue_labels(
                &mut app,
                "token",
                Vec::new(),
                event_tx,
            ),
            crate::app::AppAction::SubmitAssignees => {
                super::main_action_utils::update_issue_assignees(
                    &mut app,
                    "token",
                    Vec::new(),
                    event_tx,
                )
            }
            _ => unreachable!(),
        };
        result.expect("reject conflicting action");
        assert_eq!(app.status(), "#7 is already closing");
        assert_eq!(app.pending_issue_badge(7), Some("closing"));
        assert_eq!(app.view(), View::IssueDetail);
        assert!(event_rx.try_recv().is_err());
    }
}

#[test]
fn offscreen_completion_clears_only_its_repository_action() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_pending_issue_action(7, PendingIssueAction::Closing);
    app.set_current_repo_with_path("acme", "other", None);
    app.set_pending_issue_action(7, PendingIssueAction::Merging);
    app.set_status("Other repository");

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueUpdated {
            repo: super::RepoIdentity::new("acme", "blippy"),
            issue_number: 7,
            message: "closed".to_string(),
        })
        .expect("offscreen completion");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle completion");

    assert_eq!(app.pending_issue_badge(7), Some("merging"));
    assert_eq!(app.status(), "Other repository");
    app.set_current_repo_with_path("acme", "blippy", None);
    assert_eq!(app.pending_issue_badge(7), None);
}

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
        labels: Vec::new(),
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
        labels: Vec::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    let url = issue_url(&app).expect("url");

    assert_eq!(url, "https://github.com/acme/blippy/issues/7");
}

#[test]
fn copy_selected_url_copies_repo_from_picker() {
    let mut app = crate::app::App::new(Config::default());
    app.set_repos(vec![LocalRepoRow {
        path: "/tmp/blippy".to_string(),
        remote_name: "origin".to_string(),
        owner: "acme".to_string(),
        repo: "blippy".to_string(),
        url: "git@github.com:acme/blippy.git".to_string(),
        last_seen: None,
        last_scanned: None,
    }]);
    let mut copied = String::new();

    super::main_actions::copy_selected_url(&mut app, |url| {
        copied = url.to_string();
        Ok(())
    });

    assert_eq!(copied, "https://github.com/acme/blippy");
    assert_eq!(app.status(), "URL copied");
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
        labels: Vec::new(),
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
    let mut clipboard = crate::clipboard::SystemClipboard::default();
    super::main_actions::handle_actions(&mut app, &mut clipboard, &conn, "token", event_tx)
        .expect("handled");

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
        labels: Vec::new(),
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
        labels: Vec::new(),
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
        labels: Vec::new(),
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
        labels: Vec::new(),
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
        labels: Vec::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_pending_issue_action(92, PendingIssueAction::Merging);

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueUpdated {
            repo: super::RepoIdentity::new("acme", "blippy"),
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
fn issue_events_from_another_repo_do_not_change_current_repo() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "current", None);
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 34,
        repo_id: 2,
        number: 92,
        state: "open".to_string(),
        title: "Current repo issue".to_string(),
        body: String::new(),
        labels: Vec::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueUpdated {
            repo: super::RepoIdentity::new("acme", "previous"),
            issue_number: 92,
            message: "closed".to_string(),
        })
        .expect("send event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle events");

    assert_eq!(app.issues()[0].state, "open");
    assert_ne!(app.status(), "#92 closed");
}

#[test]
fn failed_link_probe_is_not_retried_automatically() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    assert!(app.begin_linked_pull_request_lookup(7));

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::LinkedPullRequestLookupFailed {
            repo: super::RepoIdentity::new("acme", "blippy"),
            issue_number: 7,
            message: "offline".to_string(),
            target: super::LinkedPullRequestTarget::Probe,
        })
        .expect("send event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle events");

    assert!(app.linked_pull_request_known(7));
    assert!(!app.begin_linked_pull_request_lookup(7));
}

#[test]
fn pull_request_view_state_failure_stays_unknown() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_current_issue(33, 92);

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::PullRequestFilesUpdated {
            issue_id: 33,
            files: vec![crate::app::PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: None,
            }],
            pull_request_id: None,
            viewed_files: std::collections::HashSet::new(),
            view_state_error: Some("offline".to_string()),
        })
        .expect("send event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle events");

    assert_eq!(app.pull_request_files().len(), 1);
    assert!(!app.pull_request_view_state_loaded());
    assert!(app.selected_pull_request_file_view_toggle().is_none());
    assert!(app.status().contains("view state unavailable"));
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
    let mut clipboard = crate::clipboard::SystemClipboard::default();
    super::main_actions::handle_actions(&mut app, &mut clipboard, &conn, "token", event_tx)
        .expect("handled");

    assert_eq!(app.status(), "Issue title required");
    assert_eq!(app.view(), View::CommentEditor);
}

fn linked_navigation_app(target_is_pr: bool) -> crate::app::App {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_work_item_mode(if target_is_pr {
        WorkItemMode::Issues
    } else {
        WorkItemMode::PullRequests
    });
    app.set_issues(vec![
        IssueRow {
            id: 1,
            repo_id: 1,
            number: 7,
            state: "open".to_string(),
            title: "Source".to_string(),
            body: String::new(),
            labels: Vec::new(),
            assignees: "alex".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: !target_is_pr,
        },
        IssueRow {
            id: 2,
            repo_id: 1,
            number: 8,
            state: "closed".to_string(),
            title: "Linked target".to_string(),
            body: String::new(),
            labels: Vec::new(),
            assignees: "sam".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: target_is_pr,
        },
    ]);
    for ch in "/#7".chars() {
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(ch),
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    app.on_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app.on_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('a'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.issue_query(), "#7");
    assert!(app.has_assignee_filter());
    app.set_current_issue(1, 7);
    app.set_view(View::IssueDetail);
    app
}

#[test]
fn linked_navigation_opens_cached_targets_hidden_by_filters() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    for target_is_pr in [true, false] {
        let mut app = linked_navigation_app(target_is_pr);
        app.capture_linked_navigation_origin();
        let opened = if target_is_pr {
            super::main_linked_actions::open_linked_item_in_tui(
                &mut app,
                &conn,
                8,
                WorkItemMode::PullRequests,
            )
        } else {
            super::main_linked_actions::open_linked_item_in_tui(
                &mut app,
                &conn,
                8,
                WorkItemMode::Issues,
            )
        }
        .expect("navigate");

        assert!(opened);
        assert_eq!(app.view(), View::IssueDetail);
        assert_eq!(app.current_issue_number(), Some(8));
        assert_eq!(app.selected_issue_row().expect("selected").number, 8);
        assert!(app.issue_query().is_empty());
        assert!(!app.has_assignee_filter());

        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.view(), View::Issues);
        assert_eq!(app.selected_issue_row().expect("origin").number, 7);
    }
}

#[test]
fn missing_linked_target_preserves_navigation() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    for target_is_pr in [true, false] {
        let mut app = linked_navigation_app(target_is_pr);
        let mode = app.work_item_mode();
        let filter = app.issue_filter();
        let opened = if target_is_pr {
            super::main_linked_actions::open_linked_item_in_tui(
                &mut app,
                &conn,
                99,
                WorkItemMode::PullRequests,
            )
        } else {
            super::main_linked_actions::open_linked_item_in_tui(
                &mut app,
                &conn,
                99,
                WorkItemMode::Issues,
            )
        }
        .expect("navigate");

        assert!(!opened);
        assert_eq!(app.view(), View::IssueDetail);
        assert_eq!(app.current_issue_number(), Some(7));
        assert_eq!(app.work_item_mode(), mode);
        assert_eq!(app.issue_filter(), filter);
        assert_eq!(app.issue_query(), "#7");
        assert!(app.has_assignee_filter());
    }
}

#[test]
fn checkout_requires_a_local_repository() {
    let mut app = linked_navigation_app(false);
    super::main_action_utils::checkout_pull_request(&mut app).expect("checkout guard");
    assert_eq!(app.status(), "No local repository selected");
}

#[test]
fn created_issue_is_opened_even_when_hidden_by_filters() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    let mut app = linked_navigation_app(true);
    let mut created = app.issues()[1].clone();
    created.is_pr = false;
    created.state = "open".to_string();
    crate::store::upsert_repo(
        &conn,
        &crate::store::RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: None,
            etag: None,
        },
    )
    .expect("cache repo");
    assert!(
        crate::store::list_issues(&conn, 1)
            .expect("cached issues")
            .is_empty()
    );

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueCreated {
            repo: super::RepoIdentity::new("acme", "blippy"),
            issue: created.clone(),
        })
        .expect("created event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle creation");

    assert_eq!(app.view(), View::IssueDetail);
    assert_eq!(app.current_issue_id(), Some(created.id));
    assert_eq!(app.current_issue_number(), Some(created.number));
    assert!(app.issue_query().is_empty());
    assert!(!app.has_assignee_filter());
}

#[test]
fn file_view_updates_block_duplicate_toggles_and_allow_retry_after_failure() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_current_issue(1, 7);
    app.set_pull_request_files(
        1,
        vec![crate::app::PullRequestFile {
            filename: "src/main.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            patch: None,
        }],
    );
    app.set_pull_request_view_state(Some("PR_id".to_string()), Default::default());
    assert!(app.begin_pull_request_file_view_update(1, "src/main.rs"));
    app.set_pull_request_file_viewed("src/main.rs", true);

    let (event_tx, event_rx) = channel();
    super::main_action_utils::toggle_pull_request_file_viewed(&mut app, "token", event_tx.clone())
        .expect("duplicate toggle");
    assert_eq!(app.status(), "Updating view state for src/main.rs");
    assert!(app.pull_request_file_is_viewed("src/main.rs"));
    assert!(event_rx.try_recv().is_err());

    event_tx
        .send(super::AppEvent::PullRequestFileViewedUpdateFailed {
            issue_id: 1,
            path: "src/main.rs".to_string(),
            viewed: true,
            message: "offline".to_string(),
        })
        .expect("failed event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle failure");
    assert!(!app.pull_request_file_is_viewed("src/main.rs"));
    assert!(app.begin_pull_request_file_view_update(1, "src/main.rs"));
}

#[test]
fn comment_deletion_preserves_the_total_when_the_cache_is_partial() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    let mut app = linked_navigation_app(true);
    let mut issue = app.issues()[0].clone();
    issue.comments_count = 10;
    crate::store::upsert_repo(
        &conn,
        &crate::store::RepoRow {
            id: 1,
            owner: "acme".to_string(),
            name: "blippy".to_string(),
            updated_at: None,
            etag: None,
        },
    )
    .expect("cache repo");
    crate::store::upsert_issue(&conn, &issue).expect("cache issue");
    app.set_issues(vec![issue]);
    let comment = crate::store::CommentRow {
        id: 50,
        issue_id: 1,
        author: "alex".to_string(),
        body: "Comment".to_string(),
        created_at: None,
        last_accessed_at: None,
    };
    crate::store::upsert_comment(&conn, &comment).expect("cache one of ten comments");
    app.set_comments(vec![comment]);

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueCommentDeleted {
            repo: super::RepoIdentity::new("acme", "blippy"),
            issue_number: 7,
            issue_id: 1,
            comment_id: 50,
        })
        .expect("deleted event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle deletion");

    assert!(app.comments().is_empty());
    assert_eq!(
        app.current_issue_row()
            .expect("current issue")
            .comments_count,
        9
    );
    assert_eq!(
        crate::store::list_issues(&conn, 1).expect("cached issues")[0].comments_count,
        9
    );
}

#[test]
fn metadata_fetch_failures_report_errors_and_release_the_request() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.begin_repo_labels_sync();
    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::RepoLabelsFailed {
            owner: "acme".to_string(),
            repo: "other".to_string(),
            message: "wrong repo".to_string(),
        })
        .expect("unrelated event");
    super::main_events::handle_events(&mut app, &conn, &event_rx)
        .expect("ignore unrelated failure");
    assert!(app.repo_labels_syncing());
    assert!(!app.status().contains("wrong repo"));

    event_tx
        .send(super::AppEvent::RepoLabelsFailed {
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            message: "offline".to_string(),
        })
        .expect("failed labels event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle labels failure");
    assert!(!app.repo_labels_syncing());
    assert_eq!(app.status(), "Repo labels unavailable: offline");

    event_tx
        .send(super::AppEvent::RepoAssigneesFailed {
            owner: "acme".to_string(),
            repo: "blippy".to_string(),
            message: "forbidden".to_string(),
        })
        .expect("failed assignees event");
    super::main_events::handle_events(&mut app, &conn, &event_rx)
        .expect("handle assignees failure");
    assert_eq!(app.status(), "Repo assignees unavailable: forbidden");
}

#[test]
fn file_view_updates_survive_navigation_and_clear_on_completion() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_current_issue(1, 7);
    assert!(app.begin_pull_request_file_view_update(1, "src/main.rs"));
    assert!(app.begin_pull_request_file_view_update(1, "src/lib.rs"));
    app.set_current_repo_with_path("acme", "other", None);
    app.set_current_issue(2, 8);
    assert!(app.begin_pull_request_file_view_update(2, "src/main.rs"));
    assert!(!app.begin_pull_request_file_view_update(1, "src/main.rs"));

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::PullRequestFileViewedUpdated {
            issue_id: 1,
            path: "src/main.rs".to_string(),
            viewed: true,
        })
        .expect("completed event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle completion");

    assert!(!app.pull_request_file_is_viewed("src/main.rs"));
    assert!(app.begin_pull_request_file_view_update(1, "src/main.rs"));
    assert!(!app.begin_pull_request_file_view_update(1, "src/lib.rs"));
    assert!(!app.begin_pull_request_file_view_update(2, "src/main.rs"));
}
