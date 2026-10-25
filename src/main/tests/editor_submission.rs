use super::super::{AppEvent, RepoIdentity};
use super::*;
use crate::app::{App, PullRequestReviewTarget, ReviewSide};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

fn draft(mode: EditorMode) -> App {
    let mut app = linked_navigation_app(mode != EditorMode::AddPullRequestReviewComment);
    match mode {
        EditorMode::CreateIssue => app.open_create_issue_editor(View::Issues),
        EditorMode::AddComment => app.open_issue_comment_editor(View::IssueComments),
        EditorMode::EditComment => app.open_comment_edit_editor(View::IssueComments, 91, ""),
        EditorMode::AddPullRequestReviewComment => app.open_pull_request_review_comment_editor(
            View::PullRequestFiles,
            PullRequestReviewTarget {
                commit_id: "original-commit".into(),
                path: "src/original.rs".into(),
                line: 12,
                side: ReviewSide::Right,
                start_line: Some(10),
                start_side: Some(ReviewSide::Right),
            },
        ),
        EditorMode::EditPullRequestReviewComment => {
            app.open_pull_request_review_comment_edit_editor(View::PullRequestFiles, 92, "")
        }
        _ => unreachable!(),
    }
    app.editor_mut().append_name('T');
    app.on_paste("Draft body\nsecond line");
    if mode == EditorMode::CreateIssue {
        app.editor_mut().focus_create_issue_body();
        app.on_paste("Draft body\nsecond line");
    }
    app
}

fn submit(app: &mut App, event_tx: Sender<super::super::AppEvent>) {
    use super::super::main_action_utils::*;
    let body = app.editor().text().to_string();
    // A newline makes reqwest reject the Authorization header before sending any request.
    let result = match app.editor_mode() {
        EditorMode::CreateIssue => submit_created_issue(app, "invalid\n", event_tx),
        EditorMode::AddComment => post_issue_comment(app, "invalid\n", body, event_tx),
        EditorMode::EditComment => update_issue_comment(app, "invalid\n", body, event_tx),
        EditorMode::AddPullRequestReviewComment => {
            submit_pull_request_review_comment(app, "invalid\n", body, event_tx)
        }
        EditorMode::EditPullRequestReviewComment => {
            update_pull_request_review_comment(app, "invalid\n", body, event_tx)
        }
        _ => unreachable!(),
    };
    result.expect("submit action");
}

fn receive(event_rx: &Receiver<super::super::AppEvent>) -> super::super::AppEvent {
    event_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("worker result")
}

fn apply(app: &mut App, conn: &rusqlite::Connection, event: super::super::AppEvent) {
    let (tx, rx) = channel();
    tx.send(event).unwrap();
    super::super::main_events::handle_events(app, conn, &rx).expect("apply result");
}

