mod app;
mod auth;
mod cli;
mod config;
mod discovery;
mod git;
mod github;
mod keybinds;
mod markdown;
mod pr_diff;
mod repo_index;
mod store;
mod sync;
mod theme;
mod ui;

use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{self, Stdout, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{
    App, AppAction, IssueFilter, PendingIssueAction, PresetSelection, PullRequestFile,
    PullRequestReviewComment, ReviewSide, View, WorkItemMode,
};
use crate::auth::{SystemAuth, clear_auth_token, resolve_auth_token};
use crate::cli::{CliCommand, parse_args};
use crate::config::Config;
use crate::discovery::{home_dir, quick_scan};
use crate::git::list_github_remotes_at;
use crate::github::GitHubClient;
use crate::repo_index::index_repo_path;
use crate::store::delete_db;
use crate::store::{
    comment_now_epoch, comments_for_issue, get_repo_by_slug, list_issues, list_local_repos,
    prune_comments, touch_comments_for_issue, update_issue_comments_count,
};
use crate::sync::{SyncStats, sync_repo_with_progress};

type TuiBackend = CrosstermBackend<Stdout>;
type Tui = Terminal<TuiBackend>;

const AUTH_DEBUG_ENV: &str = "GLYPH_AUTH_DEBUG";
const ISSUE_POLL_INTERVAL: Duration = Duration::from_secs(15);
const COMMENT_POLL_INTERVAL: Duration = Duration::from_secs(30);
const COMMENT_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const COMMENT_CAP: i64 = 7_500;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(command) = parse_args(&args)? {
        return handle_command(command);
    }

    let auth = SystemAuth::new();
    let auth_token = resolve_auth_token(&auth)?;
    if env::var(AUTH_DEBUG_ENV).is_ok() {
        eprintln!("Auth source: {}", auth_token.method.label());
    }
    let token = auth_token.value;

    let mut terminal_guard = TerminalGuard::init()?;
    let config = Config::load()?;
    let conn = crate::store::open_db()?;
    let mut app = App::new(config);
    initialize_app(&mut app, &conn)?;

    let (event_tx, event_rx) = mpsc::channel();
    if app.view() == View::RepoPicker {
        app.set_scanning(true);
        app.set_status("Scanning...");
    }
    maybe_start_scan(&app, event_tx.clone())?;

    run_app(
        terminal_guard.terminal_mut(),
        &mut app,
        &conn,
        &token,
        event_rx,
        event_tx,
    )?;
    Ok(())
}

fn handle_command(command: CliCommand) -> Result<()> {
    match command {
        CliCommand::AuthReset => handle_auth_reset(),
        CliCommand::CacheReset => handle_cache_reset(),
        CliCommand::Sync => handle_sync(),
    }
}

fn handle_auth_reset() -> Result<()> {
    let auth = SystemAuth::new();
    let cleared = clear_auth_token(&auth)?;
    if cleared {
        println!("Auth token removed from keychain.");
        return Ok(());
    }

    println!("No stored auth token found.");
    Ok(())
}

fn handle_cache_reset() -> Result<()> {
    let deleted = delete_db()?;
    if deleted {
        println!("Cache removed.");
        return Ok(());
    }

    println!("No cache found.");
    Ok(())
}

fn handle_sync() -> Result<()> {
    let home = home_dir().unwrap_or(env::current_dir()?);
    let repos = crate::discovery::full_scan(&home)?;
    let conn = crate::store::open_db()?;

    let start = Instant::now();
    let mut indexed = 0usize;
    for repo in &repos {
        indexed += index_repo_path(&conn, &repo.path)?;
    }

    let duration = start.elapsed();
    println!(
        "Discovered {} repos ({} remotes) in {:.2?}",
        repos.len(),
        indexed,
        duration
    );
    Ok(())
}

