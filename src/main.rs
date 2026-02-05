mod app;
mod auth;
mod cli;
mod config;
mod discovery;
mod git;
mod github;
mod repo_index;
mod sync;
mod store;
mod ui;

use std::collections::HashSet;
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

use crate::app::{App, AppAction, View};
use crate::auth::{clear_auth_token, resolve_auth_token, SystemAuth};
use crate::cli::{parse_args, CliCommand};
use crate::config::Config;
use crate::discovery::{home_dir, quick_scan};
use crate::git::list_github_remotes_at;
use crate::github::GitHubClient;
use crate::repo_index::index_repo_path;
use crate::store::delete_db;
use crate::sync::{sync_repo, SyncStats};
use crate::store::{get_repo_by_slug, list_issues, list_local_repos};

type TuiBackend = CrosstermBackend<Stdout>;
type Tui = Terminal<TuiBackend>;

const AUTH_DEBUG_ENV: &str = "GLYPH_AUTH_DEBUG";

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
    let auth = SystemAuth::new();
    let auth_token = resolve_auth_token(&auth)?;
    let client = GitHubClient::new(&auth_token.value)?;

    let home = home_dir().unwrap_or(env::current_dir()?);
    let repos = crate::discovery::full_scan(&home)?;
    let conn = crate::store::open_db()?;

    let mut indexed = 0usize;
    for repo in &repos {
        indexed += index_repo_path(&conn, &repo.path)?;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let mut seen = HashSet::new();
    let mut total = SyncStats::default();
    let start = Instant::now();

    runtime.block_on(async {
        for repo in &repos {
            let remotes = list_github_remotes_at(&repo.path)?;
            for remote in remotes {
                let key = format!("{}/{}", remote.slug.owner, remote.slug.repo);
                if !seen.insert(key) {
                    continue;
                }
                let stats = sync_repo(&client, &conn, &remote.slug.owner, &remote.slug.repo).await?;
                total.issues += stats.issues;
                total.comments += stats.comments;
            }
        }
        Ok::<(), anyhow::Error>(())
    })?;

    let duration = start.elapsed();
    println!(
        "Synced {} repos ({} remotes), {} issues, {} comments in {:.2?}",
        seen.len(),
        indexed,
        total.issues,
        total.comments,
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

    loop {
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

        handle_actions(app, conn)?;
        maybe_start_repo_sync(app, token, event_tx.clone())?;
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

fn handle_actions(app: &mut App, conn: &rusqlite::Connection) -> Result<()> {
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
    }
    Ok(())
}

fn load_issues_for_slug(
    app: &mut App,
    conn: &rusqlite::Connection,
    owner: &str,
    repo: &str,
) -> Result<()> {
    let repo_row = get_repo_by_slug(conn, owner, repo)?;
    let repo_row = match repo_row {
        Some(repo_row) => repo_row,
        None => {
            app.set_issues(Vec::new());
            app.set_status("No cached issues yet. Run `glyph sync`.");
            return Ok(());
        }
    };
    let issues = list_issues(conn, repo_row.id)?;
    app.set_issues(issues);
    app.set_status(format!("{}/{}", owner, repo));
    app.set_current_repo(owner, repo);
    Ok(())
}

fn load_repos(conn: &rusqlite::Connection) -> Result<Vec<crate::store::LocalRepoRow>> {
    list_local_repos(conn)
}

fn maybe_start_scan(app: &App, event_tx: Sender<AppEvent>) -> Result<()> {
    if app.view() != View::RepoPicker {
        return Ok(());
    }

    start_scan(event_tx, ScanMode::Initial)
}

fn maybe_start_rescan(app: &mut App, event_tx: Sender<AppEvent>) -> Result<()> {
    if !app.take_rescan_request() {
        return Ok(());
    }

    start_scan(event_tx, ScanMode::Full)
}

fn start_scan(event_tx: Sender<AppEvent>, mode: ScanMode) -> Result<()> {
    let cwd = env::current_dir()?;
    let home = home_dir().unwrap_or(cwd.clone());
    thread::spawn(move || {
        let conn = match crate::store::open_db() {
            Ok(conn) => conn,
            Err(_) => return,
        };

        if mode == ScanMode::Initial {
            let quick = quick_scan(&cwd, 4, 2).unwrap_or_default();
            for repo in &quick {
                let _ = index_repo_path(&conn, &repo.path);
            }
            let _ = event_tx.send(AppEvent::ReposUpdated);
        }

        let full = crate::discovery::full_scan(&home).unwrap_or_default();
        for repo in &full {
            let _ = index_repo_path(&conn, &repo.path);
        }
        let _ = event_tx.send(AppEvent::ReposUpdated);
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
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    Initial,
    Full,
}

#[derive(Debug, Clone)]
enum AppEvent {
    ReposUpdated,
    ScanFinished,
    SyncFinished { owner: String, repo: String, stats: SyncStats },
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

fn start_repo_sync(owner: String, repo: String, token: String, event_tx: Sender<AppEvent>) {
    thread::spawn(move || {
        let conn = match crate::store::open_db() {
            Ok(conn) => conn,
            Err(_) => return,
        };
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

        let result = runtime.block_on(async { sync_repo(&client, &conn, &owner, &repo).await });
        let stats = match result {
            Ok(stats) => stats,
            Err(_) => return,
        };
        let _ = event_tx.send(AppEvent::SyncFinished { owner, repo, stats });
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
