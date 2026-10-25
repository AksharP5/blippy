use super::main_actions::issue_url;
use crate::app::{EditorMode, PendingIssueAction, View, WorkItemMode};
use crate::config::Config;
use crate::store::{IssueRow, LocalRepoRow};
use std::sync::mpsc::channel;

#[path = "tests/editor_submission.rs"]
mod editor_submission;

#[test]
fn worker_events_request_one_redraw_per_batch() {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    let (event_tx, event_rx) = channel();
    assert!(!super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap());

    event_tx
        .send(super::AppEvent::IssueCreateFailed {
            repo: super::RepoIdentity::new("other", "repo", None),
            message: "Offscreen failure".to_string(),
        })
        .unwrap();
    assert!(super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap());
    assert!(!super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap());

    event_tx
        .send(super::AppEvent::ScanFailed {
            message: "Scan unavailable".to_string(),
        })
        .unwrap();
    assert!(super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap());
    assert_eq!(app.status(), "Scan failed: Scan unavailable");
    assert!(!super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap());
}

#[test]
fn completed_issue_actions_preserve_the_selected_item() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    for action in ["close", "merge", "labels", "assignees", "reopen"] {
        let mut app = crate::app::App::new(Config::default());
        app.set_current_repo_with_path("acme", "blippy", None);
        app.set_view(View::Issues);
        if action == "merge" {
            app.set_work_item_mode(WorkItemMode::PullRequests);
        }
        app.set_issues(
            (1..=3)
                .map(|number| IssueRow {
                    id: number,
                    repo_id: 1,
                    number,
                    state: if action == "reopen" && number == 3 {
                        "closed"
                    } else {
                        "open"
                    }
                    .into(),
                    title: format!("Issue {number}"),
                    body: "Long description\n".repeat(20),
                    labels: vec!["bug".into()],
                    assignees: "alex".into(),
                    comments_count: 0,
                    updated_at: None,
                    is_pr: action == "merge",
                })
                .collect(),
        );
        let query = match action {
            "labels" => "label:bug",
            "assignees" => "assignee:alex",
            _ => "",
        };
        if !query.is_empty() {
            for ch in format!("/{query}").chars() {
                app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            }
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        assert!(app.select_issue_by_number(2));
        app.set_issues_preview_max_scroll(10);
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.issues_preview_scroll(), 1);
        let repo = super::RepoIdentity::new("acme", "blippy", None);
        let event = match action {
            "labels" => super::AppEvent::IssueLabelsUpdated {
                repo,
                issue_number: 3,
                labels: Vec::new(),
            },
            "assignees" => super::AppEvent::IssueAssigneesUpdated {
                repo,
                issue_number: 3,
                assignees: "sam".into(),
            },
            _ => super::AppEvent::IssueUpdated {
                repo,
                issue_number: 3,
                message: match action {
                    "reopen" => "reopened",
                    "merge" => "merged",
                    _ => "closed",
                }
                .into(),
            },
        };
        let (event_tx, event_rx) = channel();
        event_tx
            .send(event)
            .expect("completed action on previous item");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle completion");

        assert_eq!(
            app.selected_issue_row().map(|issue| issue.number),
            Some(2),
            "{action} completion changed selection"
        );
        assert_eq!(app.issues_preview_scroll(), 1);
        app.on_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        super::main_actions::handle_actions(
            &mut app,
            &mut crate::clipboard::SystemClipboard::default(),
            &conn,
            "token",
            event_tx.clone(),
        )
        .expect("open comment editor for selected item");
        assert_eq!(app.current_issue_number(), Some(2));
        assert_eq!(app.view(), View::CommentEditor);

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        event_tx
            .send(super::AppEvent::IssueUpdated {
                repo: super::RepoIdentity::new("acme", "blippy", None),
                issue_number: 2,
                message: "closed".into(),
            })
            .expect("selected item leaves the open filter");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle removal");
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(1));
        assert_eq!(app.issues_preview_scroll(), 0);
    }
}