fn run_app(
    terminal: &mut Tui,
    app: &mut App,
    conn: &rusqlite::Connection,
    token: &str,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut last_issue_poll = Instant::now();
    let mut last_comment_poll = Instant::now();
    let mut last_view = app.view();

    loop {
        if app.view() != last_view {
            if matches!(
                last_view,
                View::IssueDetail | View::IssueComments | View::PullRequestFiles
            ) {
                app.set_comment_syncing(false);
                app.set_pull_request_files_syncing(false);
                app.set_pull_request_review_comments_syncing(false);
            }
            last_view = app.view();
            last_issue_poll = Instant::now();
            last_comment_poll = Instant::now();
        }

        handle_events(app, conn, &event_rx)?;
        drive_background_tasks(
            app,
            conn,
            token,
            event_tx.clone(),
            &mut last_issue_poll,
            &mut last_comment_poll,
        )?;
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit() {
            return Ok(());
        }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if !event::poll(timeout)? {
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
            continue;
        }

        match event::read()? {
            Event::Key(key) => app.on_key(key),
            Event::Mouse(mouse) => app.on_mouse(mouse),
            _ => {}
        }

        handle_actions(app, conn, token, event_tx.clone())?;
        drive_background_tasks(
            app,
            conn,
            token,
            event_tx.clone(),
            &mut last_issue_poll,
            &mut last_comment_poll,
        )?;

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn drive_background_tasks(
    app: &mut App,
    conn: &rusqlite::Connection,
    token: &str,
    event_tx: Sender<AppEvent>,
    last_issue_poll: &mut Instant,
    last_comment_poll: &mut Instant,
) -> Result<()> {
    maybe_start_issue_poll(app, last_issue_poll);
    maybe_start_repo_sync(app, token, event_tx.clone())?;
    maybe_start_comment_poll(app, token, event_tx.clone(), last_comment_poll)?;
    maybe_start_pull_request_files_sync(app, token, event_tx.clone())?;
    maybe_start_pull_request_review_comments_sync(app, token, event_tx.clone())?;
    maybe_probe_visible_linked_items(app, token, event_tx.clone());
    if app.view() == View::RepoPicker && app.repos().is_empty() {
        app.set_repos(load_repos(conn)?);
    }
    maybe_start_rescan(app, event_tx)?;
    Ok(())
}

fn maybe_probe_visible_linked_items(app: &mut App, token: &str, event_tx: Sender<AppEvent>) {
    if app.view() != View::Issues {
        return;
    }
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => return,
    };

    let visible = app
        .issues_for_view()
        .iter()
        .take(20)
        .map(|issue| (issue.number, issue.is_pr))
        .collect::<Vec<(i64, bool)>>();

    for (number, is_pr) in visible {
        if is_pr {
            if !app.begin_linked_issue_lookup(number) {
                continue;
            }
            start_linked_issue_lookup(
                owner.clone(),
                repo.clone(),
                number,
                token.to_string(),
                event_tx.clone(),
                LinkedIssueTarget::Probe,
            );
            continue;
        }

        if !app.begin_linked_pull_request_lookup(number) {
            continue;
        }
        start_linked_pull_request_lookup(
            owner.clone(),
            repo.clone(),
            number,
            token.to_string(),
            event_tx.clone(),
            LinkedPullRequestTarget::Probe,
        );
    }
}

fn initialize_app(app: &mut App, conn: &rusqlite::Connection) -> Result<()> {
    let repo_root = crate::git::repo_root()?;
    if let Some(root) = repo_root {
        let remotes = list_github_remotes_at(&root)?;
        if remotes.is_empty() {
            app.set_status("No GitHub remotes found.");
            app.set_repos(load_repos(conn)?);
            app.set_view(View::RepoPicker);
            return Ok(());
        }

        if remotes.len() == 1 {
            let remote = &remotes[0];
            let root_path = root.to_string_lossy().to_string();
            load_issues_for_slug(
                app,
                conn,
                &remote.slug.owner,
                &remote.slug.repo,
                Some(root_path.as_str()),
            )?;
            app.set_view(View::Issues);
            app.request_sync();
            return Ok(());
        }

        app.set_remotes(remotes);
        app.set_view(View::RemoteChooser);
        return Ok(());
    }

    app.set_repos(load_repos(conn)?);
    app.set_view(View::RepoPicker);
    Ok(())
}

fn handle_actions(
    app: &mut App,
    conn: &rusqlite::Connection,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let action = match app.take_action() {
        Some(action) => action,
        None => return Ok(()),
    };

    match action {
        AppAction::PickRepo => {
            let (owner, repo, path) = match app.selected_repo_target() {
                Some(target) => target,
                None => return Ok(()),
            };
            load_issues_for_slug(app, conn, &owner, &repo, Some(path.as_str()))?;
            app.set_view(View::Issues);
            app.request_sync();
        }
        AppAction::PickRemote => {
            let (owner, repo) = match app.remotes().get(app.selected_remote()) {
                Some(remote) => (remote.slug.owner.clone(), remote.slug.repo.clone()),
                None => return Ok(()),
            };
            let repo_path = crate::git::repo_root()?.map(|path| path.to_string_lossy().to_string());
            load_issues_for_slug(app, conn, &owner, &repo, repo_path.as_deref())?;
            app.set_view(View::Issues);
            app.request_sync();
        }
        AppAction::PickIssue => {
            let (issue_id, issue_number, is_pr) = match app.selected_issue_row() {
                Some(issue) => (issue.id, issue.number, issue.is_pr),
                None => return Ok(()),
            };
            app.set_current_issue(issue_id, issue_number);
            app.reset_issue_detail_scroll();
            load_comments_for_issue(app, conn, issue_id)?;
            app.set_view(View::IssueDetail);
            app.set_comment_syncing(false);
            app.request_comment_sync();
            if is_pr {
                app.request_pull_request_files_sync();
                app.request_pull_request_review_comments_sync();
                if app.begin_linked_issue_lookup(issue_number) {
                    if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo()) {
                        start_linked_issue_lookup(
                            owner.to_string(),
                            repo.to_string(),
                            issue_number,
                            token.to_string(),
                            event_tx.clone(),
                            LinkedIssueTarget::Probe,
                        );
                    } else {
                        app.end_linked_issue_lookup(issue_number);
                    }
                }
            } else if app.begin_linked_pull_request_lookup(issue_number) {
                if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo()) {
                    start_linked_pull_request_lookup(
                        owner.to_string(),
                        repo.to_string(),
                        issue_number,
                        token.to_string(),
                        event_tx.clone(),
                        LinkedPullRequestTarget::Probe,
                    );
                } else {
                    app.end_linked_pull_request_lookup(issue_number);
                }
            }
        }
        AppAction::OpenInBrowser => {
            if let Some(url) = issue_url(app) {
                if let Err(error) = open_url(&url) {
                    app.set_status(format!("Open failed: {}", error));
                } else {
                    app.set_status("Opened in browser".to_string());
                }
            } else {
                app.set_status("No issue selected".to_string());
            }
        }
        AppAction::CheckoutPullRequest => {
            checkout_pull_request(app)?;
        }
        AppAction::OpenLinkedPullRequestInBrowser => {
            open_linked_pull_request(
                app,
                token,
                event_tx.clone(),
                LinkedPullRequestTarget::Browser,
            )?;
        }
        AppAction::OpenLinkedPullRequestInTui => {
            open_linked_pull_request(app, token, event_tx.clone(), LinkedPullRequestTarget::Tui)?;
        }
        AppAction::OpenLinkedIssueInBrowser => {
            open_linked_issue(app, token, event_tx.clone(), LinkedIssueTarget::Browser)?;
        }
        AppAction::OpenLinkedIssueInTui => {
            open_linked_issue(app, token, event_tx.clone(), LinkedIssueTarget::Tui)?;
        }
        AppAction::CopyStatus => {
            copy_status_to_clipboard(app)?;
        }
        AppAction::AddIssueComment => {
            let (issue_id, issue_number, _) = match selected_issue_for_action(app) {
                Some(issue) => issue,
                None => {
                    app.set_status("No issue selected".to_string());
                    return Ok(());
                }
            };
            app.set_current_issue(issue_id, issue_number);
            app.open_issue_comment_editor(app.view());
        }
        AppAction::EditIssueComment => {
            let return_view = app.view();
            let comment = match app.selected_comment_row() {
                Some(comment) => comment.clone(),
                None => {
                    app.set_status("No comment selected".to_string());
                    return Ok(());
                }
            };
            app.open_comment_edit_editor(return_view, comment.id, comment.body.as_str());
        }
        AppAction::DeleteIssueComment => {
            delete_issue_comment(app, token, event_tx.clone())?;
        }
        AppAction::AddPullRequestReviewComment => {
            let target = match app.selected_pull_request_review_target() {
                Some(target) => target,
                None => {
                    app.set_status("Select a diff line to comment on".to_string());
                    return Ok(());
                }
            };
            app.open_pull_request_review_comment_editor(app.view(), target);
        }
        AppAction::SubmitPullRequestReviewComment => {
            let comment = app.editor().text().to_string();
            submit_pull_request_review_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::EditPullRequestReviewComment => {
            let return_view = app.view();
            let comment = match app.selected_pull_request_review_comment() {
                Some(comment) => comment.clone(),
                None => {
                    app.set_status("No review comment selected".to_string());
                    return Ok(());
                }
            };
            app.open_pull_request_review_comment_edit_editor(
                return_view,
                comment.id,
                comment.body.as_str(),
            );
        }
        AppAction::DeletePullRequestReviewComment => {
            delete_pull_request_review_comment(app, token, event_tx.clone())?;
        }
        AppAction::ResolvePullRequestReviewComment => {
            resolve_pull_request_review_comment(app, token, event_tx.clone())?;
        }
        AppAction::TogglePullRequestFileViewed => {
            toggle_pull_request_file_viewed(app, token, event_tx.clone())?;
        }
        AppAction::SubmitEditedPullRequestReviewComment => {
            let comment = app.editor().text().to_string();
            update_pull_request_review_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::EditLabels => {
            let return_view = app.view();
            let (issue_id, issue_number, _) = match selected_issue_for_action(app) {
                Some(issue) => issue,
                None => {
                    app.set_status("No issue selected".to_string());
                    return Ok(());
                }
            };
            app.set_current_issue(issue_id, issue_number);
            let labels = selected_issue_labels(app).unwrap_or_default();
            let options = label_options_for_repo(app);
            app.open_label_picker(return_view, options, labels.as_str());
            if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo()) {
                start_fetch_labels(
                    owner.to_string(),
                    repo.to_string(),
                    token.to_string(),
                    event_tx.clone(),
                );
            }
        }
        AppAction::EditAssignees => {
            let return_view = app.view();
            let (issue_id, issue_number, _) = match selected_issue_for_action(app) {
                Some(issue) => issue,
                None => {
                    app.set_status("No issue selected".to_string());
                    return Ok(());
                }
            };
            app.set_current_issue(issue_id, issue_number);
            let assignees = selected_issue_assignees(app).unwrap_or_default();
            let options = assignee_options_for_repo(app);
            app.open_assignee_picker(return_view, options, assignees.as_str());
        }
        AppAction::SubmitIssueComment => {
            let comment = app.editor().text().to_string();
            post_issue_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::SubmitEditedComment => {
            let comment = app.editor().text().to_string();
            update_issue_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::SubmitLabels => {
            let labels = app.selected_labels();
            update_issue_labels(app, token, labels, event_tx.clone())?;
        }
        AppAction::SubmitAssignees => {
            let assignees = app.selected_assignees();
            update_issue_assignees(app, token, assignees, event_tx.clone())?;
        }
        AppAction::CloseIssue => {
            if let Some((issue_id, issue_number, _)) = selected_issue_for_action(app) {
                app.set_current_issue(issue_id, issue_number);
            }
            app.set_selected_preset(0);
            app.set_view(View::CommentPresetPicker);
        }
        AppAction::ReopenIssue => {
            reopen_issue(app, token, event_tx.clone())?;
        }
        AppAction::PickPreset => handle_preset_selection(app, conn, token, event_tx)?,
        AppAction::SubmitComment => {
            let comment = app.editor().text().to_string();
            close_issue_with_comment(app, token, Some(comment), event_tx.clone())?;
        }
        AppAction::SavePreset => {
            save_preset_from_editor(app)?;
            app.set_view(View::CommentPresetPicker);
        }
    }
    Ok(())
}

fn handle_preset_selection(
    app: &mut App,
    _conn: &rusqlite::Connection,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    match app.preset_selection() {
        PresetSelection::CloseWithoutComment => {
            close_issue_with_comment(app, token, None, event_tx)?;
        }
        PresetSelection::CustomMessage => {
            app.open_close_comment_editor();
        }
        PresetSelection::Preset(index) => {
            let body = app
                .comment_defaults()
                .get(index)
                .map(|preset| preset.body.clone());
            if body.is_none() {
                app.set_status("Preset not found".to_string());
                return Ok(());
            }
            close_issue_with_comment(app, token, body, event_tx)?;
        }
        PresetSelection::AddPreset => {
            app.editor_mut().reset_for_preset_name();
            app.set_view(View::CommentPresetName);
        }
    }
    Ok(())
}

fn save_preset_from_editor(app: &mut App) -> Result<()> {
    let name = app.editor().name().trim().to_string();
    if name.is_empty() {
        app.set_status("Preset name required".to_string());
        return Ok(());
    }
    let body = app.editor().text().to_string();
    if body.trim().is_empty() {
        app.set_status("Preset body required".to_string());
        return Ok(());
    }

    app.add_comment_default(crate::config::CommentDefault { name, body });
    app.save_config()?;
    app.set_status("Preset saved".to_string());
    Ok(())
}

fn close_issue_with_comment(
    app: &mut App,
    token: &str,
    body: Option<String>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let (owner, repo, issue_number) =
        match (app.current_owner(), app.current_repo(), issue_number(app)) {
            (Some(owner), Some(repo), Some(issue_number)) => {
                (owner.to_string(), repo.to_string(), issue_number)
            }
            _ => {
                app.set_status("No issue selected".to_string());
                return Ok(());
            }
        };

    start_close_issue(owner, repo, issue_number, token.to_string(), body, event_tx);
    app.set_pending_issue_action(issue_number, PendingIssueAction::Closing);
    app.set_view(View::Issues);
    app.set_status("Closing issue...".to_string());
    Ok(())
}

fn post_issue_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Comment cannot be empty".to_string());
        return Ok(());
    }

    let (owner, repo, issue_number) =
        match (app.current_owner(), app.current_repo(), issue_number(app)) {
            (Some(owner), Some(repo), Some(issue_number)) => {
                (owner.to_string(), repo.to_string(), issue_number)
            }
            _ => {
                app.set_status("No issue selected".to_string());
                return Ok(());
            }
        };

    start_add_comment(owner, repo, issue_number, token.to_string(), body, event_tx);
    app.set_view(app.editor_cancel_view());
    app.set_status("Posting comment...".to_string());
    Ok(())
}

