use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Config;
use crate::git::RemoteInfo;
use crate::store::{CommentRow, IssueRow, LocalRepoRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    RepoPicker,
    RemoteChooser,
    Issues,
    IssueDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    PickRepo,
    PickRemote,
    PickIssue,
}

pub struct App {
    should_quit: bool,
    config: Config,
    view: View,
    repos: Vec<LocalRepoRow>,
    remotes: Vec<RemoteInfo>,
    issues: Vec<IssueRow>,
    comments: Vec<CommentRow>,
    selected_repo: usize,
    selected_remote: usize,
    selected_issue: usize,
    selected_comment: usize,
    status: String,
    scanning: bool,
    syncing: bool,
    comment_syncing: bool,
    comment_sync_requested: bool,
    sync_requested: bool,
    rescan_requested: bool,
    action: Option<AppAction>,
    current_owner: Option<String>,
    current_repo: Option<String>,
    current_issue_id: Option<i64>,
    current_issue_number: Option<i64>,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            should_quit: false,
            config,
            view: View::RepoPicker,
            repos: Vec::new(),
            remotes: Vec::new(),
            issues: Vec::new(),
            comments: Vec::new(),
            selected_repo: 0,
            selected_remote: 0,
            selected_issue: 0,
            selected_comment: 0,
            status: String::new(),
            scanning: false,
            syncing: false,
            comment_syncing: false,
            comment_sync_requested: false,
            sync_requested: false,
            rescan_requested: false,
            action: None,
            current_owner: None,
            current_repo: None,
            current_issue_id: None,
            current_issue_number: None,
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn repos(&self) -> &[LocalRepoRow] {
        &self.repos
    }

    pub fn remotes(&self) -> &[RemoteInfo] {
        &self.remotes
    }

    pub fn issues(&self) -> &[IssueRow] {
        &self.issues
    }

    pub fn comments(&self) -> &[CommentRow] {
        &self.comments
    }

    pub fn selected_repo(&self) -> usize {
        self.selected_repo
    }

    pub fn selected_remote(&self) -> usize {
        self.selected_remote
    }

    pub fn selected_issue(&self) -> usize {
        self.selected_issue
    }

    pub fn selected_comment(&self) -> usize {
        self.selected_comment
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn current_owner(&self) -> Option<&str> {
        self.current_owner.as_deref()
    }

    pub fn current_repo(&self) -> Option<&str> {
        self.current_repo.as_deref()
    }

    pub fn scanning(&self) -> bool {
        self.scanning
    }

    pub fn syncing(&self) -> bool {
        self.syncing
    }

    pub fn comment_syncing(&self) -> bool {
        self.comment_syncing
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            if self.view == View::RepoPicker {
                self.rescan_requested = true;
                self.scanning = true;
                self.status = "Scanning...".to_string();
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view = View::RepoPicker;
            }
            KeyCode::Esc if self.view == View::IssueDetail => {
                self.view = View::Issues;
            }
            KeyCode::Up => self.move_selection_up(),
            KeyCode::Down => self.move_selection_down(),
            KeyCode::Enter => self.activate_selection(),
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    pub fn set_repos(&mut self, repos: Vec<LocalRepoRow>) {
        self.repos = repos;
        self.selected_repo = 0;
    }

    pub fn set_remotes(&mut self, remotes: Vec<RemoteInfo>) {
        self.remotes = remotes;
        self.selected_remote = 0;
    }

    pub fn set_issues(&mut self, issues: Vec<IssueRow>) {
        self.issues = issues;
        self.selected_issue = 0;
    }

    pub fn set_comments(&mut self, comments: Vec<CommentRow>) {
        self.comments = comments;
        self.selected_comment = 0;
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn set_scanning(&mut self, scanning: bool) {
        self.scanning = scanning;
    }

    pub fn set_syncing(&mut self, syncing: bool) {
        self.syncing = syncing;
    }

    pub fn set_comment_syncing(&mut self, syncing: bool) {
        self.comment_syncing = syncing;
    }

    pub fn request_comment_sync(&mut self) {
        self.comment_sync_requested = true;
    }

    pub fn take_comment_sync_request(&mut self) -> bool {
        let requested = self.comment_sync_requested;
        self.comment_sync_requested = false;
        requested
    }

    pub fn request_sync(&mut self) {
        self.sync_requested = true;
    }

    pub fn take_sync_request(&mut self) -> bool {
        let requested = self.sync_requested;
        self.sync_requested = false;
        requested
    }

    pub fn set_current_repo(&mut self, owner: &str, repo: &str) {
        self.current_owner = Some(owner.to_string());
        self.current_repo = Some(repo.to_string());
    }

    pub fn set_current_issue(&mut self, issue_id: i64, issue_number: i64) {
        self.current_issue_id = Some(issue_id);
        self.current_issue_number = Some(issue_number);
    }

    pub fn current_issue_id(&self) -> Option<i64> {
        self.current_issue_id
    }

    pub fn current_issue_number(&self) -> Option<i64> {
        self.current_issue_number
    }

    pub fn take_rescan_request(&mut self) -> bool {
        let requested = self.rescan_requested;
        self.rescan_requested = false;
        requested
    }

    pub fn take_action(&mut self) -> Option<AppAction> {
        self.action.take()
    }

    fn move_selection_up(&mut self) {
        match self.view {
            View::RepoPicker => {
                if self.selected_repo > 0 {
                    self.selected_repo -= 1;
                }
            }
            View::RemoteChooser => {
                if self.selected_remote > 0 {
                    self.selected_remote -= 1;
                }
            }
            View::Issues => {
                if self.selected_issue > 0 {
                    self.selected_issue -= 1;
                }
            }
            View::IssueDetail => {
                if self.selected_comment > 0 {
                    self.selected_comment -= 1;
                }
            }
        }
    }

    fn move_selection_down(&mut self) {
        match self.view {
            View::RepoPicker => {
                if self.selected_repo + 1 < self.repos.len() {
                    self.selected_repo += 1;
                }
            }
            View::RemoteChooser => {
                if self.selected_remote + 1 < self.remotes.len() {
                    self.selected_remote += 1;
                }
            }
            View::Issues => {
                if self.selected_issue + 1 < self.issues.len() {
                    self.selected_issue += 1;
                }
            }
            View::IssueDetail => {
                if self.selected_comment + 1 < self.comments.len() {
                    self.selected_comment += 1;
                }
            }
        }
    }

    fn activate_selection(&mut self) {
        match self.view {
            View::RepoPicker => {
                self.action = Some(AppAction::PickRepo);
            }
            View::RemoteChooser => {
                self.action = Some(AppAction::PickRemote);
            }
            View::Issues => {
                self.action = Some(AppAction::PickIssue);
            }
            View::IssueDetail => {}
        }
    }
}