#[test]
fn repository_scan_updates_preserve_the_selected_checkout_and_remote() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    for filtered in [false, true] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let rows = [
            ("/work/main", "origin", "blippy", "2026-10-22"),
            ("/work/review", "origin", "blippy", "2026-10-21"),
            ("/work/unrelated", "origin", "other", "2026-10-20"),
        ]
        .map(|(path, remote_name, repo, last_seen)| LocalRepoRow {
            path: path.into(),
            remote_name: remote_name.into(),
            owner: "acme".into(),
            repo: repo.into(),
            url: format!("https://github.com/acme/{repo}.git"),
            last_seen: Some(last_seen.into()),
            last_scanned: None,
        });
        for row in &rows {
            crate::store::upsert_local_repo(&conn, row).expect("cache checkout");
        }
        let mut app = crate::app::App::new(Config::default());
        app.set_view(View::RepoPicker);
        app.set_repos(super::main_data::load_repos(&conn).expect("cached repos"));
        if filtered {
            for ch in "/blippy".chars() {
                app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            }
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.selected_repo_target().unwrap().2, "/work/review");

        crate::store::upsert_local_repo(
            &conn,
            &LocalRepoRow {
                remote_name: "upstream".into(),
                last_seen: Some("2026-10-23".into()),
                ..rows[1].clone()
            },
        )
        .expect("scan discovers another remote");
        let (event_tx, event_rx) = channel();
        event_tx
            .send(super::AppEvent::ReposUpdated)
            .expect("scan update");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle scan");

        let selected = app.filtered_repo_rows()[app.selected_repo()];
        assert_eq!(selected.path, "/work/review");
        assert_eq!(selected.remote_name, "origin");
        assert_eq!(app.repo_query(), if filtered { "blippy" } else { "" });
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        super::main_actions::handle_actions(
            &mut app,
            &mut crate::clipboard::SystemClipboard::default(),
            &conn,
            "token",
            event_tx,
        )
        .expect("open selected checkout");
        assert_eq!(app.current_repo_path(), Some("/work/review"));
    }
}

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
fn preset_save_failures_keep_the_draft_and_allow_retry() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;

    const CONFIG_ENV: &str = "BLIPPY_TEST_PRESET_CONFIG";
    let Some(config_dir) = std::env::var_os(CONFIG_ENV) else {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("timestamp")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("blippy-preset-save-{nanos}"));
        fs::create_dir_all(dir.join("blippy/config.toml")).expect("block config writes");
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "tests::preset_save_failures_keep_the_draft_and_allow_retry",
            ])
            .env(CONFIG_ENV, &dir)
            .env("XDG_CONFIG_HOME", &dir)
            .output()
            .expect("isolated preset test");
        fs::remove_dir_all(&dir).expect("remove test config");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    };

    let config_path = std::path::PathBuf::from(config_dir).join("blippy/config.toml");
    let config = Config {
        theme: Some("midnight".into()),
        comment_defaults: vec![crate::config::CommentDefault {
            name: "existing".into(),
            body: "Existing response".into(),
        }],
        ..Config::default()
    };
    let mut app = crate::app::App::new(config);
    app.editor_mut().reset_for_preset_name();
    for ch in "resolved".chars() {
        app.editor_mut().append_name(ch);
    }
    for ch in "Closing as resolved.".chars() {
        app.editor_mut().append_text(ch);
    }
    app.set_view(View::CommentEditor);
    app.set_selected_preset(3);
    let conn = rusqlite::Connection::open_in_memory().expect("conn");

    for writable in [false, true] {
        if writable {
            fs::remove_dir(&config_path).expect("allow config writes");
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let (event_tx, _) = channel();
        super::main_actions::handle_actions(
            &mut app,
            &mut crate::clipboard::SystemClipboard::default(),
            &conn,
            "token",
            event_tx,
        )
        .expect("keep the TUI running after preset save");

        if !writable {
            assert_eq!(app.view(), View::CommentEditor);
            assert!(app.status().starts_with("Preset save failed:"));
            assert_eq!(app.editor().name(), "resolved");
            assert_eq!(app.editor().text(), "Closing as resolved.");
            assert_eq!(app.comment_defaults().len(), 1);
            assert_eq!(app.selected_preset(), 3);
            continue;
        }

        assert_eq!(app.view(), View::CommentPresetPicker);
        assert_eq!(app.status(), "Preset saved");
        assert_eq!(app.comment_defaults().len(), 2);
        assert_eq!(app.selected_preset(), 0);
        let saved = Config::load().expect("saved config");
        assert_eq!(saved.theme.as_deref(), Some("midnight"));
        assert_eq!(saved.comment_defaults, app.comment_defaults());
        assert_eq!(saved.comment_defaults[1].name, "resolved");
        assert_eq!(saved.comment_defaults[1].body, "Closing as resolved.");
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
            repo: super::RepoIdentity::new("ACME", "One", None),
            message: "old".into(),
        },
        super::AppEvent::RepoLabelsFailed {
            repo: super::RepoIdentity::new("ACME", "One", None),
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
            repo: super::RepoIdentity::new("acme", "blippy", None),
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
    app.set_current_repo_id(1);
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
            repo: super::RepoIdentity::new("acme", "blippy", Some(1)),
            can_edit_issue_metadata: true,
            can_merge_pull_request: true,
        })
        .expect("old permission request completed");
    event_tx
        .send(super::AppEvent::RepoLabelsSuggested {
            repo: super::RepoIdentity::new("acme", "blippy", Some(1)),
            labels: vec![("bug".into(), "ff0000".into())],
        })
        .expect("old label request completed");
    event_tx
        .send(super::AppEvent::IssueUpdated {
            repo: super::RepoIdentity::new("acme", "blippy", Some(1)),
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
fn recreated_repository_sync_replaces_the_current_tui_context() {
    for target_is_pr in [true, false] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = linked_navigation_app(target_is_pr);
        app.set_current_repo_id(1);
        app.set_repo_issue_metadata_editable(Some(true));
        app.set_repo_pull_request_mergeable(Some(true));
        app.set_pending_issue_action(7, PendingIssueAction::Closing);
        app.set_comments(vec![crate::store::CommentRow {
            id: 10,
            issue_id: 1,
            author: "alex".into(),
            body: "Previous repository comment".into(),
            created_at: None,
            last_accessed_at: None,
        }]);
        let replacement = crate::store::RepoRow {
            id: 2,
            owner: "Acme".into(),
            name: "blippy".into(),
            updated_at: None,
            etag: None,
        };
        crate::store::upsert_repo(&conn, &replacement).expect("replacement repository cache");
        let issue = IssueRow {
            id: 100,
            repo_id: 2,
            title: "Replacement repository issue".into(),
            ..app.issues()[0].clone()
        };
        crate::store::upsert_issue(&conn, &issue).expect("replacement issue");
        let linked_item = IssueRow {
            id: 101,
            repo_id: 2,
            title: "Replacement linked item".into(),
            ..app.issues()[1].clone()
        };
        crate::store::upsert_issue(&conn, &linked_item).expect("replacement linked item");
        app.set_issues(vec![app.issues()[0].clone()]);
        let previous_issues = app.issues().to_vec();
        app.begin_repo_sync();
        app.begin_repo_permissions_sync();
        app.begin_repo_labels_sync();
        let (event_tx, event_rx) = channel();
        let old_request = super::RepoIdentity::new("acme", "blippy", app.current_repo_id());
        let linked_event = if target_is_pr {
            super::AppEvent::LinkedPullRequestResolved {
                repo: old_request,
                issue_number: 7,
                pull_requests: vec![(8, String::new())],
                target: super::LinkedPullRequestTarget::Tui,
            }
        } else {
            super::AppEvent::LinkedIssueResolved {
                repo: old_request,
                pull_number: 7,
                issues: crate::github::LinkedIssues {
                    issues: vec![(8, String::new())],
                    incomplete_reason: None,
                },
                target: super::LinkedIssueTarget::Tui,
            }
        };
        event_tx.send(linked_event).expect("old linked response");
        super::main_events::handle_events(&mut app, &conn, &event_rx)
            .expect("linked response before sync finishes");

        assert_eq!(app.current_repo_id(), Some(1));
        assert_eq!(app.issues(), previous_issues);
        assert_eq!(app.current_issue_id(), Some(1));
        assert_eq!(app.comments()[0].id, 10);
        assert_eq!(app.repo_issue_metadata_editable(), Some(true));
        assert_eq!(app.repo_pull_request_mergeable(), Some(true));
        event_tx
            .send(super::AppEvent::SyncFinished {
                owner: "acme".into(),
                repo: "blippy".into(),
                resolved_repo: replacement,
                stats: Default::default(),
            })
            .expect("replacement sync completed");

        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("replace context");

        assert_eq!(app.issues(), &[linked_item, issue]);
        assert_eq!(app.current_repo_id(), Some(2));
        assert_eq!(app.pending_issue_badge(7), None);
        assert_eq!(app.current_issue_id(), None);
        assert!(app.comments().is_empty());
        assert!(app.linked_pull_requests_for_issue(7).is_empty());
        assert!(app.linked_issues_for_pull_request(7).is_empty());
        assert_eq!(app.repo_issue_metadata_editable(), None);
        assert_eq!(app.repo_pull_request_mergeable(), None);
        assert_eq!(app.view(), View::Issues);
        assert_eq!(app.issue_query(), "");
        assert!(!app.has_assignee_filter());
        assert!(!app.syncing());
        assert!(!app.repo_permissions_syncing());
        assert!(!app.repo_labels_syncing());
        assert!(app.status().starts_with("Synced"));
    }
}

#[test]
fn repository_metadata_waits_for_a_known_repository_id() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    let (event_tx, event_rx) = channel();

    super::main_sync::maybe_start_repo_permissions_sync(&mut app, "token", event_tx.clone());
    super::main_sync::maybe_start_repo_labels_sync(&mut app, "token", event_tx);

    assert!(app.take_repo_permissions_sync_request());
    assert!(app.take_repo_labels_sync_request());
    assert!(!app.repo_permissions_syncing());
    assert!(!app.repo_labels_syncing());
    assert!(event_rx.try_recv().is_err());
}