fn update_issue_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Comment cannot be empty".to_string());
        return Ok(());
    }

    let comment_id = match app.take_editing_comment_id() {
        Some(comment_id) => comment_id,
        None => {
            app.set_status("No comment selected".to_string());
            return Ok(());
        }
    };

    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_update_comment(
        owner,
        repo,
        issue_number,
        comment_id,
        token.to_string(),
        body,
        event_tx,
    );
    app.set_view(app.editor_cancel_view());
    app.set_status("Updating comment...".to_string());
    Ok(())
}

fn submit_pull_request_review_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Review comment cannot be empty".to_string());
        return Ok(());
    }

    let target = match app.take_pending_review_target() {
        Some(target) => target,
        None => {
            app.set_status("No review target selected".to_string());
            return Ok(());
        }
    };

    let pull_number = match issue_number(app) {
        Some(pull_number) => pull_number,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_create_pull_request_review_comment(
        owner,
        repo,
        issue_id,
        pull_number,
        target.path,
        target.line,
        target.side,
        target.start_line,
        target.start_side,
        token.to_string(),
        body,
        event_tx,
    );
    app.set_view(app.editor_cancel_view());
    app.set_status("Submitting review comment...".to_string());
    Ok(())
}

fn update_pull_request_review_comment(
    app: &mut App,
    token: &str,
    body: String,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if body.trim().is_empty() {
        app.set_status("Review comment cannot be empty".to_string());
        return Ok(());
    }

    let comment_id = match app.take_editing_pull_request_review_comment_id() {
        Some(comment_id) => comment_id,
        None => {
            app.set_status("No review comment selected".to_string());
            return Ok(());
        }
    };

    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_update_pull_request_review_comment(
        owner,
        repo,
        issue_id,
        comment_id,
        token.to_string(),
        body,
        event_tx,
    );
    app.set_view(app.editor_cancel_view());
    app.set_status("Updating review comment...".to_string());
    Ok(())
}

fn delete_pull_request_review_comment(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let comment = match app.selected_pull_request_review_comment() {
        Some(comment) => comment,
        None => {
            app.set_status("No review comment selected".to_string());
            return Ok(());
        }
    };

    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_delete_pull_request_review_comment(
        owner,
        repo,
        issue_id,
        comment.id,
        token.to_string(),
        event_tx,
    );
    app.set_status("Deleting review comment...".to_string());
    Ok(())
}

fn resolve_pull_request_review_comment(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let comment = match app.selected_pull_request_review_comment() {
        Some(comment) => comment,
        None => {
            app.set_status("No review comment selected".to_string());
            return Ok(());
        }
    };
    let thread_id = match comment.thread_id.clone() {
        Some(thread_id) => thread_id,
        None => {
            app.set_status("Selected comment has no resolvable thread".to_string());
            return Ok(());
        }
    };
    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    let resolve = !comment.resolved;
    start_toggle_pull_request_review_thread_resolution(
        owner,
        repo,
        issue_id,
        thread_id,
        resolve,
        token.to_string(),
        event_tx,
    );
    if resolve {
        app.set_status("Resolving review thread...".to_string());
        return Ok(());
    }
    app.set_status("Reopening review thread...".to_string());
    Ok(())
}

fn toggle_pull_request_file_viewed(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let (path, viewed) = match app.selected_pull_request_file_view_toggle() {
        Some(toggle) => toggle,
        None => {
            app.set_status("No changed file selected".to_string());
            return Ok(());
        }
    };
    let issue_id = match app.current_issue_id() {
        Some(issue_id) => issue_id,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    let pull_request_id = match app.pull_request_id() {
        Some(pull_request_id) => pull_request_id.to_string(),
        None => {
            app.request_pull_request_files_sync();
            app.set_status("Loading pull request metadata...".to_string());
            return Ok(());
        }
    };
    if !matches!(
        (app.current_owner(), app.current_repo()),
        (Some(_), Some(_))
    ) {
        app.set_status("No repo selected".to_string());
        return Ok(());
    }

    app.set_pull_request_file_viewed(path.as_str(), viewed);
    start_set_pull_request_file_viewed(
        issue_id,
        pull_request_id,
        path.clone(),
        viewed,
        token.to_string(),
        event_tx,
    );
    if viewed {
        app.set_status(format!("Marking {} viewed on GitHub...", path));
        return Ok(());
    }
    app.set_status(format!("Marking {} unviewed on GitHub...", path));
    Ok(())
}

fn delete_issue_comment(app: &mut App, token: &str, event_tx: Sender<AppEvent>) -> Result<()> {
    let comment = match app.selected_comment_row() {
        Some(comment) => comment,
        None => {
            app.set_status("No comment selected".to_string());
            return Ok(());
        }
    };
    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_delete_comment(
        owner,
        repo,
        issue_number,
        comment.id,
        comment.issue_id,
        token.to_string(),
        event_tx,
    );
    app.set_status("Deleting comment...".to_string());
    Ok(())
}

fn update_issue_labels(
    app: &mut App,
    token: &str,
    labels: Vec<String>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    let labels_display = labels.join(",");
    start_update_labels(
        owner,
        repo,
        issue_number,
        token.to_string(),
        labels,
        event_tx,
        labels_display,
    );
    app.set_pending_issue_action(issue_number, PendingIssueAction::UpdatingLabels);
    app.set_view(app.editor_cancel_view());
    app.set_status(format!("Updating labels for #{}...", issue_number));
    Ok(())
}

fn update_issue_assignees(
    app: &mut App,
    token: &str,
    assignees: Vec<String>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let issue_number = match issue_number(app) {
        Some(issue_number) => issue_number,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    let assignees_display = assignees.join(",");
    start_update_assignees(
        owner,
        repo,
        issue_number,
        token.to_string(),
        assignees,
        event_tx,
        assignees_display,
    );
    app.set_pending_issue_action(issue_number, PendingIssueAction::UpdatingAssignees);
    app.set_view(app.editor_cancel_view());
    app.set_status(format!("Updating assignees for #{}...", issue_number));
    Ok(())
}

fn reopen_issue(app: &mut App, token: &str, event_tx: Sender<AppEvent>) -> Result<()> {
    let (issue_id, issue_number, issue_state) = match selected_issue_for_action(app) {
        Some(issue) => issue,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };

    if issue_state
        .as_deref()
        .is_some_and(|state| state.eq_ignore_ascii_case("open"))
    {
        app.set_status("Issue is already open".to_string());
        return Ok(());
    }

    app.set_current_issue(issue_id, issue_number);
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_reopen_issue(owner, repo, issue_number, token.to_string(), event_tx);
    app.set_pending_issue_action(issue_number, PendingIssueAction::Reopening);
    app.set_status("Reopening issue...".to_string());
    Ok(())
}

fn selected_issue_for_action(app: &App) -> Option<(i64, i64, Option<String>)> {
    if app.view() == View::Issues {
        return app
            .selected_issue_row()
            .map(|issue| (issue.id, issue.number, Some(issue.state.clone())));
    }

    if matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        if let Some(issue) = app.current_issue_row() {
            return Some((issue.id, issue.number, Some(issue.state.clone())));
        }
        if let (Some(issue_id), Some(issue_number)) =
            (app.current_issue_id(), app.current_issue_number())
        {
            return Some((issue_id, issue_number, None));
        }
    }

    None
}

fn selected_issue_labels(app: &App) -> Option<String> {
    if app.view() == View::Issues {
        return app.selected_issue_row().map(|issue| issue.labels.clone());
    }
    if matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles | View::CommentEditor
    ) {
        return app.current_issue_row().map(|issue| issue.labels.clone());
    }
    None
}

