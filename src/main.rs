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
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::auth::{clear_auth_token, resolve_auth_token, SystemAuth};
use crate::cli::{parse_args, CliCommand};
use crate::config::Config;
use crate::discovery::home_dir;
use crate::git::list_github_remotes_at;
use crate::github::GitHubClient;
use crate::repo_index::index_repo_path;
use crate::store::delete_db;
use crate::sync::{sync_repo, SyncStats};

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
    let _token = auth_token.value;

    let mut terminal_guard = TerminalGuard::init()?;
    let config = Config::load()?;
    let mut app = App::new(config);

    run_app(terminal_guard.terminal_mut(), &mut app)?;
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

fn run_app(terminal: &mut Tui, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
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

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
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