#[test]
fn recreated_repository_rejects_old_completions_without_clearing_new_pending() {
    for action in [
        "close",
        "labels",
        "assignees",
        "failure",
        "metadata",
        "links",
    ] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = linked_navigation_app(true);
        app.set_current_repo_id(1);
        app.set_pending_issue_action(7, PendingIssueAction::Closing);
        app.begin_repo_permissions_sync();
        app.begin_repo_labels_sync();
        let old_request = super::RepoIdentity::new("acme", "blippy", app.current_repo_id());
        let replacement = crate::store::RepoRow {
            id: 2,
            owner: "acme".into(),
            name: "blippy".into(),
            updated_at: None,
            etag: None,
        };
        crate::store::upsert_repo(&conn, &replacement).expect("replacement repository");
        let issue = IssueRow {
            id: 100,
            repo_id: 2,
            ..app.issues()[0].clone()
        };
        crate::store::upsert_issue(&conn, &issue).expect("replacement issue");
        let (event_tx, event_rx) = channel();
        event_tx
            .send(super::AppEvent::SyncFinished {
                owner: "acme".into(),
                repo: "blippy".into(),
                resolved_repo: replacement,
                stats: Default::default(),
            })
            .unwrap();
        super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap();
        app.set_pending_issue_action(7, PendingIssueAction::UpdatingLabels);
        app.begin_repo_permissions_sync();
        app.begin_repo_labels_sync();
        app.set_repo_issue_metadata_editable(Some(true));
        app.set_repo_pull_request_mergeable(Some(true));
        if action == "metadata" {
            app.open_assignee_picker(View::Issues, vec!["current".into()], "");
        }
        if action == "links" {
            assert!(app.begin_linked_pull_request_lookup(7));
            assert!(app.begin_linked_issue_lookup(7));
        }
        app.set_status("New repository action");
        app.take_sync_request();
        let events = match action {
            "labels" => vec![super::AppEvent::IssueLabelsUpdated {
                repo: old_request,
                issue_number: 7,
                labels: vec!["stale".into()],
            }],
            "assignees" => vec![super::AppEvent::IssueAssigneesUpdated {
                repo: old_request,
                issue_number: 7,
                assignees: "stale".into(),
            }],
            "metadata" => vec![
                super::AppEvent::RepoPermissionsResolved {
                    repo: old_request.clone(),
                    can_edit_issue_metadata: false,
                    can_merge_pull_request: false,
                },
                super::AppEvent::RepoPermissionsFailed {
                    repo: old_request.clone(),
                    message: "old".into(),
                },
                super::AppEvent::RepoLabelsSuggested {
                    repo: old_request.clone(),
                    labels: vec![("stale".into(), "ff0000".into())],
                },
                super::AppEvent::RepoLabelsFailed {
                    repo: old_request.clone(),
                    message: "old".into(),
                },
                super::AppEvent::RepoAssigneesFailed {
                    repo: old_request.clone(),
                    message: "old".into(),
                },
                super::AppEvent::RepoAssigneesSuggested {
                    repo: old_request,
                    assignees: vec!["stale".into()],
                },
            ],
            "links" => vec![
                super::AppEvent::LinkedPullRequestResolved {
                    repo: old_request.clone(),
                    issue_number: 7,
                    pull_requests: vec![(8, String::new())],
                    target: super::LinkedPullRequestTarget::Probe,
                },
                super::AppEvent::LinkedIssueResolved {
                    repo: old_request.clone(),
                    pull_number: 7,
                    issues: crate::github::LinkedIssues {
                        issues: vec![(8, String::new())],
                        incomplete_reason: None,
                    },
                    target: super::LinkedIssueTarget::Probe,
                },
                super::AppEvent::LinkedPullRequestLookupFailed {
                    repo: old_request.clone(),
                    issue_number: 7,
                    message: "old".into(),
                    target: super::LinkedPullRequestTarget::Probe,
                },
                super::AppEvent::LinkedIssueLookupFailed {
                    repo: old_request,
                    pull_number: 7,
                    message: "old".into(),
                    target: super::LinkedIssueTarget::Probe,
                },
            ],
            _ => vec![super::AppEvent::IssueUpdated {
                repo: old_request,
                issue_number: 7,
                message: if action == "close" {
                    "closed"
                } else {
                    "close failed: old"
                }
                .into(),
            }],
        };
        for event in events {
            event_tx.send(event).unwrap();
        }
        super::main_events::handle_events(&mut app, &conn, &event_rx).unwrap();

        assert_eq!(app.issues(), &[issue], "{action}");
        assert_eq!(
            app.pending_issue_badge(7),
            Some("updating labels"),
            "{action}"
        );
        assert_eq!(app.status(), "New repository action", "{action}");
        assert!(app.repo_permissions_syncing(), "{action}");
        assert!(app.repo_labels_syncing(), "{action}");
        assert_eq!(app.repo_issue_metadata_editable(), Some(true), "{action}");
        assert_eq!(app.repo_pull_request_mergeable(), Some(true), "{action}");
        assert_eq!(app.repo_label_color("stale"), None, "{action}");
        assert!(app.linked_pull_requests_for_issue(7).is_empty(), "{action}");
        assert!(app.linked_issues_for_pull_request(7).is_empty(), "{action}");
        if action == "metadata" {
            assert_eq!(app.assignee_options(), &["current"], "{action}");
        }
        if action == "links" {
            assert!(!app.begin_linked_pull_request_lookup(7));
            assert!(!app.begin_linked_issue_lookup(7));
        }
        assert!(!app.take_sync_request(), "{action}");
    }
}