fn selected_issue_assignees(app: &App) -> Option<String> {
    if app.view() == View::Issues {
        return app
            .selected_issue_row()
            .map(|issue| issue.assignees.clone());
    }
    if matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles | View::CommentEditor
    ) {
        return app.current_issue_row().map(|issue| issue.assignees.clone());
    }
    None
}

fn label_options_for_repo(app: &App) -> Vec<String> {
    let mut labels = app
        .issues()
        .iter()
        .flat_map(|issue| issue.labels.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<String>>();
    labels.sort_by_key(|value| value.to_ascii_lowercase());
    labels.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    labels
}

fn assignee_options_for_repo(app: &App) -> Vec<String> {
    let mut assignees = app
        .issues()
        .iter()
        .flat_map(|issue| issue.assignees.split(','))
        .map(str::trim)
        .map(|value| value.trim_start_matches('@'))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<String>>();
    assignees.sort_by_key(|value| value.to_ascii_lowercase());
    assignees.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    assignees
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

fn issue_number(app: &App) -> Option<i64> {
    match app.view() {
        View::IssueDetail
        | View::IssueComments
        | View::PullRequestFiles
        | View::LabelPicker
        | View::AssigneePicker
        | View::CommentPresetPicker
        | View::CommentPresetName
        | View::CommentEditor => app.current_issue_number(),
        View::Issues => app.selected_issue_row().map(|issue| issue.number),
        _ => None,
    }
}

fn issue_url(app: &App) -> Option<String> {
    let owner = app.current_owner()?;
    let repo = app.current_repo()?;
    let issue = app.current_or_selected_issue()?;
    let issue_number = issue.number;
    let route = if issue.is_pr { "pull" } else { "issues" };

    Some(format!(
        "https://github.com/{}/{}/{}/{}",
        owner, repo, route, issue_number
    ))
}

fn checkout_pull_request(app: &mut App) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    if !issue.is_pr {
        app.set_status("Selected item is not a pull request".to_string());
        return Ok(());
    }

    let working_dir = app.current_repo_path().unwrap_or(".").to_string();
    let issue_number = issue.number;
    let number = issue_number.to_string();
    let before_branch = current_git_branch(working_dir.as_str());
    let before_head = current_git_head(working_dir.as_str());

    let output = std::process::Command::new("gh")
        .args(["pr", "checkout", number.as_str()])
        .current_dir(working_dir.as_str())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            app.set_status(format!("PR checkout failed: {}", error));
            return Ok(());
        }
    };

    if output.status.success() {
        return finalize_checkout_status(
            app,
            working_dir.as_str(),
            issue_number,
            before_branch,
            before_head,
        );
    }

    let detached_output = std::process::Command::new("gh")
        .args(["pr", "checkout", number.as_str(), "--detach"])
        .current_dir(working_dir.as_str())
        .output();

    if detached_output
        .as_ref()
        .is_ok_and(|out| out.status.success())
    {
        return finalize_checkout_status(
            app,
            working_dir.as_str(),
            issue_number,
            before_branch,
            before_head,
        );
    }

    let primary_message = command_error_message(&output);
    let detached_message = detached_output
        .as_ref()
        .map(command_error_message)
        .unwrap_or_else(|error| error.to_string());
    let combined = if detached_message.is_empty() || detached_message == primary_message {
        primary_message
    } else if primary_message.is_empty() {
        detached_message
    } else {
        format!("{}; fallback failed: {}", primary_message, detached_message)
    };

    if combined.is_empty() {
        app.set_status(format!("PR checkout failed for #{}", issue_number));
        return Ok(());
    }

    app.set_status(format!("PR checkout failed: {}", combined));
    Ok(())
}

fn finalize_checkout_status(
    app: &mut App,
    working_dir: &str,
    issue_number: i64,
    before_branch: Option<String>,
    before_head: Option<String>,
) -> Result<()> {
    let after_branch = current_git_branch(working_dir);
    let after_head = current_git_head(working_dir);

    if before_branch == after_branch && before_head == after_head {
        if let Some(branch) = after_branch {
            app.set_status(format!(
                "PR #{} already active on {} (no checkout changes)",
                issue_number, branch
            ));
            return Ok(());
        }
        app.set_status(format!(
            "PR #{} already active (no checkout changes)",
            issue_number
        ));
        return Ok(());
    }

    if let Some(branch) = after_branch {
        app.set_status(format!("Checked out PR #{} on {}", issue_number, branch));
        return Ok(());
    }

    app.set_status(format!("Checked out PR #{}", issue_number));
    Ok(())
}

fn command_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(output.stderr.as_slice())
        .trim()
        .to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string()
}

fn current_git_branch(working_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

fn current_git_head(working_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

fn copy_status_to_clipboard(app: &mut App) -> Result<()> {
    let status = app.status().trim();
    if status.is_empty() {
        app.set_status("No status text to copy".to_string());
        return Ok(());
    }

    match write_clipboard(status) {
        Ok(()) => app.set_status("Copied status to clipboard".to_string()),
        Err(error) => app.set_status(format!("Clipboard copy failed: {}", error)),
    }
    Ok(())
}

fn write_clipboard(value: &str) -> Result<()> {
    if cfg!(target_os = "macos") {
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(value.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            return Ok(());
        }
        anyhow::bail!("pbcopy exited with status {}", status);
    }

    if cfg!(target_os = "windows") {
        let mut child = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(value.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            return Ok(());
        }
        anyhow::bail!("clip exited with status {}", status);
    }

    let mut child = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(value.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        return Ok(());
    }

    anyhow::bail!("clipboard command exited with status {}", status)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkedPullRequestTarget {
    Tui,
    Browser,
    Probe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkedIssueTarget {
    Tui,
    Browser,
    Probe,
}

fn open_linked_pull_request(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    target: LinkedPullRequestTarget,
) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };
    if issue.is_pr {
        app.set_status("Selected item is already a pull request".to_string());
        return Ok(());
    }

    let issue_number = issue.number;
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_linked_pull_request_lookup(
        owner,
        repo,
        issue_number,
        token.to_string(),
        event_tx,
        target,
    );
    if target == LinkedPullRequestTarget::Tui {
        app.set_status("Looking up linked pull request for TUI...".to_string());
        return Ok(());
    }
    app.set_status("Looking up linked pull request for browser...".to_string());
    Ok(())
}

fn open_linked_issue(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    target: LinkedIssueTarget,
) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };
    if !issue.is_pr {
        app.set_status("Selected item is not a pull request".to_string());
        return Ok(());
    }

    let pull_number = issue.number;
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => {
            app.set_status("No repo selected".to_string());
            return Ok(());
        }
    };

    start_linked_issue_lookup(
        owner,
        repo,
        pull_number,
        token.to_string(),
        event_tx,
        target,
    );
    if target == LinkedIssueTarget::Tui {
        app.set_status("Looking up linked issue for TUI...".to_string());
        return Ok(());
    }
    app.set_status("Looking up linked issue for browser...".to_string());
    Ok(())
}

fn open_pull_request_in_tui(
    app: &mut App,
    conn: &rusqlite::Connection,
    pull_number: i64,
) -> Result<bool> {
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::PullRequests);

    let try_filters = [IssueFilter::Open, IssueFilter::Closed];
    for filter in try_filters {
        app.set_issue_filter(filter);
        if !app.select_issue_by_number(pull_number) {
            continue;
        }

        let (issue_id, issue_number) = match app.selected_issue_row() {
            Some(issue) => (issue.id, issue.number),
            None => return Ok(false),
        };
        app.set_current_issue(issue_id, issue_number);
        app.reset_issue_detail_scroll();
        load_comments_for_issue(app, conn, issue_id)?;
        app.set_view(View::IssueDetail);
        app.set_comment_syncing(false);
        app.request_comment_sync();
        app.request_pull_request_files_sync();
        app.request_pull_request_review_comments_sync();
        return Ok(true);
    }

    Ok(false)
}

fn open_issue_in_tui(
    app: &mut App,
    conn: &rusqlite::Connection,
    issue_number: i64,
) -> Result<bool> {
    app.set_view(View::Issues);
    app.set_work_item_mode(WorkItemMode::Issues);

    let try_filters = [IssueFilter::Open, IssueFilter::Closed];
    for filter in try_filters {
        app.set_issue_filter(filter);
        if !app.select_issue_by_number(issue_number) {
            continue;
        }

        let (issue_id, issue_number) = match app.selected_issue_row() {
            Some(issue) => (issue.id, issue.number),
            None => return Ok(false),
        };
        app.set_current_issue(issue_id, issue_number);
        app.reset_issue_detail_scroll();
        load_comments_for_issue(app, conn, issue_id)?;
        app.set_view(View::IssueDetail);
        app.set_comment_syncing(false);
        app.request_comment_sync();
        return Ok(true);
    }

    Ok(false)
}

