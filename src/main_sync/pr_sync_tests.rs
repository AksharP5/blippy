use super::*;
use crate::app::PullRequestReviewFocus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

const ORIGINAL: &str = "@@ -0,0 +1,2 @@\n+unsafe_call();\n+return true;";
const INSERTED: &str = "@@ -0,0 +1,3 @@\n+let safe = true;\n+unsafe_call();\n+return true;";

fn commits(head: &str, base: &str) -> crate::github::ApiPullRequestSummary {
    serde_json::from_value(serde_json::json!({
        "head": {"sha": head}, "base": {"sha": base}
    }))
    .expect("diff commits")
}

fn files(head: &str, base: &str, patch: &str) -> AppEvent {
    AppEvent::PullRequestFilesUpdated {
        issue_id: 901,
        commits: commits(head, base),
        files: vec![PullRequestFile {
            filename: "src/main.rs".into(),
            status: "added".into(),
            additions: 3,
            deletions: 0,
            patch: Some(patch.into()),
        }],
        pull_request_id: Some("PR_7".into()),
        viewed_files: HashSet::new(),
        view_state_error: None,
    }
}

fn comments(head: &str, base: &str, line: i64) -> AppEvent {
    let comment = serde_json::from_value(serde_json::json!({
        "id": 10,
        "path": "src/main.rs",
        "line": line,
        "side": "RIGHT",
        "body": "Unsafe call needs checking",
        "user": {"login": "alice"},
    }))
    .expect("API comment");
    AppEvent::PullRequestReviewCommentsUpdated {
        issue_id: 901,
        commits: commits(head, base),
        comments: map_review_comments(vec![comment]),
    }
}

fn failure() -> AppEvent {
    AppEvent::PullRequestReviewCommentsFailed {
        issue_id: 901,
        message: "GraphQL temporarily unavailable".into(),
    }
}

fn apply(app: &mut App, events: impl IntoIterator<Item = AppEvent>) {
    let conn = rusqlite::Connection::open_in_memory().expect("conn");
    let (tx, rx) = std::sync::mpsc::channel();
    for event in events {
        tx.send(event).expect("completion");
    }
    crate::main_events::handle_events(app, &conn, &rx).expect("apply completions");
}

fn key(app: &mut App, ch: char) {
    app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
}

fn review() -> App {
    let mut app = App::new(crate::config::Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_issues(vec![crate::store::IssueRow {
        id: 901,
        repo_id: 900,
        number: 7,
        state: "open".into(),
        title: "Review".into(),
        body: String::new(),
        labels: Vec::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_current_issue(901, 7);
    app.set_view(View::PullRequestFiles);
    apply(
        &mut app,
        [
            files("head-a", "base-a", ORIGINAL),
            comments("head-a", "base-a", 1),
        ],
    );
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
    key(&mut app, 'j');
    assert_eq!(app.selected_pull_request_review_comment().unwrap().id, 10);
    app
}

fn render(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).expect("terminal");
    terminal
        .draw(|frame| crate::ui::draw(frame, app))
        .expect("render review");
    terminal
        .backend()
        .buffer()
        .content
        .chunks(120)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn changed_diff_hides_old_review_anchors_when_comments_fail() {
    for (head, base, patch) in [
        ("head-b", "base-a", INSERTED),
        ("head-a", "base-b", INSERTED),
        ("head-a", "base-a", INSERTED),
        ("head-b", "base-a", ORIGINAL),
    ] {
        for files_first in [true, false] {
            let mut app = review();
            key(&mut app, 'r');
            assert!(app.take_pull_request_files_sync_request());
            assert!(app.take_pull_request_review_comments_sync_request());
            let events = if files_first {
                [files(head, base, patch), failure()]
            } else {
                [failure(), files(head, base, patch)]
            };
            apply(&mut app, events);
            app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
            key(&mut app, 'j');
            assert!(app.selected_pull_request_review_comment().is_none());
            key(&mut app, 'n');
            assert_eq!(app.selected_pull_request_review_comment_id(), None);
            assert_eq!(app.pull_request_comments_count_for_path("src/main.rs"), 0);
            let text = render(&mut app);
            assert!(!text.contains("Unsafe call needs"), "{head}/{base}\n{text}");
            if patch == INSERTED {
                assert!(text.contains("let safe = true;"));
            }
            println!("{head}/{base}, files_first={files_first}\n{text}");
        }
    }
}

#[test]
fn matching_review_comments_survive_either_completion_order() {
    for (head, base) in [("head-b", "base-a"), ("head-a", "base-b")] {
        for files_first in [true, false] {
            let mut app = review();
            if files_first {
                apply(&mut app, [files(head, base, INSERTED)]);
            } else {
                apply(&mut app, [comments(head, base, 2)]);
            }
            assert_eq!(app.pull_request_comments_count_for_path("src/main.rs"), 0);
            if files_first {
                apply(&mut app, [comments(head, base, 2)]);
            } else {
                apply(&mut app, [files(head, base, INSERTED)]);
            }
            app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
            key(&mut app, 'j');
            assert!(app.selected_pull_request_review_comment().is_none());
            key(&mut app, 'j');
            assert_eq!(app.selected_pull_request_review_comment().unwrap().id, 10);
            let text = render(&mut app);
            assert!(text.contains("Unsafe call needs"), "{head}/{base}\n{text}");
            let rows = text.lines().collect::<Vec<_>>();
            let comment_row = rows
                .iter()
                .position(|row| row.contains("Unsafe call needs"))
                .unwrap();
            assert!(rows[comment_row - 1].contains("unsafe_call();"));
            apply(&mut app, [failure()]);
            assert_eq!(app.selected_pull_request_review_comment().unwrap().id, 10);
            println!("{head}/{base}, files_first={files_first}\n{text}");
        }
    }
}

#[test]
fn late_comments_for_an_old_diff_stay_unavailable() {
    let mut app = review();
    apply(&mut app, [files("head-b", "base-a", INSERTED)]);
    apply(&mut app, [comments("head-a", "base-a", 1)]);
    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
    key(&mut app, 'j');
    assert!(app.selected_pull_request_review_comment().is_none());
    assert!(!render(&mut app).contains("Unsafe call needs"));
    apply(&mut app, [comments("head-b", "base-a", 2)]);
    key(&mut app, 'j');
    assert_eq!(app.selected_pull_request_review_comment().unwrap().id, 10);
}

#[test]
fn unchanged_refresh_preserves_review_reading_state_and_comments() {
    let mut app = review();
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.set_pull_request_diff_horizontal_max(20);
    key(&mut app, ']');
    let reading_position = (
        app.pull_request_review_focus(),
        app.selected_pull_request_diff_line(),
        app.pull_request_diff_expanded(),
        app.pull_request_diff_horizontal_scroll(),
        app.selected_pull_request_review_comment_id(),
    );
    apply(&mut app, [failure(), files("head-a", "base-a", ORIGINAL)]);
    assert_eq!(
        (
            app.pull_request_review_focus(),
            app.selected_pull_request_diff_line(),
            app.pull_request_diff_expanded(),
            app.pull_request_diff_horizontal_scroll(),
            app.selected_pull_request_review_comment_id(),
        ),
        reading_position
    );
    assert_eq!(app.selected_pull_request_review_comment().unwrap().id, 10);
}