#[test]
fn repository_replacement_requires_the_matching_cache_and_missing_old_identity() {
    for (cached_id, keep_old_identity) in [(3, false), (2, true)] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = linked_navigation_app(true);
        app.set_current_repo_id(1);
        let previous_issues = app.issues().to_vec();
        let resolved = crate::store::RepoRow {
            id: 2,
            owner: "acme".into(),
            name: "blippy".into(),
            updated_at: None,
            etag: None,
        };
        crate::store::upsert_repo(
            &conn,
            &crate::store::RepoRow {
                id: cached_id,
                ..resolved.clone()
            },
        )
        .expect("current cache identity");
        if keep_old_identity {
            crate::store::upsert_repo(
                &conn,
                &crate::store::RepoRow {
                    id: 1,
                    name: "renamed".into(),
                    ..resolved.clone()
                },
            )
            .expect("old repository still exists");
        }
        let (event_tx, event_rx) = channel();
        event_tx
            .send(super::AppEvent::SyncFinished {
                owner: "acme".into(),
                repo: "blippy".into(),
                resolved_repo: resolved,
                stats: Default::default(),
            })
            .expect("late sync event");

        super::main_events::handle_events(&mut app, &conn, &event_rx)
            .expect("ignore stale identity");

        assert_eq!(app.current_repo_id(), Some(1));
        assert_eq!(app.issues(), previous_issues);
        assert_eq!(app.current_issue_id(), Some(1));
        assert_eq!(app.view(), View::IssueDetail);
    }
}

#[test]
fn late_sync_does_not_select_a_repository_after_its_old_name_is_reused() {
    for (cached, requested) in [(false, "new"), (true, "new"), (true, "old")] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = crate::app::App::new(Config::default());
        let renamed = crate::store::RepoRow {
            id: 1,
            owner: "acme".into(),
            name: "new".into(),
            updated_at: None,
            etag: None,
        };
        crate::store::upsert_repo(&conn, &renamed).expect("renamed repository");
        let (event_tx, event_rx) = channel();
        app.set_current_repo_with_path("acme", "old", None);
        event_tx
            .send(super::AppEvent::SyncFinished {
                owner: "acme".into(),
                repo: "old".into(),
                resolved_repo: renamed.clone(),
                stats: Default::default(),
            })
            .expect("rename completed");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("learn rename");
        app.begin_repo_sync();

        if cached {
            let reused = crate::store::RepoRow {
                id: 2,
                name: "old".into(),
                ..renamed.clone()
            };
            crate::store::upsert_repo(&conn, &reused).expect("different repository at old name");
        }
        super::main_data::load_issues_for_slug(&mut app, &conn, "acme", "old", None)
            .expect("select old name");
        event_tx
            .send(super::AppEvent::SyncFinished {
                owner: "acme".into(),
                repo: requested.into(),
                resolved_repo: renamed,
                stats: Default::default(),
            })
            .expect("late sync completed");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle late sync");
        assert_eq!(app.current_repo(), Some("old"));
        assert!(app.issues().is_empty());
        assert!(!app.status().starts_with("Synced"));
    }
}

#[test]
fn successful_github_mutations_survive_cache_write_failures() {
    for action in ["create", "edit", "delete"] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = linked_navigation_app(true);
        crate::store::upsert_repo(
            &conn,
            &crate::store::RepoRow {
                id: 1,
                owner: "acme".into(),
                name: "blippy".into(),
                updated_at: None,
                etag: None,
            },
        )
        .expect("repo");
        crate::store::upsert_issue(&conn, &app.issues()[0]).expect("issue");
        let comment = crate::store::CommentRow {
            id: 50,
            issue_id: 1,
            author: "alex".into(),
            body: "before".into(),
            created_at: None,
            last_accessed_at: None,
        };
        crate::store::upsert_comment(&conn, &comment).expect("comment");
        app.set_comments(vec![comment]);
        conn.pragma_update(None, "query_only", true)
            .expect("read-only cache");
        let repo = super::RepoIdentity::new("acme", "blippy", None);
        let event = match action {
            "create" => super::AppEvent::IssueCreated {
                repo,
                issue: app.issues()[1].clone(),
            },
            "edit" => super::AppEvent::IssueCommentUpdated {
                repo,
                issue_number: 7,
                comment_id: 50,
                body: "after".into(),
            },
            _ => super::AppEvent::IssueCommentDeleted {
                repo,
                issue_number: 7,
                issue_id: 1,
                comment_id: 50,
            },
        };
        let (event_tx, event_rx) = channel();
        event_tx.send(event).expect("successful GitHub mutation");
        event_tx
            .send(super::AppEvent::RepoPermissionsResolved {
                repo: super::RepoIdentity::new("acme", "blippy", None),
                can_edit_issue_metadata: true,
                can_merge_pull_request: true,
            })
            .expect("subsequent event");
        super::main_events::handle_events(&mut app, &conn, &event_rx)
            .expect("cache failures must not close the app");
        assert_eq!(app.repo_issue_metadata_editable(), Some(true));
        assert!(app.status().contains("cache update failed"));
        assert!(app.take_sync_request());
        if action == "edit" {
            assert_eq!(app.comments()[0].body, "after");
            assert!(app.take_comment_sync_request());
        }
        if action == "delete" {
            assert!(app.comments().is_empty());
            assert!(app.take_comment_sync_request());
        }
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
        app.set_pull_request_files(1, "head-a".into(), Vec::new());
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
                commits: serde_json::from_value(serde_json::json!({
                    "head": {"sha": "head-a"}, "base": {"sha": ""}
                }))
                .expect("diff commits"),
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
                commits: serde_json::from_value(serde_json::json!({
                    "head": {"sha": "head-a"}, "base": {"sha": ""}
                }))
                .expect("diff commits"),
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
            commits: serde_json::from_value(serde_json::json!({
                "head": {"sha": "head-a"}, "base": {"sha": ""}
            }))
            .expect("diff commits"),
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
            commits: serde_json::from_value(serde_json::json!({
                "head": {"sha": "head-a"}, "base": {"sha": ""}
            }))
            .expect("diff commits"),
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
    app.set_current_repo_id(1);
    app.set_pending_issue_action(7, PendingIssueAction::Closing);
    app.set_current_repo_with_path("acme", "other", None);
    app.set_current_repo_id(2);
    app.set_pending_issue_action(7, PendingIssueAction::Merging);
    app.set_status("Other repository");

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::IssueUpdated {
            repo: super::RepoIdentity::new("acme", "blippy", Some(1)),
            issue_number: 7,
            message: "closed".to_string(),
        })
        .expect("offscreen completion");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle completion");

    assert_eq!(app.pending_issue_badge(7), Some("merging"));
    assert_eq!(app.status(), "Other repository");
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_current_repo_id(1);
    assert_eq!(app.pending_issue_badge(7), None);
}