fn start_linked_pull_request_lookup(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
    target: LinkedPullRequestTarget,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::LinkedPullRequestLookupFailed {
                    issue_number,
                    message: error.to_string(),
                    target,
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::LinkedPullRequestLookupFailed {
                    issue_number,
                    message: error.to_string(),
                    target,
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .find_linked_pull_request(&owner, &repo, issue_number)
                .await
        });

        match result {
            Ok(linked) => {
                let (pull_number, url) = match linked {
                    Some((pull_number, url)) => (Some(pull_number), Some(url)),
                    None => (None, None),
                };
                let _ = event_tx.send(AppEvent::LinkedPullRequestResolved {
                    issue_number,
                    pull_number,
                    url,
                    target,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::LinkedPullRequestLookupFailed {
                    issue_number,
                    message: error.to_string(),
                    target,
                });
            }
        }
    });
}

fn start_linked_issue_lookup(
    owner: String,
    repo: String,
    pull_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
    target: LinkedIssueTarget,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::LinkedIssueLookupFailed {
                    pull_number,
                    message: error.to_string(),
                    target,
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::LinkedIssueLookupFailed {
                    pull_number,
                    message: error.to_string(),
                    target,
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .find_linked_issue_for_pull_request(&owner, &repo, pull_number)
                .await
        });

        match result {
            Ok(linked) => {
                let (issue_number, url) = match linked {
                    Some((issue_number, url)) => (Some(issue_number), Some(url)),
                    None => (None, None),
                };
                let _ = event_tx.send(AppEvent::LinkedIssueResolved {
                    pull_number,
                    issue_number,
                    url,
                    target,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::LinkedIssueLookupFailed {
                    pull_number,
                    message: error.to_string(),
                    target,
                });
            }
        }
    });
}

fn open_url(url: &str) -> Result<()> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()?;
        return Ok(());
    }

    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .status()?;
        return Ok(());
    }

    std::process::Command::new("xdg-open").arg(url).status()?;
    Ok(())
}

fn load_issues_for_slug(
    app: &mut App,
    conn: &rusqlite::Connection,
    owner: &str,
    repo: &str,
    repo_path: Option<&str>,
) -> Result<()> {
    app.set_current_repo_with_path(owner, repo, repo_path);
    let repo_row = get_repo_by_slug(conn, owner, repo)?;
    let repo_row = match repo_row {
        Some(repo_row) => repo_row,
        None => {
            app.set_issues(Vec::new());
            app.set_status("No cached issues yet. Press r to sync.".to_string());
            app.request_sync();
            return Ok(());
        }
    };
    let issues = list_issues(conn, repo_row.id)?;
    app.set_issues(issues);
    app.set_status(format!("{}/{}", owner, repo));
    Ok(())
}

fn load_comments_for_issue(
    app: &mut App,
    conn: &rusqlite::Connection,
    issue_id: i64,
) -> Result<()> {
    let comments = comments_for_issue(conn, issue_id)?;
    app.set_comments(comments);
    let now = comment_now_epoch();
    touch_comments_for_issue(conn, issue_id, now)?;
    prune_comments(conn, COMMENT_TTL_SECONDS, COMMENT_CAP)?;
    Ok(())
}

fn load_repos(conn: &rusqlite::Connection) -> Result<Vec<crate::store::LocalRepoRow>> {
    list_local_repos(conn)
}

fn maybe_start_scan(app: &App, event_tx: Sender<AppEvent>) -> Result<()> {
    if app.view() != View::RepoPicker {
        return Ok(());
    }

    let mode = if app.repos().is_empty() {
        ScanMode::QuickAndFull
    } else {
        ScanMode::QuickOnly
    };

    start_scan(event_tx, mode)
}

fn maybe_start_rescan(app: &mut App, event_tx: Sender<AppEvent>) -> Result<()> {
    if !app.take_rescan_request() {
        return Ok(());
    }

    start_scan(event_tx, ScanMode::FullOnly)
}

fn start_scan(event_tx: Sender<AppEvent>, mode: ScanMode) -> Result<()> {
    let cwd = env::current_dir()?;
    let home = home_dir().unwrap_or(cwd.clone());
    thread::spawn(move || {
        let conn = match crate::store::open_db() {
            Ok(conn) => conn,
            Err(_) => return,
        };

        if matches!(mode, ScanMode::QuickOnly | ScanMode::QuickAndFull) {
            let quick = quick_scan(&cwd, 4, 2).unwrap_or_default();
            for repo in &quick {
                let _ = index_repo_path(&conn, &repo.path);
            }
            let _ = event_tx.send(AppEvent::ReposUpdated);
        }

        if matches!(mode, ScanMode::FullOnly | ScanMode::QuickAndFull) {
            let full = crate::discovery::full_scan(&home).unwrap_or_default();
            for repo in &full {
                let _ = index_repo_path(&conn, &repo.path);
            }
            let _ = event_tx.send(AppEvent::ReposUpdated);
        }

        let _ = event_tx.send(AppEvent::ScanFinished);
    });

    Ok(())
}

