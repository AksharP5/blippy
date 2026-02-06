mod app;
mod auth;
mod cli;
mod config;
mod discovery;
mod git;
mod github;
mod markdown;
mod repo_index;
mod sync;
mod store;
mod ui;

use std::env;
use std::io::{self, Stdout};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::{App, AppAction, PresetSelection, View};
use crate::auth::{clear_auth_token, resolve_auth_token, SystemAuth};
use crate::cli::{parse_args, CliCommand};
use crate::config::Config;
use crate::discovery::{home_dir, quick_scan};
use crate::git::list_github_remotes_at;
use crate::github::GitHubClient;
use crate::repo_index::index_repo_path;
use crate::store::delete_db;
use crate::sync::{sync_repo, SyncStats};
use crate::store::{
    comment_now_epoch, comments_for_issue, get_repo_by_slug, list_issues, list_local_repos,
    prune_comments, touch_comments_for_issue, update_issue_comments_count,
};

type TuiBackend = CrosstermBackend<Stdout>;
type Tui = Terminal<TuiBackend>;

const AUTH_DEBUG_ENV: &str = "GLYPH_AUTH_DEBUG";
const ISSUE_POLL_INTERVAL: Duration = Duration::from_secs(60);
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
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    let mut last_issue_poll = Instant::now();
    let mut last_comment_poll = Instant::now();
    let mut last_view = app.view();

    loop {
        if app.view() != last_view {
            if matches!(last_view, View::IssueDetail | View::IssueComments) {
                app.set_comment_syncing(false);
            }
            last_view = app.view();
            last_issue_poll = Instant::now();
            last_comment_poll = Instant::now();
        }

        handle_events(app, conn, &event_rx)?;
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
            _ => {}
        }

        handle_actions(app, conn, token, event_tx.clone())?;
        maybe_start_issue_poll(app, &mut last_issue_poll);
        maybe_start_repo_sync(app, token, event_tx.clone())?;
        maybe_start_comment_poll(app, token, event_tx.clone(), &mut last_comment_poll)?;
        if app.view() == View::RepoPicker && app.repos().is_empty() {
            app.set_repos(load_repos(conn)?);
        }
        maybe_start_rescan(app, event_tx.clone())?;

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
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
            load_issues_for_slug(app, conn, &remote.slug.owner, &remote.slug.repo)?;
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
            let (owner, repo) = match app.repos().get(app.selected_repo()) {
                Some(repo) => (repo.owner.clone(), repo.repo.clone()),
                None => return Ok(()),
            };
            load_issues_for_slug(app, conn, &owner, &repo)?;
            app.set_view(View::Issues);
            app.request_sync();
        }
        AppAction::PickRemote => {
            let (owner, repo) = match app.remotes().get(app.selected_remote()) {
                Some(remote) => (remote.slug.owner.clone(), remote.slug.repo.clone()),
                None => return Ok(()),
            };
            load_issues_for_slug(app, conn, &owner, &repo)?;
            app.set_view(View::Issues);
            app.request_sync();
        }
        AppAction::PickIssue => {
            let (issue_id, issue_number) = match app.issues().get(app.selected_issue()) {
                Some(issue) => (issue.id, issue.number),
                None => return Ok(()),
            };
            app.set_current_issue(issue_id, issue_number);
            app.reset_issue_detail_scroll();
            load_comments_for_issue(app, conn, issue_id)?;
            app.set_view(View::IssueDetail);
            app.set_comment_syncing(false);
            app.request_comment_sync();
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
        AppAction::CloseIssue => {
            app.set_selected_preset(0);
            app.set_view(View::CommentPresetPicker);
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
            app.editor_mut().reset_for_close();
            app.set_view(View::CommentEditor);
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
    let (owner, repo, issue_number) = match (app.current_owner(), app.current_repo(), issue_number(app)) {
        (Some(owner), Some(repo), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_number)
        }
        _ => {
            app.set_status("No issue selected".to_string());
            return Ok(());
        }
    };

    start_close_issue(owner, repo, issue_number, token.to_string(), body, event_tx);
    app.set_view(View::Issues);
    app.set_status("Closing issue...".to_string());
    Ok(())
}

fn issue_number(app: &App) -> Option<i64> {
    match app.view() {
        View::IssueDetail | View::IssueComments => app.current_issue_number(),
        View::Issues => app.issues().get(app.selected_issue()).map(|issue| issue.number),
        _ => None,
    }
}

fn issue_url(app: &App) -> Option<String> {
    let owner = app.current_owner()?;
    let repo = app.current_repo()?;
    let issue_number = match app.view() {
        View::IssueDetail | View::IssueComments => app.current_issue_number(),
        View::Issues => app.issues().get(app.selected_issue()).map(|issue| issue.number),
        _ => None,
    }?;

    Some(format!(
        "https://github.com/{}/{}/issues/{}",
        owner, repo, issue_number
    ))
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
) -> Result<()> {
    app.set_current_repo(owner, repo);
    let repo_row = get_repo_by_slug(conn, owner, repo)?;
    let repo_row = match repo_row {
        Some(repo_row) => repo_row,
        None => {
            app.set_issues(Vec::new());
            app.set_status("No cached issues yet. Syncing...".to_string());
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
                    load_issues_for_slug(app, conn, &owner, &repo)?;
                    app.set_status(format!("Synced {} issues", stats.issues));
                }
            }
            AppEvent::SyncFailed { owner, repo, message } => {
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
            AppEvent::IssueClosed { issue_number, message } => {
                app.set_status(format!("#{} {}", issue_number, message));
                app.request_sync();
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
    SyncFinished { owner: String, repo: String, stats: SyncStats },
    SyncFailed { owner: String, repo: String, message: String },
    CommentsUpdated { issue_id: i64, count: usize },
    CommentsFailed { issue_id: i64, message: String },
    IssueClosed { issue_number: i64, message: String },
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
    if app.view() != View::Issues {
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
    if !matches!(app.view(), View::IssueDetail | View::IssueComments) {
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

    start_comment_sync(owner, repo, issue_id, issue_number, token.to_string(), event_tx);
    app.set_comment_syncing(true);
    *last_poll = Instant::now();
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

        let result = runtime.block_on(async { sync_repo(&client, &conn, &owner, &repo).await });
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

        let result = runtime.block_on(async { client.list_comments(&owner, &repo, issue_number).await });
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
                let _ = event_tx.send(AppEvent::IssueClosed {
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
                let _ = event_tx.send(AppEvent::IssueClosed {
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
                let _ = event_tx.send(AppEvent::IssueClosed {
                    issue_number,
                    message: format!("closed (comment failed: {})", comment_error),
                });
            }
            Ok(None) => {
                let _ = event_tx.send(AppEvent::IssueClosed {
                    issue_number,
                    message: "closed".to_string(),
                });
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::IssueClosed {
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