fn metadata_picker_app(view: View) -> crate::app::App {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_repo_issue_metadata_editable(Some(true));
    let is_pr = view == View::PullRequestFiles;
    if is_pr {
        app.set_work_item_mode(WorkItemMode::PullRequests);
    }
    app.set_issues(
        [
            (7, vec!["bug".into()], "alex"),
            (8, vec!["docs".into(), "urgent".into()], "sam,taylor"),
        ]
        .into_iter()
        .map(|(number, labels, assignees)| IssueRow {
            id: number,
            repo_id: 1,
            number,
            state: "open".into(),
            title: format!("Issue {number}"),
            body: String::new(),
            labels,
            assignees: assignees.into(),
            comments_count: 0,
            updated_at: None,
            is_pr,
        })
        .collect(),
    );
    assert!(app.select_issue_by_number(7));
    app.set_current_issue(7, 7);
    app.set_view(view);
    app
}

#[test]
fn pending_issue_actions_keep_metadata_pickers_closed() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    for view in [
        View::Issues,
        View::IssueDetail,
        View::IssueComments,
        View::PullRequestFiles,
    ] {
        for pending in [
            PendingIssueAction::Closing,
            PendingIssueAction::UpdatingLabels,
            PendingIssueAction::UpdatingAssignees,
        ] {
            for key in [
                KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT),
            ] {
                if view == View::PullRequestFiles && key.code == KeyCode::Char('l') {
                    continue;
                }
                for (width, height) in [(80, 24), (120, 36)] {
                    let mut app = metadata_picker_app(view);
                    if view == View::Issues {
                        app.set_current_issue(8, 8);
                    }
                    let current_issue = app.current_issue_id();
                    app.set_pending_issue_action(7, pending);
                    let status = format!("#7 is already {}", app.pending_issue_badge(7).unwrap());
                    app.set_status(&status);
                    app.take_repo_labels_sync_request();
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    terminal
                        .draw(|frame| crate::ui::draw(frame, &mut app))
                        .unwrap();
                    let before = terminal.backend().buffer().clone();

                    app.on_key(key);
                    let (event_tx, _event_rx) = channel();
                    super::main_actions::handle_actions(
                        &mut app,
                        &mut crate::clipboard::SystemClipboard::default(),
                        &conn,
                        "\n",
                        event_tx,
                    )
                    .expect("reject picker entry during pending action");

                    assert_eq!(app.view(), view, "{key:?}, {pending:?}");
                    assert_eq!(app.current_issue_id(), current_issue);
                    assert_eq!(app.status(), status);
                    assert!(!app.take_repo_labels_sync_request());
                    terminal
                        .draw(|frame| crate::ui::draw(frame, &mut app))
                        .unwrap();
                    assert_eq!(terminal.backend().buffer(), &before);
                }
            }
        }
    }
}

#[test]
fn label_picker_preserves_completed_updates() {
    assert_metadata_picker_preserves_completed_updates(false);
}

#[test]
fn assignee_picker_preserves_completed_updates() {
    assert_metadata_picker_preserves_completed_updates(true);
}

fn assert_metadata_picker_preserves_completed_updates(assignees: bool) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let edit_key = if assignees {
        KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)
    } else {
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)
    };
    let (first_addition, second_addition) = if assignees {
        ("sam", "taylor")
    } else {
        ("urgent", "docs")
    };
    for view in [
        View::Issues,
        View::IssueDetail,
        View::IssueComments,
        View::PullRequestFiles,
    ] {
        // Review uses l to select the right diff side.
        if view == View::PullRequestFiles && !assignees {
            continue;
        }
        let mut app = metadata_picker_app(view);
        app.set_pending_issue_action(8, PendingIssueAction::UpdatingLabels);
        let (worker_tx, _worker_rx) = channel();
        let mut clipboard = crate::clipboard::SystemClipboard::default();
        let mut handle_action = |app: &mut crate::app::App| {
            // Invalid authorization prevents network I/O; completions are queued below.
            super::main_actions::handle_actions(
                app,
                &mut clipboard,
                &conn,
                "\n",
                worker_tx.clone(),
            )
            .expect("handle metadata action");
        };
        app.on_key(edit_key);
        handle_action(&mut app);
        assert_eq!(
            app.view(),
            if assignees {
                View::AssigneePicker
            } else {
                View::LabelPicker
            }
        );
        for ch in first_addition.chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert_eq!(
            if assignees {
                app.selected_assignees()
            } else {
                app.selected_labels()
            },
            if assignees {
                vec!["alex", "sam"]
            } else {
                vec!["bug", "urgent"]
            },
        );
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        handle_action(&mut app);
        assert_eq!(app.view(), view);
        assert_eq!(
            app.pending_issue_badge(7),
            Some(if assignees {
                "updating assignees"
            } else {
                "updating labels"
            })
        );

        app.on_key(edit_key);
        handle_action(&mut app);
        let pending_view = app.view();
        let repo = super::RepoIdentity::new("acme", "blippy", None);
        let completion = if assignees {
            super::AppEvent::IssueAssigneesUpdated {
                repo,
                issue_number: 7,
                assignees: "alex,sam".into(),
            }
        } else {
            super::AppEvent::IssueLabelsUpdated {
                repo,
                issue_number: 7,
                labels: vec!["bug".into(), "urgent".into()],
            }
        };
        let (event_tx, event_rx) = channel();
        event_tx.send(completion).expect("first save completed");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle completion");
        assert_eq!(app.pending_issue_badge(7), None);
        assert_eq!(app.pending_issue_badge(8), Some("updating labels"));
        if app.view() == view {
            app.on_key(edit_key);
            handle_action(&mut app);
        }
        for ch in second_addition.chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        let selected = if assignees {
            app.selected_assignees()
        } else {
            app.selected_labels()
        };
        assert_eq!(
            selected,
            if assignees {
                vec!["alex", "sam", "taylor"]
            } else {
                vec!["bug", "docs", "urgent"]
            },
            "source={view:?}, pending view={pending_view:?}"
        );
        for (width, height) in [(80, 24), (120, 36)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| crate::ui::draw(frame, &mut app))
                .unwrap();
            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                rendered.contains(&format!("[x] {first_addition}")),
                "{rendered}"
            );
            assert!(
                rendered.contains(&format!("[x] {second_addition}")),
                "{rendered}"
            );
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            app.take_action(),
            Some(if assignees {
                crate::app::AppAction::SubmitAssignees
            } else {
                crate::app::AppAction::SubmitLabels
            })
        );
    }
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
    app.set_linked_pull_requests(7, vec![42, 43], true);

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
    app.set_linked_issues_for_pull_request(9, vec![100, 101], true);

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
            repo: super::RepoIdentity::new("acme", "blippy", None),
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
            repo: super::RepoIdentity::new("acme", "previous", None),
            issue_number: 92,
            message: "closed".to_string(),
        })
        .expect("send event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle events");

    assert_eq!(app.issues()[0].state, "open");
    assert_ne!(app.status(), "#92 closed");
}