fn handle_events(
    app: &mut App,
    conn: &rusqlite::Connection,
    event_rx: &Receiver<AppEvent>,
) -> Result<()> {
    while let Ok(event) = event_rx.try_recv() {
        match event {
            AppEvent::ReposUpdated => {
                if app.view() == View::RepoPicker {
                    app.set_repos(load_repos(conn)?);
                    app.set_status(String::new());
                }
            }
            AppEvent::ScanFinished => {
                app.set_scanning(false);
                if app.view() == View::RepoPicker {
                    app.set_status(String::new());
                }
            }
            AppEvent::SyncFinished { owner, repo, stats } => {
                app.set_syncing(false);
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    refresh_current_repo_issues(app, conn)?;
                    let (open_count, closed_count) = app.issue_counts();
                    if stats.not_modified {
                        app.set_status(format!(
                            "No issue changes (open: {}, closed: {})",
                            open_count, closed_count
                        ));
                        continue;
                    }
                    app.set_status(format!(
                        "Synced {} issues (open: {}, closed: {})",
                        stats.issues, open_count, closed_count
                    ));
                }
            }
            AppEvent::SyncProgress {
                owner,
                repo,
                page,
                stats,
            } => {
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    refresh_current_repo_issues(app, conn)?;
                    let (open_count, closed_count) = app.issue_counts();
                    app.set_status(format!(
                        "Syncing page {}: {} issues cached (open: {}, closed: {})",
                        page, stats.issues, open_count, closed_count
                    ));
                }
            }
            AppEvent::SyncFailed {
                owner,
                repo,
                message,
            } => {
                app.set_syncing(false);
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    app.set_status(format!("Sync failed: {}", message));
                }
            }
            AppEvent::CommentsUpdated { issue_id, count } => {
                app.set_comment_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    load_comments_for_issue(app, conn, issue_id)?;
                    app.set_status(format!("Updated {} comments", count));
                }
            }
            AppEvent::CommentsFailed { issue_id, message } => {
                app.set_comment_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Comments unavailable: {}", message));
                }
            }
            AppEvent::IssueUpdated {
                issue_number,
                message,
            } => {
                if message.starts_with("closed")
                    || message.starts_with("close failed")
                    || message.starts_with("reopened")
                    || message.starts_with("reopen failed")
                    || message.starts_with("label update failed")
                    || message.starts_with("assignee update failed")
                {
                    app.clear_pending_issue_action(issue_number);
                }
                if message.starts_with("closed") {
                    app.update_issue_state_by_number(issue_number, "closed");
                }
                if message.starts_with("reopened") {
                    app.update_issue_state_by_number(issue_number, "open");
                }
                app.set_status(format!("#{} {}", issue_number, message));
                app.request_sync();
                if app.current_issue_number() == Some(issue_number) {
                    app.request_comment_sync();
                }
            }
            AppEvent::IssueLabelsUpdated {
                issue_number,
                labels,
            } => {
                app.clear_pending_issue_action(issue_number);
                app.update_issue_labels_by_number(issue_number, labels.as_str());
                app.set_status(format!("#{} labels updated", issue_number));
                app.request_sync();
            }
            AppEvent::IssueAssigneesUpdated {
                issue_number,
                assignees,
            } => {
                app.clear_pending_issue_action(issue_number);
                app.update_issue_assignees_by_number(issue_number, assignees.as_str());
                app.set_status(format!("#{} assignees updated", issue_number));
                app.request_sync();
            }
            AppEvent::PullRequestFilesUpdated {
                issue_id,
                files,
                pull_request_id,
                viewed_files,
            } => {
                app.set_pull_request_files_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    let count = files.len();
                    app.set_pull_request_files(issue_id, files);
                    app.set_pull_request_view_state(pull_request_id, viewed_files);
                    app.set_status(format!("Loaded {} changed files", count));
                }
            }
            AppEvent::PullRequestFilesFailed { issue_id, message } => {
                app.set_pull_request_files_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("PR files unavailable: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentsUpdated { issue_id, comments } => {
                app.set_pull_request_review_comments_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    let count = comments.len();
                    app.set_pull_request_review_comments(comments);
                    app.set_status(format!("Loaded {} review comments", count));
                }
            }
            AppEvent::PullRequestReviewCommentsFailed { issue_id, message } => {
                app.set_pull_request_review_comments_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("PR review comments unavailable: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentCreated { issue_id } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.request_pull_request_review_comments_sync();
                    app.set_status("Review comment submitted".to_string());
                }
            }
            AppEvent::PullRequestReviewCommentCreateFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review comment failed: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentUpdated {
                issue_id,
                comment_id,
                body,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.update_pull_request_review_comment_body_by_id(comment_id, body.as_str());
                    app.set_status("Review comment updated".to_string());
                }
            }
            AppEvent::PullRequestReviewCommentUpdateFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review comment update failed: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentDeleted {
                issue_id,
                comment_id,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.remove_pull_request_review_comment_by_id(comment_id);
                    app.set_status("Review comment deleted".to_string());
                }
            }
            AppEvent::PullRequestReviewCommentDeleteFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review comment delete failed: {}", message));
                }
            }
            AppEvent::PullRequestReviewThreadResolutionUpdated { issue_id, resolved } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.request_pull_request_review_comments_sync();
                    if resolved {
                        app.set_status("Review thread resolved".to_string());
                    } else {
                        app.set_status("Review thread reopened".to_string());
                    }
                }
            }
            AppEvent::PullRequestReviewThreadResolutionFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review thread resolution failed: {}", message));
                }
            }
            AppEvent::PullRequestFileViewedUpdated {
                issue_id,
                path,
                viewed,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_pull_request_file_viewed(path.as_str(), viewed);
                    if viewed {
                        app.set_status(format!("Marked {} viewed on GitHub", path));
                    } else {
                        app.set_status(format!("Marked {} unviewed on GitHub", path));
                    }
                }
            }
            AppEvent::PullRequestFileViewedUpdateFailed {
                issue_id,
                path,
                viewed,
                message,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_pull_request_file_viewed(path.as_str(), !viewed);
                    app.set_status(format!(
                        "GitHub view state failed for {}: {}",
                        path, message
                    ));
                }
            }
            AppEvent::LinkedPullRequestResolved {
                issue_number,
                pull_number,
                url,
                target,
            } => {
                app.set_linked_pull_request(issue_number, pull_number);
                let pull_number = match pull_number {
                    Some(pull_number) => pull_number,
                    None => {
                        if target == LinkedPullRequestTarget::Probe {
                            continue;
                        }
                        app.set_status(format!(
                            "No linked pull request found for #{}",
                            issue_number
                        ));
                        continue;
                    }
                };

                if target == LinkedPullRequestTarget::Probe {
                    continue;
                }

                if target == LinkedPullRequestTarget::Tui {
                    refresh_current_repo_issues(app, conn)?;
                    if open_pull_request_in_tui(app, conn, pull_number)? {
                        app.set_status(format!(
                            "Opened linked pull request #{} in TUI",
                            pull_number
                        ));
                        continue;
                    }

                    app.set_status(format!(
                        "Linked PR #{} not cached in TUI yet; press r then Shift+P",
                        pull_number
                    ));
                    continue;
                }

                let browser_url = match url {
                    Some(url) => Some(url),
                    None => {
                        if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo())
                        {
                            Some(format!(
                                "https://github.com/{}/{}/pull/{}",
                                owner, repo, pull_number
                            ))
                        } else {
                            None
                        }
                    }
                };

                if let Some(browser_url) = browser_url {
                    if let Err(error) = open_url(browser_url.as_str()) {
                        app.set_status(format!("Open linked PR failed: {}", error));
                        continue;
                    }
                    app.set_status(format!(
                        "Opened linked pull request #{} in browser",
                        pull_number
                    ));
                    continue;
                }

                app.set_status(format!(
                    "Linked PR #{} found but URL unavailable",
                    pull_number
                ));
            }
            AppEvent::LinkedPullRequestLookupFailed {
                issue_number,
                message,
                target,
            } => {
                app.end_linked_pull_request_lookup(issue_number);
                if target == LinkedPullRequestTarget::Probe {
                    continue;
                }
                let target_label = match target {
                    LinkedPullRequestTarget::Tui => "TUI",
                    LinkedPullRequestTarget::Browser => "browser",
                    LinkedPullRequestTarget::Probe => "probe",
                };
                app.set_status(format!(
                    "Linked pull request lookup failed for #{} ({}): {}",
                    issue_number, target_label, message
                ));
            }
            AppEvent::LinkedIssueResolved {
                pull_number,
                issue_number,
                url,
                target,
            } => {
                app.set_linked_issue_for_pull_request(pull_number, issue_number);
                let issue_number = match issue_number {
                    Some(issue_number) => issue_number,
                    None => {
                        if target == LinkedIssueTarget::Probe {
                            continue;
                        }
                        app.set_status(format!("No linked issue found for PR #{}", pull_number));
                        continue;
                    }
                };

                if target == LinkedIssueTarget::Probe {
                    continue;
                }

                if target == LinkedIssueTarget::Tui {
                    refresh_current_repo_issues(app, conn)?;
                    if open_issue_in_tui(app, conn, issue_number)? {
                        app.set_status(format!("Opened linked issue #{} in TUI", issue_number));
                        continue;
                    }

                    app.set_status(format!(
                        "Linked issue #{} not cached in TUI yet; press r then Shift+P",
                        issue_number
                    ));
                    continue;
                }

                let browser_url = match url {
                    Some(url) => Some(url),
                    None => {
                        if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo())
                        {
                            Some(format!(
                                "https://github.com/{}/{}/issues/{}",
                                owner, repo, issue_number
                            ))
                        } else {
                            None
                        }
                    }
                };

                if let Some(browser_url) = browser_url {
                    if let Err(error) = open_url(browser_url.as_str()) {
                        app.set_status(format!("Open linked issue failed: {}", error));
                        continue;
                    }
                    app.set_status(format!("Opened linked issue #{} in browser", issue_number));
                    continue;
                }

                app.set_status(format!(
                    "Linked issue #{} found but URL unavailable",
                    issue_number
                ));
            }
            AppEvent::LinkedIssueLookupFailed {
                pull_number,
                message,
                target,
            } => {
                app.end_linked_issue_lookup(pull_number);
                if target == LinkedIssueTarget::Probe {
                    continue;
                }
                let target_label = match target {
                    LinkedIssueTarget::Tui => "TUI",
                    LinkedIssueTarget::Browser => "browser",
                    LinkedIssueTarget::Probe => "probe",
                };
                app.set_status(format!(
                    "Linked issue lookup failed for PR #{} ({}): {}",
                    pull_number, target_label, message
                ));
            }
            AppEvent::IssueCommentUpdated {
                issue_number,
                comment_id,
                body,
            } => {
                app.update_comment_body_by_id(comment_id, body.as_str());
                app.set_status(format!("#{} comment updated", issue_number));
                app.request_comment_sync();
                app.request_sync();
            }
            AppEvent::IssueCommentDeleted {
                issue_number,
                comment_id,
                count,
            } => {
                app.remove_comment_by_id(comment_id);
                app.update_issue_comments_count_by_number(issue_number, count as i64);
                app.set_status(format!("#{} comment deleted", issue_number));
                app.request_comment_sync();
                app.request_sync();
            }
            AppEvent::RepoLabelsSuggested {
                owner,
                repo,
                labels,
            } => {
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                    && app.view() == View::LabelPicker
                {
                    app.merge_label_options(labels);
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    QuickOnly,
    QuickAndFull,
    FullOnly,
}

#[derive(Debug, Clone)]
enum AppEvent {
    ReposUpdated,
    ScanFinished,
    SyncProgress {
        owner: String,
        repo: String,
        page: u32,
        stats: SyncStats,
    },
    SyncFinished {
        owner: String,
        repo: String,
        stats: SyncStats,
    },
    SyncFailed {
        owner: String,
        repo: String,
        message: String,
    },
    CommentsUpdated {
        issue_id: i64,
        count: usize,
    },
    CommentsFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestFilesUpdated {
        issue_id: i64,
        files: Vec<PullRequestFile>,
        pull_request_id: Option<String>,
        viewed_files: HashSet<String>,
    },
    PullRequestFilesFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestReviewCommentsUpdated {
        issue_id: i64,
        comments: Vec<PullRequestReviewComment>,
    },
    PullRequestReviewCommentsFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestReviewCommentCreated {
        issue_id: i64,
    },
    PullRequestReviewCommentCreateFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestReviewCommentUpdated {
        issue_id: i64,
        comment_id: i64,
        body: String,
    },
    PullRequestReviewCommentUpdateFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestReviewCommentDeleted {
        issue_id: i64,
        comment_id: i64,
    },
    PullRequestReviewCommentDeleteFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestReviewThreadResolutionUpdated {
        issue_id: i64,
        resolved: bool,
    },
    PullRequestReviewThreadResolutionFailed {
        issue_id: i64,
        message: String,
    },
    PullRequestFileViewedUpdated {
        issue_id: i64,
        path: String,
        viewed: bool,
    },
    PullRequestFileViewedUpdateFailed {
        issue_id: i64,
        path: String,
        viewed: bool,
        message: String,
    },
    LinkedPullRequestResolved {
        issue_number: i64,
        pull_number: Option<i64>,
        url: Option<String>,
        target: LinkedPullRequestTarget,
    },
    LinkedPullRequestLookupFailed {
        issue_number: i64,
        message: String,
        target: LinkedPullRequestTarget,
    },
    LinkedIssueResolved {
        pull_number: i64,
        issue_number: Option<i64>,
        url: Option<String>,
        target: LinkedIssueTarget,
    },
    LinkedIssueLookupFailed {
        pull_number: i64,
        message: String,
        target: LinkedIssueTarget,
    },
    IssueUpdated {
        issue_number: i64,
        message: String,
    },
    IssueLabelsUpdated {
        issue_number: i64,
        labels: String,
    },
    IssueAssigneesUpdated {
        issue_number: i64,
        assignees: String,
    },
    IssueCommentUpdated {
        issue_number: i64,
        comment_id: i64,
        body: String,
    },
    IssueCommentDeleted {
        issue_number: i64,
        comment_id: i64,
        count: usize,
    },
    RepoLabelsSuggested {
        owner: String,
        repo: String,
        labels: Vec<String>,
    },
}

fn maybe_start_repo_sync(app: &mut App, token: &str, event_tx: Sender<AppEvent>) -> Result<()> {
    if app.syncing() {
        return Ok(());
    }

    if !app.take_sync_request() {
        return Ok(());
    }

    let owner = match app.current_owner() {
        Some(owner) => owner.to_string(),
        None => return Ok(()),
    };
    let repo = match app.current_repo() {
        Some(repo) => repo.to_string(),
        None => return Ok(()),
    };

    start_repo_sync(owner, repo, token.to_string(), event_tx);
    app.set_syncing(true);
    app.set_status("Syncing...".to_string());
    Ok(())
}

fn maybe_start_issue_poll(app: &mut App, last_poll: &mut Instant) {
    if !matches!(
        app.view(),
        View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return;
    }

    if last_poll.elapsed() < ISSUE_POLL_INTERVAL {
        return;
    }

    app.request_sync();
    *last_poll = Instant::now();
}

fn maybe_start_comment_poll(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    last_poll: &mut Instant,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }

    if app.comment_syncing() {
        return Ok(());
    }

    if !app.take_comment_sync_request() {
        if last_poll.elapsed() < COMMENT_POLL_INTERVAL {
            return Ok(());
        }
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    start_comment_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_comment_syncing(true);
    *last_poll = Instant::now();
    Ok(())
}

fn maybe_start_pull_request_files_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }
    if app.pull_request_files_syncing() {
        return Ok(());
    }
    if !app.take_pull_request_files_sync_request() {
        return Ok(());
    }
    if !app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    start_pull_request_files_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_pull_request_files_syncing(true);
    app.set_status("Loading pull request changes...".to_string());
    Ok(())
}