fn failed_submission(mode: EditorMode, conn: &rusqlite::Connection) {
    let mut app = draft(mode);
    let title = app.editor().name().to_string();
    let body = app.editor().text().to_string();
    let (tx, rx) = channel();
    let target = app.pending_review_target().cloned();
    submit(&mut app, tx.clone());
    assert!(app.editor_submission_pending());
    assert_eq!(app.view(), View::CommentEditor);
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.take_action().is_none(),
        "repeat Enter must not submit again"
    );
    app.on_paste("ignored while pending");
    app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.editor().text(), body);
    submit(&mut app, tx);
    apply(&mut app, conn, receive(&rx));
    assert!(
        matches!(
            rx.recv_timeout(Duration::from_secs(10)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ),
        "only one worker should run"
    );
    eprintln!("{mode:?}: view={:?}, status={}", app.view(), app.status());
    assert!(app.status().contains("failed"));
    assert_eq!(
        app.view(),
        View::CommentEditor,
        "failed draft must remain usable"
    );
    assert_eq!(app.editor().name(), title);
    assert_eq!(app.editor().text(), body);
    assert!(!app.editor_submission_pending());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app))
        .unwrap();
    let screen = terminal
        .backend()
        .buffer()
        .content
        .chunks(100)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("Draft body"));
    assert!(screen.contains("second line"));
    eprintln!("{mode:?} after failure at 100x30:\n{screen}");
    assert_eq!(app.pending_review_target(), target.as_ref());
    if mode == EditorMode::AddPullRequestReviewComment {
        app.set_pull_request_files(1, "new-commit".into(), Vec::new());
        assert_eq!(app.pending_review_target(), target.as_ref());
    }
    if mode == EditorMode::EditComment {
        assert_eq!(app.editing_comment_id(), Some(91));
    }
    if mode == EditorMode::EditPullRequestReviewComment {
        assert_eq!(app.editing_pull_request_review_comment_id(), Some(92));
    }

    app.on_paste("\nretry edit");
    let retry_body = format!("{body}\nretry edit");
    assert_eq!(app.editor().text(), retry_body);
    let (tx, rx) = channel();
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    if mode == EditorMode::CreateIssue {
        assert!(app.editor().create_issue_confirm_visible());
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
    super::super::main_actions::handle_actions(
        &mut app,
        &mut crate::clipboard::SystemClipboard::default(),
        conn,
        "invalid\n",
        tx,
    )
    .expect("manually retry through editor input");
    assert!(app.editor_submission_pending());
    apply(&mut app, conn, receive(&rx));
    assert_eq!(app.view(), View::CommentEditor);
    assert_eq!(app.editor().text(), retry_body);
    assert!(!app.editor_submission_pending());
    assert_eq!(app.pending_review_target(), target.as_ref());
}

const MODES: [EditorMode; 5] = [
    EditorMode::CreateIssue,
    EditorMode::AddComment,
    EditorMode::EditComment,
    EditorMode::AddPullRequestReviewComment,
    EditorMode::EditPullRequestReviewComment,
];

fn cache() -> rusqlite::Connection {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).unwrap();
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
    .unwrap();
    for issue in linked_navigation_app(true).issues() {
        crate::store::upsert_issue(&conn, issue).unwrap();
    }
    conn
}

fn completion(app: &mut App, succeeded: bool) -> AppEvent {
    let submission = app.begin_editor_submission().expect("pending submission");
    let repo = RepoIdentity::new("acme", "blippy", None);
    let event = match (app.editor_mode(), succeeded) {
        (EditorMode::CreateIssue, true) => {
            let mut issue = app.issues()[0].clone();
            issue.id = 3;
            issue.number = 9;
            AppEvent::IssueCreated { repo, issue }
        }
        (EditorMode::CreateIssue, false) => AppEvent::IssueCreateFailed {
            repo,
            message: "offline".into(),
        },
        (EditorMode::AddComment, _) => AppEvent::IssueUpdated {
            repo,
            issue_number: 7,
            message: if succeeded {
                "commented"
            } else {
                "comment failed: offline"
            }
            .into(),
        },
        (EditorMode::EditComment, true) => AppEvent::IssueCommentUpdated {
            repo,
            issue_number: 7,
            comment_id: 91,
            body: app.editor().text().into(),
        },
        (EditorMode::EditComment, false) => AppEvent::IssueUpdated {
            repo,
            issue_number: 7,
            message: "comment update failed: offline".into(),
        },
        (EditorMode::AddPullRequestReviewComment, true) => {
            AppEvent::PullRequestReviewCommentCreated { issue_id: 1 }
        }
        (EditorMode::AddPullRequestReviewComment, false) => {
            AppEvent::PullRequestReviewCommentCreateFailed {
                issue_id: 1,
                message: "offline".into(),
            }
        }
        (EditorMode::EditPullRequestReviewComment, true) => {
            AppEvent::PullRequestReviewCommentUpdated {
                issue_id: 1,
                comment_id: 92,
                body: app.editor().text().into(),
            }
        }
        (EditorMode::EditPullRequestReviewComment, false) => {
            AppEvent::PullRequestReviewCommentUpdateFailed {
                issue_id: 1,
                message: "offline".into(),
            }
        }
        _ => unreachable!(),
    };
    AppEvent::EditorSubmissionFinished {
        submission,
        succeeded,
        event: Box::new(event),
    }
}