#[test]
fn failed_link_probe_retries_only_after_refresh() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    assert!(app.begin_linked_pull_request_lookup(7));

    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::LinkedPullRequestLookupFailed {
            repo: super::RepoIdentity::new("acme", "blippy", None),
            issue_number: 7,
            message: "offline".to_string(),
            target: super::LinkedPullRequestTarget::Probe,
        })
        .expect("send event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle events");

    assert!(!app.linked_pull_request_known(7));
    assert!(!app.begin_linked_pull_request_lookup(7));
    app.set_view(View::Issues);
    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(app.begin_linked_pull_request_lookup(7));
}

#[test]
fn partial_linked_issues_remain_visible_and_refresh_discovers_missing_links() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
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
    app.set_current_issue(21, 9);
    app.set_view(View::IssueDetail);
    assert!(app.begin_linked_issue_lookup(9));
    // The reverse association must not complete an in-flight lookup for the issue.
    assert!(app.begin_linked_pull_request_lookup(100));
    let (event_tx, event_rx) = channel();
    event_tx
        .send(super::AppEvent::LinkedIssueResolved {
            repo: super::RepoIdentity::new("acme", "blippy", None),
            pull_number: 9,
            issues: crate::github::LinkedIssues {
                issues: vec![(100, "https://github.com/acme/blippy/issues/100".to_string())],
                incomplete_reason: Some("Timeline unavailable".to_string()),
            },
            target: super::LinkedIssueTarget::Probe,
        })
        .expect("send partial result");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle result");

    assert_eq!(app.linked_issues_for_pull_request(9), vec![100]);
    assert!(!app.linked_issue_known(9));
    assert!(!app.begin_linked_issue_lookup(9));
    assert!(!app.begin_linked_pull_request_lookup(100));
    assert!(app.status().contains("Timeline unavailable"));
    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(app.begin_linked_issue_lookup(9));
    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(
        !app.begin_linked_issue_lookup(9),
        "refresh must preserve in-flight lookup"
    );
    event_tx
        .send(super::AppEvent::LinkedIssueResolved {
            repo: super::RepoIdentity::new("acme", "blippy", None),
            pull_number: 9,
            issues: crate::github::LinkedIssues {
                issues: vec![
                    (100, "https://github.com/acme/blippy/issues/100".to_string()),
                    (101, "https://github.com/acme/blippy/issues/101".to_string()),
                ],
                incomplete_reason: None,
            },
            target: super::LinkedIssueTarget::Probe,
        })
        .expect("send complete result");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle result");
    assert!(app.linked_issue_known(9));
    assert_eq!(app.linked_issues_for_pull_request(9), vec![100, 101]);
    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(!app.begin_linked_issue_lookup(9));
    assert!(
        super::main_linked_actions::try_open_cached_linked_issue(
            &mut app,
            &conn,
            super::LinkedIssueTarget::Browser,
        )
        .expect("complete cache usable")
    );
    assert_eq!(app.linked_picker_numbers(), vec![100, 101]);
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
            commits: serde_json::from_value(serde_json::json!({
                "head": {"sha": "head-a"}, "base": {"sha": ""}
            }))
            .expect("diff commits"),
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

fn cached_linked_navigation_app(target_is_pr: bool) -> (rusqlite::Connection, crate::app::App) {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
    let mut app = linked_navigation_app(target_is_pr);
    let mut issues = app.issues().to_vec();
    issues.push(IssueRow {
        id: 3,
        number: 6,
        ..issues[0].clone()
    });
    app.set_issues(issues);
    crate::store::upsert_repo(
        &conn,
        &crate::store::RepoRow {
            id: 1,
            owner: "acme".into(),
            name: "blippy".into(),
            updated_at: None,
            etag: None,
        },
    )
    .expect("cache repo");
    for issue in app.issues() {
        crate::store::upsert_issue(&conn, issue).expect("cache issue");
    }
    (conn, app)
}

fn linked_navigation_response(
    target: crate::app::LinkedPickerTarget,
    numbers: &[i64],
) -> super::AppEvent {
    use crate::app::LinkedPickerTarget;

    let repo = super::RepoIdentity::new("acme", "blippy", None);
    let items = numbers
        .iter()
        .map(|number| (*number, String::new()))
        .collect();
    match target {
        LinkedPickerTarget::PullRequestTui | LinkedPickerTarget::PullRequestBrowser => {
            super::AppEvent::LinkedPullRequestResolved {
                repo,
                issue_number: 7,
                pull_requests: items,
                target: if target == LinkedPickerTarget::PullRequestTui {
                    super::LinkedPullRequestTarget::Tui
                } else {
                    super::LinkedPullRequestTarget::Browser
                },
            }
        }
        LinkedPickerTarget::IssueTui | LinkedPickerTarget::IssueBrowser => {
            super::AppEvent::LinkedIssueResolved {
                repo,
                pull_number: 7,
                issues: crate::github::LinkedIssues {
                    issues: items,
                    incomplete_reason: None,
                },
                target: if target == LinkedPickerTarget::IssueTui {
                    super::LinkedIssueTarget::Tui
                } else {
                    super::LinkedIssueTarget::Browser
                },
            }
        }
    }
}