fn maybe_start_pull_request_review_comments_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }
    if app.pull_request_review_comments_syncing() {
        return Ok(());
    }
    if !app.take_pull_request_review_comments_sync_request() {
        return Ok(());
    }
    if !app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    start_pull_request_review_comments_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_pull_request_review_comments_syncing(true);
    Ok(())
}

fn start_repo_sync(owner: String, repo: String, token: String, event_tx: Sender<AppEvent>) {
    thread::spawn(move || {
        let conn = match crate::store::open_db() {
            Ok(conn) => conn,
            Err(error) => {
                let _ = event_tx.send(AppEvent::SyncFailed {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    message: error.to_string(),
                });
                return;
            }
        };
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::SyncFailed {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::SyncFailed {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    message: error.to_string(),
                });
                return;
            }
        };

        let progress_tx = event_tx.clone();
        let result = runtime.block_on(async {
            sync_repo_with_progress(&client, &conn, &owner, &repo, |page, stats| {
                let _ = progress_tx.send(AppEvent::SyncProgress {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    page,
                    stats: stats.clone(),
                });
            })
            .await
        });
        let stats = match result {
            Ok(stats) => stats,
            Err(error) => {
                let _ = event_tx.send(AppEvent::SyncFailed {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    message: error.to_string(),
                });
                return;
            }
        };
        let _ = event_tx.send(AppEvent::SyncFinished { owner, repo, stats });
    });
}

fn refresh_current_repo_issues(app: &mut App, conn: &rusqlite::Connection) -> Result<()> {
    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner, repo),
        _ => return Ok(()),
    };
    let repo_row = match get_repo_by_slug(conn, owner, repo)? {
        Some(repo_row) => repo_row,
        None => {
            app.set_issues(Vec::new());
            return Ok(());
        }
    };
    let issues = list_issues(conn, repo_row.id)?;
    app.set_issues(issues);
    Ok(())
}

fn start_comment_sync(
    owner: String,
    repo: String,
    issue_id: i64,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let conn = match crate::store::open_db() {
            Ok(conn) => conn,
            Err(error) => {
                let _ = event_tx.send(AppEvent::CommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::CommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::CommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result =
            runtime.block_on(async { client.list_comments(&owner, &repo, issue_number).await });
        let comments = match result {
            Ok(comments) => comments,
            Err(error) => {
                let _ = event_tx.send(AppEvent::CommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let now = comment_now_epoch();
        let mut count = 0usize;
        for comment in comments {
            let mut row = crate::sync::map_comment_to_row(issue_id, &comment);
            row.last_accessed_at = Some(now);
            let _ = crate::store::upsert_comment(&conn, &row);
            count += 1;
        }
        let _ = update_issue_comments_count(&conn, issue_id, count as i64);
        let _ = touch_comments_for_issue(&conn, issue_id, now);
        let _ = prune_comments(&conn, COMMENT_TTL_SECONDS, COMMENT_CAP);

        let _ = event_tx.send(AppEvent::CommentsUpdated { issue_id, count });
    });
}

fn start_pull_request_files_sync(
    owner: String,
    repo: String,
    issue_id: i64,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestFilesFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestFilesFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .list_pull_request_files(&owner, &repo, issue_number)
                .await
        });

        let files = match result {
            Ok(files) => files,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestFilesFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let (pull_request_id, viewed_files) = runtime
            .block_on(async {
                client
                    .pull_request_file_view_state(&owner, &repo, issue_number)
                    .await
            })
            .unwrap_or((None, HashSet::new()));

        let mapped = files
            .into_iter()
            .map(|file| PullRequestFile {
                filename: file.filename,
                status: file.status,
                additions: file.additions,
                deletions: file.deletions,
                patch: file.patch,
            })
            .collect::<Vec<PullRequestFile>>();
        let _ = event_tx.send(AppEvent::PullRequestFilesUpdated {
            issue_id,
            files: mapped,
            pull_request_id,
            viewed_files,
        });
    });
}

fn start_pull_request_review_comments_sync(
    owner: String,
    repo: String,
    issue_id: i64,
    pull_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .list_pull_request_review_comments(&owner, &repo, pull_number)
                .await
        });

        let comments = match result {
            Ok(comments) => comments,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentsFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let mut anchors = HashMap::new();
        for comment in &comments {
            let line = comment.line.or(comment.original_line);
            let side = comment
                .side
                .as_ref()
                .map(|value| {
                    if value.eq_ignore_ascii_case("left") {
                        ReviewSide::Left
                    } else {
                        ReviewSide::Right
                    }
                })
                .unwrap_or(ReviewSide::Right);
            if let Some(line) = line {
                anchors.insert(comment.id, (line, side, comment.path.clone()));
            }
        }

        let mut mapped = Vec::new();
        for comment in comments {
            let anchor = anchors.get(&comment.id).cloned().or_else(|| {
                comment
                    .in_reply_to_id
                    .and_then(|reply_to_id| anchors.get(&reply_to_id).cloned())
            });
            let (line, side, path, anchored) = match anchor {
                Some((line, side, path)) => (line, side, path, true),
                None => (0, ReviewSide::Right, comment.path.clone(), false),
            };

            mapped.push(PullRequestReviewComment {
                id: comment.id,
                thread_id: comment.thread_id,
                resolved: comment.is_resolved,
                anchored,
                path,
                line,
                side,
                body: comment.body.unwrap_or_default(),
                author: comment.user.login,
                created_at: comment.created_at,
            });
        }
        let _ = event_tx.send(AppEvent::PullRequestReviewCommentsUpdated {
            issue_id,
            comments: mapped,
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn start_create_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    pull_number: i64,
    path: String,
    line: i64,
    side: ReviewSide,
    start_line: Option<i64>,
    start_side: Option<ReviewSide>,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreateFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreateFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let head_sha = runtime.block_on(async {
            client
                .pull_request_head_sha(&owner, &repo, pull_number)
                .await
        });
        let head_sha = match head_sha {
            Ok(head_sha) => head_sha,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreateFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let created = runtime.block_on(async {
            client
                .create_pull_request_review_comment(
                    &owner,
                    &repo,
                    pull_number,
                    head_sha.as_str(),
                    path.as_str(),
                    line,
                    side.as_api_side(),
                    start_line,
                    start_side.map(ReviewSide::as_api_side),
                    body.as_str(),
                )
                .await
        });
        match created {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreated { issue_id });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreateFailed {
                    issue_id,
                    message: error.to_string(),
                });
            }
        }
    });
}

