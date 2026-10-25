use crate::main_events::handle_events;
use crate::{App, AppEvent, RepoIdentity, SyncStats, View, WorkItemMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn manual_refresh_rechecks_cached_permissions() {
    let conn = crate::store::open_db_at(std::path::Path::new(":memory:")).expect("proof database");
    let repo_row = crate::store::RepoRow {
        id: 701,
        owner: "acme".into(),
        name: "blippy".into(),
        updated_at: None,
        etag: None,
    };
    crate::store::upsert_repo(&conn, &repo_row).expect("cached repo");
    crate::store::upsert_issue(
        &conn,
        &crate::store::IssueRow {
            id: 702,
            repo_id: repo_row.id,
            number: 7,
            state: "open".into(),
            title: "Issue".into(),
            body: String::new(),
            labels: Vec::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        },
    )
    .expect("cached issue");
    let mut failures = Vec::new();
    for view in [
        View::Issues,
        View::IssueDetail,
        View::IssueComments,
        View::PullRequestFiles,
    ] {
        for not_modified in [false, true] {
            let mut app = App::new(crate::config::Config::default());
            app.set_current_repo_with_path("acme", "blippy", None);
            app.set_current_repo_id(repo_row.id);
            app.set_work_item_mode(WorkItemMode::PullRequests);
            app.set_issues(crate::store::list_issues(&conn, repo_row.id).expect("issues"));
            app.set_current_issue(702, 7);
            app.set_view(view);
            assert!(app.take_repo_permissions_sync_request());
            app.take_repo_labels_sync_request();
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(AppEvent::RepoPermissionsResolved {
                repo: RepoIdentity::new("acme", "blippy", Some(repo_row.id)),
                can_edit_issue_metadata: false,
                can_merge_pull_request: false,
            })
            .expect("initial read-only permissions");
            handle_events(&mut app, &conn, &rx).expect("initial permissions");

            // The user has since received write access on GitHub and presses refresh.
            app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
            assert!(app.take_sync_request(), "refresh issues for {view:?}");
            tx.send(AppEvent::SyncFinished {
                owner: "acme".into(),
                repo: "blippy".into(),
                resolved_repo: repo_row.clone(),
                stats: SyncStats {
                    not_modified,
                    ..SyncStats::default()
                },
            })
            .expect("successful refresh");
            handle_events(&mut app, &conn, &rx).expect("refresh completion");
            let requested = app.take_repo_permissions_sync_request();
            let edit_allowed = crate::main_action_utils::ensure_can_edit_issue_metadata(&mut app);
            let merge_allowed = crate::main_action_utils::ensure_can_merge_pull_request(&mut app);
            let action_requested = app.take_repo_permissions_sync_request();
            println!(
                "view={view:?}, not_modified={not_modified}, refresh_permission_request={requested}, edit={edit_allowed}, merge={merge_allowed}, action_permission_request={action_requested}"
            );
            assert!(!edit_allowed);
            assert!(!merge_allowed);
            if !requested {
                failures.push((view, not_modified));
            }

            // A fresh permission response is sufficient to restore the same actions.
            tx.send(AppEvent::RepoPermissionsResolved {
                repo: RepoIdentity::new("acme", "blippy", Some(repo_row.id)),
                can_edit_issue_metadata: true,
                can_merge_pull_request: true,
            })
            .expect("current permissions control");
            handle_events(&mut app, &conn, &rx).expect("current permissions");
            assert!(crate::main_action_utils::ensure_can_edit_issue_metadata(
                &mut app
            ));
            assert!(crate::main_action_utils::ensure_can_merge_pull_request(
                &mut app
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "refresh never rechecked denied permissions: {failures:?}"
    );
}