#[test]
fn late_linked_results_preserve_comment_drafts() {
    use crate::app::LinkedPickerTarget;

    for target in [
        LinkedPickerTarget::PullRequestTui,
        LinkedPickerTarget::IssueTui,
        LinkedPickerTarget::PullRequestBrowser,
        LinkedPickerTarget::IssueBrowser,
    ] {
        let target_is_pr = matches!(
            target,
            LinkedPickerTarget::PullRequestTui | LinkedPickerTarget::PullRequestBrowser
        );
        for numbers in [&[8][..], &[8, 9][..]] {
            if numbers.len() == 1
                && matches!(
                    target,
                    LinkedPickerTarget::PullRequestBrowser | LinkedPickerTarget::IssueBrowser
                )
            {
                continue;
            }
            let (conn, mut app) = cached_linked_navigation_app(target_is_pr);
            let response = linked_navigation_response(target, numbers);
            app.open_issue_comment_editor(View::IssueDetail);
            for ch in "Keep this draft".chars() {
                app.editor_mut().append_text(ch);
            }
            let (event_tx, event_rx) = channel();
            event_tx.send(response).expect("delayed linked response");
            super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle response");

            assert_eq!(app.view(), View::CommentEditor, "{target:?}: {numbers:?}");
            assert_eq!(app.editor().text(), "Keep this draft");
            assert_eq!(app.current_issue_number(), Some(7));
            assert_eq!(app.issue_query(), "#7");
            assert!(app.has_assignee_filter());
            if target_is_pr {
                assert!(app.linked_pull_request_known(7));
                assert_eq!(app.linked_pull_requests_for_issue(7), numbers);
            } else {
                assert!(app.linked_issue_known(7));
                assert_eq!(app.linked_issues_for_pull_request(7), numbers);
            }

            app.on_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ));
            let opened = if target_is_pr {
                super::main_linked_actions::try_open_cached_linked_pull_request(
                    &mut app,
                    &conn,
                    super::LinkedPullRequestTarget::Tui,
                )
            } else {
                super::main_linked_actions::try_open_cached_linked_issue(
                    &mut app,
                    &conn,
                    super::LinkedIssueTarget::Tui,
                )
            }
            .expect("explicit navigation uses the saved response");
            assert!(opened);
            assert_eq!(
                app.view(),
                if numbers.len() == 1 {
                    View::IssueDetail
                } else {
                    View::LinkedPicker
                }
            );
        }
    }
}

#[test]
fn late_linked_results_preserve_changed_contexts() {
    use crate::app::LinkedPickerTarget;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    for target_is_pr in [true, false] {
        let target = if target_is_pr {
            LinkedPickerTarget::PullRequestTui
        } else {
            LinkedPickerTarget::IssueTui
        };
        for numbers in [&[8][..], &[8, 9][..]] {
            for context in [
                "different item",
                "different mode",
                "search",
                "help",
                "picker",
                "repositories",
            ] {
                let (conn, mut app) = cached_linked_navigation_app(target_is_pr);
                let response = linked_navigation_response(target, numbers);
                match context {
                    "different item" => {
                        app.reveal_issue_by_number(6, app.work_item_mode());
                        app.set_current_issue(3, 6);
                    }
                    "different mode" => {
                        app.set_view(View::Issues);
                        app.reveal_issue_by_number(
                            8,
                            if target_is_pr {
                                WorkItemMode::PullRequests
                            } else {
                                WorkItemMode::Issues
                            },
                        );
                    }
                    "search" => {
                        app.set_view(View::Issues);
                        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
                    }
                    "help" => app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
                    "picker" => app.open_label_picker(
                        View::IssueDetail,
                        vec!["bug".into()],
                        &["bug".into()],
                    ),
                    "repositories" => app.set_view(View::RepoPicker),
                    _ => unreachable!(),
                }
                let view = app.view();
                let source = app.current_issue_number();
                let mode = app.work_item_mode();
                let query = app.issue_query().to_string();
                let (event_tx, event_rx) = channel();
                event_tx.send(response).expect("delayed linked response");
                super::main_events::handle_events(&mut app, &conn, &event_rx)
                    .expect("handle response");

                assert_eq!(app.view(), view, "{context}: {target:?}: {numbers:?}");
                assert_eq!(app.current_issue_number(), source);
                assert_eq!(app.work_item_mode(), mode);
                assert_eq!(app.issue_query(), query);
                assert_eq!(app.issue_search_mode(), context == "search");
                assert_eq!(app.help_overlay_visible(), context == "help");
                if context == "picker" {
                    assert_eq!(app.selected_labels(), vec!["bug"]);
                }
            }
        }
    }
}