fn start_update_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    comment_id: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentUpdateFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentUpdateFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .update_pull_request_review_comment(&owner, &repo, comment_id, body.as_str())
                .await
        });
        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentUpdated {
                    issue_id,
                    comment_id,
                    body,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentUpdateFailed {
                    issue_id,
                    message: error.to_string(),
                });
            }
        }
    });
}

fn start_delete_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    comment_id: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentDeleteFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentDeleteFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .delete_pull_request_review_comment(&owner, &repo, comment_id)
                .await
        });
        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentDeleted {
                    issue_id,
                    comment_id,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewCommentDeleteFailed {
                    issue_id,
                    message: error.to_string(),
                });
            }
        }
    });
}

fn start_toggle_pull_request_review_thread_resolution(
    owner: String,
    repo: String,
    issue_id: i64,
    thread_id: String,
    resolve: bool,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewThreadResolutionFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewThreadResolutionFailed {
                    issue_id,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .set_pull_request_review_thread_resolved(&owner, &repo, thread_id.as_str(), resolve)
                .await
        });
        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewThreadResolutionUpdated {
                    issue_id,
                    resolved: resolve,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestReviewThreadResolutionFailed {
                    issue_id,
                    message: error.to_string(),
                });
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn start_set_pull_request_file_viewed(
    issue_id: i64,
    pull_request_id: String,
    path: String,
    viewed: bool,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestFileViewedUpdateFailed {
                    issue_id,
                    path,
                    viewed,
                    message: error.to_string(),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::PullRequestFileViewedUpdateFailed {
                    issue_id,
                    path,
                    viewed,
                    message: error.to_string(),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .set_pull_request_file_viewed(pull_request_id.as_str(), path.as_str(), viewed)
                .await
        });
        if result.is_ok() {
            let _ = event_tx.send(AppEvent::PullRequestFileViewedUpdated {
                issue_id,
                path,
                viewed,
            });
            return;
        }
        let _ = event_tx.send(AppEvent::PullRequestFileViewedUpdateFailed {
            issue_id,
            path,
            viewed,
            message: result
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default(),
        });
    });
}

fn start_add_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment failed: {}", error),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .create_comment(&owner, &repo, issue_number, &body)
                .await
        });

        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: "commented".to_string(),
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment failed: {}", error),
                });
            }
        }
    });
}

fn start_update_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    comment_id: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment update failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment update failed: {}", error),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .update_comment(&owner, &repo, comment_id, body.as_str())
                .await
        });

        match result {
            Ok(()) => {
                if let Ok(conn) = crate::store::open_db() {
                    let _ =
                        crate::store::update_comment_body_by_id(&conn, comment_id, body.as_str());
                }
                let _ = event_tx.send(AppEvent::IssueCommentUpdated {
                    issue_number,
                    comment_id,
                    body,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment update failed: {}", error),
                });
            }
        }
    });
}

fn start_delete_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    comment_id: i64,
    issue_id: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment delete failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment delete failed: {}", error),
                });
                return;
            }
        };

        let result =
            runtime.block_on(async { client.delete_comment(&owner, &repo, comment_id).await });

        match result {
            Ok(()) => {
                let mut count = 0usize;
                if let Ok(conn) = crate::store::open_db() {
                    let _ = crate::store::delete_comment_by_id(&conn, comment_id);
                    let comments =
                        crate::store::comments_for_issue(&conn, issue_id).unwrap_or_default();
                    count = comments.len();
                    let _ = update_issue_comments_count(&conn, issue_id, count as i64);
                }
                let _ = event_tx.send(AppEvent::IssueCommentDeleted {
                    issue_number,
                    comment_id,
                    count,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("comment delete failed: {}", error),
                });
            }
        }
    });
}

fn start_update_labels(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    labels: Vec<String>,
    event_tx: Sender<AppEvent>,
    labels_display: String,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("label update failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("label update failed: {}", error),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .update_issue_labels(&owner, &repo, issue_number, &labels)
                .await
        });
        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::IssueLabelsUpdated {
                    issue_number,
                    labels: labels_display,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("label update failed: {}", error),
                });
            }
        }
    });
}

fn start_update_assignees(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    assignees: Vec<String>,
    event_tx: Sender<AppEvent>,
    assignees_display: String,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("assignee update failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("assignee update failed: {}", error),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            client
                .update_issue_assignees(&owner, &repo, issue_number, &assignees)
                .await
        });
        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::IssueAssigneesUpdated {
                    issue_number,
                    assignees: assignees_display,
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("assignee update failed: {}", error),
                });
            }
        }
    });
}

fn start_fetch_labels(owner: String, repo: String, token: String, event_tx: Sender<AppEvent>) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(_) => return,
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };

        let labels = runtime.block_on(async { client.list_labels(&owner, &repo).await });
        if let Ok(labels) = labels {
            let _ = event_tx.send(AppEvent::RepoLabelsSuggested {
                owner,
                repo,
                labels,
            });
        }
    });
}

fn start_reopen_issue(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("reopen failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("reopen failed: {}", error),
                });
                return;
            }
        };

        let result =
            runtime.block_on(async { client.reopen_issue(&owner, &repo, issue_number).await });

        match result {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: "reopened".to_string(),
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("reopen failed: {}", error),
                });
            }
        }
    });
}

fn start_close_issue(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    body: Option<String>,
    event_tx: Sender<AppEvent>,
) {
    thread::spawn(move || {
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("close failed: {}", error),
                });
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("close failed: {}", error),
                });
                return;
            }
        };

        let result = runtime.block_on(async {
            let mut comment_error = None;
            if let Some(body) = body {
                if let Err(error) = client
                    .create_comment(&owner, &repo, issue_number, &body)
                    .await
                {
                    comment_error = Some(error.to_string());
                }
            }

            if let Err(error) = client.close_issue(&owner, &repo, issue_number).await {
                return Err(error);
            }

            Ok(comment_error)
        });

        match result {
            Ok(Some(comment_error)) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("closed (comment failed: {})", comment_error),
                });
            }
            Ok(None) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: "closed".to_string(),
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueUpdated {
                    issue_number,
                    message: format!("close failed: {}", error),
                });
            }
        }
    });
}

struct TerminalGuard {
    terminal: Tui,
}

impl TerminalGuard {
    fn init() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Tui {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::{issue_url, parse_csv_values};
    use crate::app::View;
    use crate::config::Config;
    use crate::store::IssueRow;

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
        app.set_current_repo_with_path("acme", "glyph", None);
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 10,
            repo_id: 1,
            number: 42,
            state: "open".to_string(),
            title: "Improve docs".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        }]);
        app.set_current_issue(10, 42);
        app.set_view(View::IssueDetail);

        let url = issue_url(&app).expect("url");

        assert_eq!(url, "https://github.com/acme/glyph/pull/42");
    }

    #[test]
    fn issue_url_uses_issue_route_for_issues() {
        let mut app = crate::app::App::new(Config::default());
        app.set_current_repo_with_path("acme", "glyph", None);
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 11,
            repo_id: 1,
            number: 7,
            state: "open".to_string(),
            title: "Bug".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        let url = issue_url(&app).expect("url");

        assert_eq!(url, "https://github.com/acme/glyph/issues/7");
    }
}