#[test]
fn successful_submissions_close_their_own_editor() {
    for mode in MODES {
        let conn = cache();
        let mut app = draft(mode);
        let return_view = app.editor_cancel_view();
        let event = completion(&mut app, true);
        apply(&mut app, &conn, event);
        assert!(!app.editor_submission_pending());
        assert_eq!(
            app.view(),
            if mode == EditorMode::CreateIssue {
                View::IssueDetail
            } else {
                return_view
            }
        );
        assert!(app.pending_review_target().is_none());
        assert!(app.editing_comment_id().is_none());
        assert!(app.editing_pull_request_review_comment_id().is_none());
        if mode == EditorMode::CreateIssue {
            assert_eq!(app.current_issue_number(), Some(9));
            assert!(
                crate::store::list_issues(&conn, 1)
                    .unwrap()
                    .iter()
                    .any(|issue| issue.number == 9)
            );
        }
    }
}

#[test]
fn late_submissions_preserve_cancellation_and_newer_editors() {
    for mode in MODES {
        for succeeded in [false, true] {
            for newer_editor in [false, true] {
                let conn = cache();
                let mut app = draft(mode);
                app.editor_mut().show_create_issue_confirm();
                let event = completion(&mut app, succeeded);
                let return_view = app.editor_cancel_view();
                app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                assert_eq!(
                    app.view(),
                    return_view,
                    "Esc cancels even while creating an issue"
                );
                if newer_editor {
                    app.open_comment_edit_editor(View::IssueComments, 94, "New draft");
                }
                let view = app.view();
                app.set_status("Current work");
                apply(&mut app, &conn, event);
                assert_eq!(app.view(), view, "{mode:?}, success={succeeded}");
                assert_eq!(app.current_issue_number(), Some(7));
                assert_eq!(app.status(), "Current work");
                if newer_editor {
                    assert_eq!(app.editor().text(), "New draft");
                    assert_eq!(app.editing_comment_id(), Some(94));
                }
            }
        }
    }
}

#[test]
fn late_submissions_cannot_finish_a_new_pending_request() {
    let conn = cache();
    for mode in MODES {
        for succeeded in [false, true] {
            let mut app = draft(mode);
            let old_event = completion(&mut app, succeeded);
            app.open_issue_comment_editor(View::IssueComments);
            app.on_paste("New submission");
            let new_event = completion(&mut app, false);
            apply(&mut app, &conn, old_event);
            assert!(app.editor_submission_pending());
            assert_eq!(app.view(), View::CommentEditor);
            assert_eq!(app.editor().text(), "New submission");
            apply(&mut app, &conn, new_event);
            assert!(!app.editor_submission_pending());
            assert_eq!(app.view(), View::CommentEditor);
        }
    }
}

#[test]
fn late_submissions_preserve_other_repositories_and_items() {
    for mode in MODES {
        for succeeded in [false, true] {
            for other_repo in [false, true] {
                let conn = cache();
                let mut app = draft(mode);
                let event = completion(&mut app, succeeded);
                if other_repo {
                    app.set_current_repo_with_path("other", "repo", None);
                    // Keep the same issue id to exercise the repository guard independently.
                    app.set_current_issue(1, 7);
                } else {
                    app.set_current_issue(2, 8);
                }
                app.open_comment_edit_editor(View::IssueComments, 94, "Other draft");
                app.set_status("Other work");
                apply(&mut app, &conn, event);
                assert_eq!(app.view(), View::CommentEditor);
                assert_eq!(app.editor().text(), "Other draft");
                assert_eq!(app.editing_comment_id(), Some(94));
                assert_eq!(
                    app.current_issue_number(),
                    Some(if other_repo { 7 } else { 8 })
                );
                assert_eq!(app.status(), "Other work");
                assert!(!app.pull_request_review_comments_syncing());
            }
        }
    }
}