#[test]
fn linked_results_navigate_from_an_active_browsing_source() {
    use crate::app::LinkedPickerTarget;

    for target_is_pr in [true, false] {
        let target = if target_is_pr {
            LinkedPickerTarget::PullRequestTui
        } else {
            LinkedPickerTarget::IssueTui
        };
        for numbers in [&[8][..], &[8, 9][..]] {
            for view in [
                View::Issues,
                View::IssueDetail,
                View::IssueComments,
                View::PullRequestFiles,
            ] {
                if target_is_pr && view == View::PullRequestFiles {
                    continue;
                }
                let (conn, mut app) = cached_linked_navigation_app(target_is_pr);
                app.set_view(view);
                let (event_tx, event_rx) = channel();
                event_tx
                    .send(linked_navigation_response(target, numbers))
                    .expect("linked response");
                super::main_events::handle_events(&mut app, &conn, &event_rx)
                    .expect("handle response");

                if numbers.len() > 1 {
                    assert_eq!(app.view(), View::LinkedPicker);
                    assert_eq!(app.linked_picker_numbers(), numbers);
                    assert_eq!(app.current_issue_number(), Some(7));
                } else {
                    assert_eq!(app.view(), View::IssueDetail);
                    assert_eq!(app.current_issue_number(), Some(8));
                }
            }
        }
    }
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
fn partial_and_inferred_links_open_cached_targets_without_github() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
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
    for target_is_pr in [true, false] {
        for inferred in [true, false] {
            let mut app = linked_navigation_app(target_is_pr);
            for issue in app.issues() {
                crate::store::upsert_issue(&conn, issue).expect("cache target");
            }
            match (target_is_pr, inferred) {
                (true, true) => app.set_linked_issues_for_pull_request(8, vec![7], true),
                (true, false) => app.set_linked_pull_requests(7, vec![8], false),
                (false, true) => app.set_linked_pull_requests(8, vec![7], true),
                (false, false) => app.set_linked_issues_for_pull_request(7, vec![8], false),
            }
            let opened = if target_is_pr {
                assert!(!app.linked_pull_request_known(7));
                assert!(app.selected_issue_has_known_linked_pr());
                super::main_linked_actions::try_open_cached_linked_pull_request(
                    &mut app,
                    &conn,
                    super::LinkedPullRequestTarget::Tui,
                )
            } else {
                assert!(!app.linked_issue_known(7));
                assert!(app.selected_pull_request_has_known_linked_issue());
                super::main_linked_actions::try_open_cached_linked_issue(
                    &mut app,
                    &conn,
                    super::LinkedIssueTarget::Tui,
                )
            }
            .expect("open saved link");
            assert!(opened, "saved links must not require a GitHub lookup");
            assert_eq!(app.current_issue_number(), Some(8));
            assert_eq!(app.view(), View::IssueDetail);
        }
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
fn issue_creation_preserves_an_active_editor_or_picker() {
    for picker in [false, true] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = linked_navigation_app(true);
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
        for issue in app.issues() {
            crate::store::upsert_issue(&conn, issue).expect("cache issue");
        }
        let mut created = app.issues()[1].clone();
        created.is_pr = false;
        created.state = "open".to_string();

        if picker {
            app.open_label_picker(View::IssueDetail, vec!["bug".to_string()], &[]);
            app.on_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(' '),
                crossterm::event::KeyModifiers::NONE,
            ));
        } else {
            app.open_issue_comment_editor(View::IssueDetail);
            for ch in "Keep this draft".chars() {
                app.editor_mut().append_text(ch);
            }
        }
        let view = app.view();
        let (event_tx, event_rx) = channel();
        event_tx
            .send(super::AppEvent::IssueCreated {
                repo: super::RepoIdentity::new("acme", "blippy", app.current_repo_id()),
                issue: created.clone(),
            })
            .expect("earlier issue creation completes");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle creation");

        assert_eq!(
            app.view(),
            view,
            "an earlier creation must not close active input"
        );
        assert_eq!(app.current_issue_number(), Some(7));
        assert_eq!(app.issue_query(), "#7");
        assert!(app.has_assignee_filter());
        if picker {
            assert_eq!(app.selected_labels(), vec!["bug"]);
        } else {
            assert_eq!(app.editor().text(), "Keep this draft");
        }
        assert_eq!(app.status(), "Created issue #8");
        assert!(app.take_sync_request());
        assert!(
            crate::store::list_issues(&conn, 1)
                .expect("cached issues")
                .iter()
                .any(|issue| issue.id == created.id && !issue.is_pr)
        );
    }
}

#[test]
fn issue_creation_preserves_search_help_and_pull_request_contexts() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    for (searching, help, mode) in [
        (true, false, WorkItemMode::Issues),
        (false, true, WorkItemMode::Issues),
        (false, false, WorkItemMode::PullRequests),
    ] {
        let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("db");
        let mut app = linked_navigation_app(true);
        crate::store::upsert_repo(
            &conn,
            &crate::store::RepoRow {
                id: 1,
                owner: "acme".into(),
                name: "blippy".into(),
                updated_at: None,
                etag: None,
            },
        )
        .expect("cache repo");
        for issue in app.issues() {
            crate::store::upsert_issue(&conn, issue).expect("cache issue");
        }
        let created = IssueRow {
            id: 3,
            number: 9,
            ..app.issues()[0].clone()
        };

        if mode == WorkItemMode::PullRequests {
            assert!(app.reveal_issue_by_number(8, mode));
            app.set_current_issue(2, 8);
        } else {
            app.set_view(View::Issues);
        }
        if searching {
            app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        }
        if help {
            app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        }
        let previous = (
            app.view(),
            app.work_item_mode(),
            app.issue_query().to_string(),
            app.current_issue_id(),
            app.current_issue_number(),
        );
        let (event_tx, event_rx) = channel();
        event_tx
            .send(super::AppEvent::IssueCreated {
                repo: super::RepoIdentity::new("acme", "blippy", app.current_repo_id()),
                issue: created,
            })
            .expect("earlier issue creation completes");
        super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle creation");

        assert_eq!(
            (
                app.view(),
                app.work_item_mode(),
                app.issue_query().to_string(),
                app.current_issue_id(),
                app.current_issue_number(),
            ),
            previous,
            "searching={searching}, help={help}, mode={mode:?}"
        );
        assert_eq!(app.issue_search_mode(), searching);
        assert_eq!(app.help_overlay_visible(), help);
        assert_eq!(app.status(), "Created issue #9");
        assert!(app.take_sync_request());
    }
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
            repo: super::RepoIdentity::new("acme", "blippy", None),
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
        "head-a".to_string(),
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
            repo: super::RepoIdentity::new("acme", "blippy", None),
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
            repo: super::RepoIdentity::new("acme", "other", None),
            message: "wrong repo".to_string(),
        })
        .expect("unrelated event");
    super::main_events::handle_events(&mut app, &conn, &event_rx)
        .expect("ignore unrelated failure");
    assert!(app.repo_labels_syncing());
    assert!(!app.status().contains("wrong repo"));

    event_tx
        .send(super::AppEvent::RepoLabelsFailed {
            repo: super::RepoIdentity::new("acme", "blippy", None),
            message: "offline".to_string(),
        })
        .expect("failed labels event");
    super::main_events::handle_events(&mut app, &conn, &event_rx).expect("handle labels failure");
    assert!(!app.repo_labels_syncing());
    assert_eq!(app.status(), "Repo labels unavailable: offline");

    event_tx
        .send(super::AppEvent::RepoAssigneesFailed {
            repo: super::RepoIdentity::new("acme", "blippy", None),
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
