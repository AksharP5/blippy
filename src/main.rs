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

mod main_action_utils;
mod main_actions;
mod main_linked_actions;

mod main_data;
mod main_events;
mod main_sync;

use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{self, Stdout};
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
    App, AppAction, IssueFilter, LinkedPickerTarget, PendingIssueAction, PresetSelection,
    PullRequestFile, PullRequestReviewComment, ReviewSide, View, WorkItemMode,
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

use crate::main_sync::{
    start_add_comment, start_close_issue, start_create_issue,
    start_create_pull_request_review_comment, start_delete_comment,
    start_delete_pull_request_review_comment, start_fetch_assignees, start_reopen_issue,
    start_set_pull_request_file_viewed, start_toggle_pull_request_review_thread_resolution,
    start_update_assignees, start_update_comment, start_update_labels,
    start_update_pull_request_review_comment,
};

type TuiBackend = CrosstermBackend<Stdout>;
type Tui = Terminal<TuiBackend>;

struct WorkerServices {
    client: GitHubClient,
    runtime: tokio::runtime::Runtime,
}

struct WorkerContext {
    conn: rusqlite::Connection,
    services: WorkerServices,
}

enum WorkerSetupError {
    Db(String),
    Client(String),
    Runtime(String),
}

impl WorkerSetupError {
    fn into_message(self) -> String {
        match self {
            WorkerSetupError::Db(message)
            | WorkerSetupError::Client(message)
            | WorkerSetupError::Runtime(message) => message,
        }
    }
}

fn setup_worker_services(token: &str) -> Result<WorkerServices, WorkerSetupError> {
    let client = GitHubClient::new(token).map_err(|e| WorkerSetupError::Client(e.to_string()))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| WorkerSetupError::Runtime(e.to_string()))?;

    Ok(WorkerServices { client, runtime })
}

fn setup_worker_with_db(token: &str) -> Result<WorkerContext, WorkerSetupError> {
    let conn = crate::store::open_db().map_err(|e| WorkerSetupError::Db(e.to_string()))?;
    let services = setup_worker_services(token)?;
    Ok(WorkerContext { conn, services })
}

fn spawn_with_services<F, E>(token: String, event_tx: Sender<AppEvent>, on_setup_error: E, work: F)
where
    F: FnOnce(WorkerServices, Sender<AppEvent>) + Send + 'static,
    E: FnOnce(String) -> AppEvent + Send + 'static,
{
    thread::spawn(move || {
        let services = match setup_worker_services(&token) {
            Ok(services) => services,
            Err(error) => {
                let _ = event_tx.send(on_setup_error(error.into_message()));
                return;
            }
        };

        work(services, event_tx);
    });
}

fn spawn_with_db<F, E>(token: String, event_tx: Sender<AppEvent>, on_setup_error: E, work: F)
where
    F: FnOnce(WorkerContext, Sender<AppEvent>) + Send + 'static,
    E: FnOnce(String) -> AppEvent + Send + 'static,
{
    thread::spawn(move || {
        let ctx = match setup_worker_with_db(&token) {
            Ok(ctx) => ctx,
            Err(error) => {
                let _ = event_tx.send(on_setup_error(error.into_message()));
                return;
            }
        };

        work(ctx, event_tx);
    });
}

fn with_store_conn<F>(mut work: F)
where
    F: FnMut(&rusqlite::Connection),
{
    if let Ok(conn) = crate::store::open_db() {
        work(&conn);
    }
}

const AUTH_DEBUG_ENV: &str = "BLIPPY_AUTH_DEBUG";
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
    main_data::initialize_app(&mut app, &conn)?;

    let (event_tx, event_rx) = mpsc::channel();
    if app.view() == View::RepoPicker {
        app.set_scanning(true);
        app.set_status("Scanning");
    }
    main_data::maybe_start_scan(&app, event_tx.clone())?;

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
        CliCommand::Version => {
            println!("blippy {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
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

        main_events::handle_events(app, conn, &event_rx)?;
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

        main_actions::handle_actions(app, conn, token, event_tx.clone())?;
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
    main_sync::maybe_start_issue_poll(app, last_issue_poll);
    main_sync::maybe_start_repo_sync(app, token, event_tx.clone())?;
    main_sync::maybe_start_repo_permissions_sync(app, token, event_tx.clone());
    main_sync::maybe_start_repo_labels_sync(app, token, event_tx.clone());
    main_sync::maybe_start_comment_poll(app, token, event_tx.clone(), last_comment_poll)?;
    main_sync::maybe_start_pull_request_files_sync(app, token, event_tx.clone())?;
    main_sync::maybe_start_pull_request_review_comments_sync(app, token, event_tx.clone())?;
    main_linked_actions::maybe_probe_visible_linked_items(app, token, event_tx.clone());
    if app.view() == View::RepoPicker && app.repos().is_empty() {
        app.set_repos(main_data::load_repos(conn)?);
    }
    main_data::maybe_start_rescan(app, event_tx)?;
    Ok(())
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
        pull_requests: Vec<(i64, String)>,
        target: LinkedPullRequestTarget,
    },
    LinkedPullRequestLookupFailed {
        issue_number: i64,
        message: String,
        target: LinkedPullRequestTarget,
    },
    LinkedIssueResolved {
        pull_number: i64,
        issues: Vec<(i64, String)>,
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
    IssueCreated {
        issue_number: i64,
    },
    IssueCreateFailed {
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
        labels: Vec<(String, String)>,
    },
    RepoAssigneesSuggested {
        owner: String,
        repo: String,
        assignees: Vec<String>,
    },
    RepoPermissionsResolved {
        owner: String,
        repo: String,
        can_edit_issue_metadata: bool,
    },
    RepoPermissionsFailed {
        owner: String,
        repo: String,
        message: String,
    },
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
#[path = "main/tests.rs"]
mod tests;