#[test]
fn changed_editor_context_cannot_submit_or_be_closed_by_a_late_result() {
    let conn = cache();
    for other_repo in [false, true] {
        let mut app = draft(EditorMode::AddComment);
        let event = completion(&mut app, true);
        if other_repo {
            app.set_current_repo_with_path("other", "repo", None);
            app.set_current_issue(1, 7);
        } else {
            app.set_current_issue(2, 8);
        }
        apply(&mut app, &conn, event);
        assert_eq!(app.view(), View::CommentEditor);
        let (tx, rx) = channel();
        submit(&mut app, tx);
        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));
        assert_eq!(
            app.status(),
            "Editor target changed; cancel and reopen the editor"
        );
    }
}

#[test]
fn editor_submissions_follow_initial_sync_and_repository_renames() {
    for while_pending in [false, true] {
        for known_id in [false, true] {
            for renamed in [false, true] {
                let conn = cache();
                let mut app = linked_navigation_app(true);
                if known_id {
                    app.set_current_repo_id(1);
                }
                app.open_issue_comment_editor(View::IssueComments);
                app.on_paste("Keep this draft");
                let event = while_pending.then(|| completion(&mut app, false));
                let resolved = crate::store::RepoRow {
                    id: 1,
                    owner: if renamed { "new-owner" } else { "acme" }.into(),
                    name: if renamed { "new-name" } else { "blippy" }.into(),
                    updated_at: None,
                    etag: None,
                };
                assert!(app.resolve_repository_alias("acme", "blippy", &resolved));
                if let Some(event) = event {
                    apply(&mut app, &conn, event);
                    assert!(!app.editor_submission_pending());
                    assert!(app.status().contains("failed"));
                }
                assert_eq!(app.editor().text(), "Keep this draft");
                let event = completion(&mut app, true);
                apply(&mut app, &conn, event);
                assert_eq!(app.view(), View::IssueComments);
                assert_eq!(app.current_issue_number(), Some(7));
                assert_eq!(app.status(), "#7 commented");
            }
        }
    }
}

#[test]
fn editor_submissions_reject_replacement_repository_ids() {
    let conn = cache();
    for while_pending in [false, true] {
        let mut app = linked_navigation_app(true);
        app.set_current_repo_id(1);
        app.open_issue_comment_editor(View::IssueComments);
        app.on_paste("Original repository draft");
        let event = while_pending.then(|| completion(&mut app, true));
        app.set_current_repo_id(2);
        app.set_status("Replacement repository");
        if let Some(event) = event {
            apply(&mut app, &conn, event);
            assert_eq!(app.view(), View::CommentEditor);
            assert_eq!(app.status(), "Replacement repository");
        }
        assert!(app.begin_editor_submission().is_none());
        assert_eq!(app.editor().text(), "Original repository draft");
    }
}

#[test]
fn failed_add_comment_keeps_draft() {
    failed_submission(
        EditorMode::AddComment,
        &rusqlite::Connection::open_in_memory().unwrap(),
    );
}

#[test]
fn failed_edit_comment_keeps_draft() {
    failed_submission(
        EditorMode::EditComment,
        &rusqlite::Connection::open_in_memory().unwrap(),
    );
}

#[test]
fn failed_add_review_keeps_draft() {
    failed_submission(
        EditorMode::AddPullRequestReviewComment,
        &rusqlite::Connection::open_in_memory().unwrap(),
    );
}

#[test]
fn failed_edit_review_keeps_draft() {
    failed_submission(
        EditorMode::EditPullRequestReviewComment,
        &rusqlite::Connection::open_in_memory().unwrap(),
    );
}

#[test]
fn failed_create_issue_keeps_draft() {
    const ISOLATED: &str = "BLIPPY_TEST_DRAFT_DATA";
    if std::env::var_os(ISOLATED).is_some() {
        let conn = crate::store::open_db().unwrap();
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
        .unwrap();
        failed_submission(EditorMode::CreateIssue, &conn);
        return;
    }
    let dir = std::env::temp_dir().join(format!("blippy-draft-test-{}", std::process::id()));
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::editor_submission::failed_create_issue_keeps_draft",
            "--nocapture",
        ])
        .env(ISOLATED, "1")
        .env("XDG_DATA_HOME", &dir)
        .env("LOCALAPPDATA", &dir)
        .output()
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
