use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};

use anyhow::Result;
use crate::config::{CommentDefault, Config};
use crate::git::RemoteInfo;
use crate::markdown;
use crate::store::{CommentRow, IssueRow, LocalRepoRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    RepoPicker,
    RemoteChooser,
    Issues,
    IssueDetail,
    IssueComments,
    LabelPicker,
    AssigneePicker,
    CommentPresetPicker,
    CommentPresetName,
    CommentEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    PickRepo,
    PickRemote,
    PickIssue,
    OpenInBrowser,
    CheckoutPullRequest,
    CloseIssue,
    ReopenIssue,
    AddIssueComment,
    SubmitIssueComment,
    EditIssueComment,
    DeleteIssueComment,
    SubmitEditedComment,
    EditLabels,
    EditAssignees,
    SubmitLabels,
    SubmitAssignees,
    PickPreset,
    SavePreset,
    SubmitComment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetSelection {
    CloseWithoutComment,
    CustomMessage,
    Preset(usize),
    AddPreset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    IssuesList,
    IssuesPreview,
    IssueBody,
    IssueRecentComments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueFilter {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkItemMode {
    Issues,
    PullRequests,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestFile {
    pub filename: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssigneeFilter {
    All,
    Unassigned,
    User(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingIssueAction {
    Closing,
    Reopening,
    UpdatingLabels,
    UpdatingAssignees,
}

impl PendingIssueAction {
    fn label(self) -> &'static str {
        match self {
            Self::Closing => "closing...",
            Self::Reopening => "reopening...",
            Self::UpdatingLabels => "updating labels...",
            Self::UpdatingAssignees => "updating assignees...",
        }
    }
}

impl AssigneeFilter {
    fn label(&self) -> String {
        match self {
            Self::All => "all".to_string(),
            Self::Unassigned => "unassigned".to_string(),
            Self::User(user) => user.clone(),
        }
    }
}

impl IssueFilter {
    fn next(self) -> Self {
        match self {
            Self::Open => Self::Closed,
            Self::Closed => Self::Open,
        }
    }

    fn from_key(ch: char) -> Option<Self> {
        match ch {
            '1' => Some(Self::Open),
            '2' => Some(Self::Closed),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Closed => "CLOSED",
        }
    }

    fn matches(self, issue: &IssueRow) -> bool {
        if self == Self::Open {
            return issue.state.eq_ignore_ascii_case("open");
        }
        issue.state.eq_ignore_ascii_case("closed")
    }
}

impl WorkItemMode {
    fn toggle(self) -> Self {
        match self {
            Self::Issues => Self::PullRequests,
            Self::PullRequests => Self::Issues,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Issues => "issues",
            Self::PullRequests => "pull requests",
        }
    }

    fn matches(self, issue: &IssueRow) -> bool {
        match self {
            Self::Issues => !issue.is_pr,
            Self::PullRequests => issue.is_pr,
        }
    }
}

pub struct App {
    should_quit: bool,
    config: Config,
    view: View,
    focus: Focus,
    repos: Vec<LocalRepoRow>,
    remotes: Vec<RemoteInfo>,
    issues: Vec<IssueRow>,
    comments: Vec<CommentRow>,
    selected_repo: usize,
    selected_remote: usize,
    selected_issue: usize,
    selected_comment: usize,
    issue_filter: IssueFilter,
    work_item_mode: WorkItemMode,
    assignee_filter: AssigneeFilter,
    repo_query: String,
    repo_search_mode: bool,
    filtered_repo_indices: Vec<usize>,
    issue_query: String,
    issue_search_mode: bool,
    filtered_issue_indices: Vec<usize>,
    issue_detail_scroll: u16,
    issue_detail_max_scroll: u16,
    issues_preview_scroll: u16,
    issues_preview_max_scroll: u16,
    issue_comments_scroll: u16,
    issue_comments_max_scroll: u16,
    issue_recent_comments_scroll: u16,
    issue_recent_comments_max_scroll: u16,
    status: String,
    scanning: bool,
    syncing: bool,
    comment_syncing: bool,
    pull_request_files_syncing: bool,
    comment_sync_requested: bool,
    pull_request_files_sync_requested: bool,
    sync_requested: bool,
    rescan_requested: bool,
    action: Option<AppAction>,
    current_owner: Option<String>,
    current_repo: Option<String>,
    current_repo_path: Option<String>,
    current_issue_id: Option<i64>,
    current_issue_number: Option<i64>,
    pull_request_files_issue_id: Option<i64>,
    pull_request_files: Vec<PullRequestFile>,
    pending_issue_actions: HashMap<i64, PendingIssueAction>,
    pending_g: bool,
    pending_d: bool,
    comment_editor: CommentEditorState,
    editor_cancel_view: View,
    editing_comment_id: Option<i64>,
    label_options: Vec<String>,
    label_selected: HashSet<String>,
    selected_label_option: usize,
    label_query: String,
    assignee_options: Vec<String>,
    assignee_selected: HashSet<String>,
    selected_assignee_option: usize,
    assignee_query: String,
    preset_choice: usize,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            should_quit: false,
            config,
            view: View::RepoPicker,
            focus: Focus::IssuesList,
            repos: Vec::new(),
            remotes: Vec::new(),
            issues: Vec::new(),
            comments: Vec::new(),
            selected_repo: 0,
            selected_remote: 0,
            selected_issue: 0,
            selected_comment: 0,
            issue_filter: IssueFilter::Open,
            work_item_mode: WorkItemMode::Issues,
            assignee_filter: AssigneeFilter::All,
            repo_query: String::new(),
            repo_search_mode: false,
            filtered_repo_indices: Vec::new(),
            issue_query: String::new(),
            issue_search_mode: false,
            filtered_issue_indices: Vec::new(),
            issue_detail_scroll: 0,
            issue_detail_max_scroll: 0,
            issues_preview_scroll: 0,
            issues_preview_max_scroll: 0,
            issue_comments_scroll: 0,
            issue_comments_max_scroll: 0,
            issue_recent_comments_scroll: 0,
            issue_recent_comments_max_scroll: 0,
            status: String::new(),
            scanning: false,
            syncing: false,
            comment_syncing: false,
            pull_request_files_syncing: false,
            comment_sync_requested: false,
            pull_request_files_sync_requested: false,
            sync_requested: false,
            rescan_requested: false,
            action: None,
            current_owner: None,
            current_repo: None,
            current_repo_path: None,
            current_issue_id: None,
            current_issue_number: None,
            pull_request_files_issue_id: None,
            pull_request_files: Vec::new(),
            pending_issue_actions: HashMap::new(),
            pending_g: false,
            pending_d: false,
            comment_editor: CommentEditorState::default(),
            editor_cancel_view: View::Issues,
            editing_comment_id: None,
            label_options: Vec::new(),
            label_selected: HashSet::new(),
            selected_label_option: 0,
            label_query: String::new(),
            assignee_options: Vec::new(),
            assignee_selected: HashSet::new(),
            selected_assignee_option: 0,
            assignee_query: String::new(),
            preset_choice: 0,
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn repos(&self) -> &[LocalRepoRow] {
        &self.repos
    }

    pub fn filtered_repo_rows(&self) -> Vec<&LocalRepoRow> {
        self.filtered_repo_indices
            .iter()
            .filter_map(|index| self.repos.get(*index))
            .collect::<Vec<&LocalRepoRow>>()
    }

    pub fn repo_query(&self) -> &str {
        self.repo_query.as_str()
    }

    pub fn repo_search_mode(&self) -> bool {
        self.repo_search_mode
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

    pub fn issues_for_view(&self) -> Vec<&IssueRow> {
        self.filtered_issue_indices
            .iter()
            .filter_map(|index| self.issues.get(*index))
            .collect::<Vec<&IssueRow>>()
    }

    pub fn selected_issue_row(&self) -> Option<&IssueRow> {
        let issue_index = *self.filtered_issue_indices.get(self.selected_issue)?;
        self.issues.get(issue_index)
    }

    pub fn current_issue_row(&self) -> Option<&IssueRow> {
        let issue_id = self.current_issue_id?;
        self.issues.iter().find(|issue| issue.id == issue_id)
    }

    pub fn issue_filter(&self) -> IssueFilter {
        self.issue_filter
    }

    pub fn work_item_mode(&self) -> WorkItemMode {
        self.work_item_mode
    }

    pub fn current_repo_path(&self) -> Option<&str> {
        self.current_repo_path.as_deref()
    }

    pub fn assignee_filter_label(&self) -> String {
        self.assignee_filter.label()
    }

    pub fn has_assignee_filter(&self) -> bool {
        !matches!(self.assignee_filter, AssigneeFilter::All)
    }

    pub fn current_or_selected_issue(&self) -> Option<&IssueRow> {
        if self.view == View::Issues {
            return self.selected_issue_row();
        }
        self.current_issue_row()
    }

    pub fn set_issue_filter(&mut self, filter: IssueFilter) {
        self.issue_filter = filter;
        self.rebuild_issue_filter();
        self.issues_preview_scroll = 0;
        self.status = format!(
            "Filter: {} | assignee: {}",
            self.issue_filter.label(),
            self.assignee_filter.label()
        );
    }

    pub fn issue_query(&self) -> &str {
        self.issue_query.as_str()
    }

    pub fn issue_search_mode(&self) -> bool {
        self.issue_search_mode
    }

    pub fn issue_counts(&self) -> (usize, usize) {
        let open = self
            .issues
            .iter()
            .filter(|issue| self.work_item_mode.matches(issue))
            .filter(|issue| issue.state.eq_ignore_ascii_case("open"))
            .count();
        let closed = self
            .issues
            .iter()
            .filter(|issue| self.work_item_mode.matches(issue))
            .filter(|issue| issue.state.eq_ignore_ascii_case("closed"))
            .count();
        (open, closed)
    }

    pub fn comment_defaults(&self) -> &[CommentDefault] {
        &self.config.comment_defaults
    }

    pub fn selected_repo(&self) -> usize {
        self.selected_repo
    }

    pub fn selected_repo_target(&self) -> Option<(String, String, String)> {
        let repo_index = *self.filtered_repo_indices.get(self.selected_repo)?;
        let repo = self.repos.get(repo_index)?;
        Some((repo.owner.clone(), repo.repo.clone(), repo.path.clone()))
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

    pub fn selected_comment_row(&self) -> Option<&CommentRow> {
        self.comments.get(self.selected_comment)
    }

    pub fn issue_detail_scroll(&self) -> u16 {
        self.issue_detail_scroll
    }

    pub fn issues_preview_scroll(&self) -> u16 {
        self.issues_preview_scroll
    }

    pub fn issue_comments_scroll(&self) -> u16 {
        self.issue_comments_scroll
    }


    pub fn issue_recent_comments_scroll(&self) -> u16 {
        self.issue_recent_comments_scroll
    }

    pub fn selected_preset(&self) -> usize {
        self.preset_choice
    }

    pub fn label_options(&self) -> &[String] {
        &self.label_options
    }

    pub fn selected_label_option(&self) -> usize {
        self.selected_label_option
    }

    pub fn label_option_selected(&self, label: &str) -> bool {
        self.label_selected.contains(&label.to_ascii_lowercase())
    }

    pub fn label_query(&self) -> &str {
        self.label_query.as_str()
    }

    pub fn filtered_label_indices(&self) -> Vec<usize> {
        let query = self.label_query.trim().to_ascii_lowercase();
        self.label_options
            .iter()
            .enumerate()
            .filter_map(|(index, label)| {
                if query.is_empty() {
                    return Some(index);
                }
                if label.to_ascii_lowercase().contains(query.as_str()) {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<usize>>()
    }

    pub fn assignee_options(&self) -> &[String] {
        &self.assignee_options
    }

    pub fn selected_assignee_option(&self) -> usize {
        self.selected_assignee_option
    }

    pub fn assignee_option_selected(&self, assignee: &str) -> bool {
        self.assignee_selected
            .contains(&assignee.to_ascii_lowercase())
    }

    pub fn assignee_query(&self) -> &str {
        self.assignee_query.as_str()
    }

    pub fn filtered_assignee_indices(&self) -> Vec<usize> {
        let query = self.assignee_query.trim().to_ascii_lowercase();
        self.assignee_options
            .iter()
            .enumerate()
            .filter_map(|(index, assignee)| {
                if query.is_empty() {
                    return Some(index);
                }
                if assignee.to_ascii_lowercase().contains(query.as_str()) {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<usize>>()
    }

    pub fn set_selected_preset(&mut self, index: usize) {
        self.preset_choice = index;
    }

    pub fn preset_items_len(&self) -> usize {
        self.config.comment_defaults.len() + 3
    }

    pub fn preset_selection(&self) -> PresetSelection {
        let defaults = self.config.comment_defaults.len();
        match self.preset_choice {
            0 => PresetSelection::CloseWithoutComment,
            1 => PresetSelection::CustomMessage,
            idx if idx == defaults + 2 => PresetSelection::AddPreset,
            idx => {
                let preset_index = idx.saturating_sub(2);
                PresetSelection::Preset(preset_index)
            }
        }
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

    pub fn pull_request_files_syncing(&self) -> bool {
        self.pull_request_files_syncing
    }

    pub fn pull_request_files(&self) -> &[PullRequestFile] {
        &self.pull_request_files
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if matches!(self.view, View::CommentPresetName | View::CommentEditor) {
            self.handle_editor_key(key);
            return;
        }
        if self.view == View::RepoPicker && self.repo_search_mode {
            if self.handle_repo_search_key(key) {
                return;
            }
        }
        if self.view == View::Issues && self.issue_search_mode {
            if self.handle_issue_search_key(key) {
                return;
            }
        }
        if matches!(self.view, View::LabelPicker | View::AssigneePicker) {
            if self.handle_popup_filter_key(key) {
                return;
            }
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            if self.view == View::RepoPicker {
                self.rescan_requested = true;
                self.scanning = true;
                self.status = "Scanning...".to_string();
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('u') {
                self.page_up();
                return;
            }
            if key.code == KeyCode::Char('d') {
                self.page_down();
                return;
            }
            if self.handle_focus_key(key.code) {
                return;
            }
        }

        if key.code != KeyCode::Char('g') {
            self.pending_g = false;
        }
        if key.code != KeyCode::Char('d') {
            self.pending_d = false;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.repo_query.clear();
                self.repo_search_mode = false;
                self.rebuild_repo_picker_filter();
                self.set_view(View::RepoPicker);
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && self.view == View::RepoPicker => {
                self.repo_search_mode = true;
                self.status = "Search repos".to_string();
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.issue_search_mode = true;
                self.status = "Search issues".to_string();
            }
            KeyCode::Char('f') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.set_issue_filter(self.issue_filter.next());
            }
            KeyCode::Char('p') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.work_item_mode = self.work_item_mode.toggle();
                self.assignee_filter = AssigneeFilter::All;
                self.rebuild_issue_filter();
                self.issues_preview_scroll = 0;
                self.status = format!("Showing {}", self.work_item_mode.label());
            }
            KeyCode::Char('a') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.cycle_assignee_filter(true);
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty()
                    && self.view == View::Issues
                    && IssueFilter::from_key(ch).is_some() =>
            {
                self.set_issue_filter(IssueFilter::from_key(ch).unwrap_or(IssueFilter::Open));
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.request_sync();
                self.status = "Syncing...".to_string();
            }
            KeyCode::Char('r')
                if key.modifiers.is_empty()
                    && matches!(self.view, View::IssueDetail | View::IssueComments) =>
            {
                self.request_comment_sync();
                self.request_sync();
                if self.current_view_issue_is_pull_request() {
                    self.request_pull_request_files_sync();
                }
                self.status = "Syncing issue and comments...".to_string();
            }
            KeyCode::Char('g') if key.modifiers.is_empty() => {
                if self.pending_g {
                    self.jump_top();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.is_empty()
                    && matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                let has_issue = if self.view == View::Issues {
                    !self.filtered_issue_indices.is_empty()
                } else {
                    self.current_issue_id.is_some() && self.current_issue_number.is_some()
                };
                if !has_issue {
                    self.pending_d = false;
                    self.status = "No issue selected".to_string();
                    return;
                }
                if self.current_view_issue_is_closed() {
                    self.pending_d = false;
                    self.status = "Issue already closed".to_string();
                    return;
                }
                if self.pending_d {
                    self.action = Some(AppAction::CloseIssue);
                    self.pending_d = false;
                } else {
                    self.pending_d = true;
                }
            }
            KeyCode::Char('G') => self.jump_bottom(),
            KeyCode::Char('c') if self.view == View::IssueDetail => {
                self.reset_issue_comments_scroll();
                self.set_view(View::IssueComments);
            }
            KeyCode::Char('m')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::AddIssueComment);
            }
            KeyCode::Char('e') if self.view == View::IssueComments => {
                self.action = Some(AppAction::EditIssueComment);
            }
            KeyCode::Char('x') if self.view == View::IssueComments => {
                self.action = Some(AppAction::DeleteIssueComment);
            }
            KeyCode::Char('l')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::EditLabels);
            }
            KeyCode::Char('A')
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::EditAssignees);
            }
            KeyCode::Char('u')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::ReopenIssue);
            }
            KeyCode::Char(' ') if self.view == View::LabelPicker => {
                self.toggle_selected_label();
            }
            KeyCode::Char(' ') if self.view == View::AssigneePicker => {
                self.toggle_selected_assignee();
            }
            KeyCode::Enter if self.view == View::LabelPicker => {
                self.toggle_selected_label();
                self.action = Some(AppAction::SubmitLabels);
            }
            KeyCode::Enter if self.view == View::AssigneePicker => {
                self.toggle_selected_assignee();
                self.action = Some(AppAction::SubmitAssignees);
            }
            KeyCode::Char('b') if self.view == View::IssueDetail => {
                self.set_view(View::Issues);
            }
            KeyCode::Char('b') if self.view == View::IssueComments => {
                self.set_view(View::IssueDetail);
            }
            KeyCode::Esc if self.view == View::IssueDetail => {
                self.set_view(View::Issues);
            }
            KeyCode::Esc if self.view == View::IssueComments => {
                self.set_view(View::IssueDetail);
            }
            KeyCode::Esc if self.view == View::CommentPresetPicker => {
                self.set_view(View::Issues);
            }
            KeyCode::Esc if matches!(self.view, View::LabelPicker | View::AssigneePicker) => {
                self.set_view(self.editor_cancel_view);
            }
            KeyCode::Char('k') | KeyCode::Up => self.move_selection_up(),
            KeyCode::Char('j') | KeyCode::Down => self.move_selection_down(),
            KeyCode::Enter => self.activate_selection(),
            KeyCode::Char('o')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::OpenInBrowser);
            }
            KeyCode::Char('v')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::CheckoutPullRequest);
            }
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
        match self.view {
            View::Issues => self.focus = Focus::IssuesList,
            View::IssueDetail => self.focus = Focus::IssueBody,
            _ => {
                self.issue_search_mode = false;
                if self.view != View::RepoPicker {
                    self.repo_search_mode = false;
                }
            }
        }
    }

    pub fn set_repos(&mut self, repos: Vec<LocalRepoRow>) {
        self.repos = repos;
        self.rebuild_repo_picker_filter();
        if self.selected_repo >= self.filtered_repo_indices.len() {
            self.selected_repo = self.filtered_repo_indices.len().saturating_sub(1);
        }
    }

    pub fn set_remotes(&mut self, remotes: Vec<RemoteInfo>) {
        self.remotes = remotes;
        self.selected_remote = 0;
    }

    pub fn set_issues(&mut self, issues: Vec<IssueRow>) {
        let selected_issue_number = self.selected_issue_row().map(|issue| issue.number);
        let current_issue_number = self.current_issue_number;
        self.issues = issues;
        self.rebuild_issue_filter();
        self.selected_issue = selected_issue_number
            .and_then(|number| {
                self.filtered_issue_indices
                    .iter()
                    .position(|index| self.issues.get(*index).is_some_and(|issue| issue.number == number))
            })
            .unwrap_or(0);
        if let Some(number) = current_issue_number {
            if let Some(issue) = self.issues.iter().find(|issue| issue.number == number) {
                self.current_issue_id = Some(issue.id);
            }
        }
        self.issues_preview_scroll = 0;
        self.issues_preview_max_scroll = 0;
    }

    pub fn set_comments(&mut self, comments: Vec<CommentRow>) {
        let selected_comment_id = self.selected_comment_row().map(|comment| comment.id);
        self.comments = comments;
        if self.comments.is_empty() {
            self.selected_comment = 0;
            self.issue_comments_scroll = 0;
            self.issue_recent_comments_scroll = 0;
            self.issue_comments_max_scroll = 0;
            self.issue_recent_comments_max_scroll = 0;
            return;
        }
        self.selected_comment = selected_comment_id
            .and_then(|comment_id| self.comments.iter().position(|comment| comment.id == comment_id))
            .unwrap_or(0);
        self.issue_comments_scroll = 0;
        self.issue_recent_comments_scroll = 0;
        self.issue_comments_max_scroll = 0;
        self.issue_recent_comments_max_scroll = 0;
    }

    pub fn set_pull_request_files(&mut self, issue_id: i64, files: Vec<PullRequestFile>) {
        self.pull_request_files_issue_id = Some(issue_id);
        self.pull_request_files = files;
        self.issue_recent_comments_scroll = 0;
        self.issue_recent_comments_max_scroll = 0;
    }

    pub fn reset_issue_detail_scroll(&mut self) {
        self.issue_detail_scroll = 0;
    }

    pub fn set_issue_detail_max_scroll(&mut self, max_scroll: u16) {
        self.issue_detail_max_scroll = max_scroll;
        if self.issue_detail_scroll > max_scroll {
            self.issue_detail_scroll = max_scroll;
        }
    }

    pub fn set_issues_preview_max_scroll(&mut self, max_scroll: u16) {
        self.issues_preview_max_scroll = max_scroll;
        if self.issues_preview_scroll > max_scroll {
            self.issues_preview_scroll = max_scroll;
        }
    }

    pub fn reset_issue_comments_scroll(&mut self) {
        self.issue_comments_scroll = 0;
    }

    pub fn set_issue_comments_max_scroll(&mut self, max_scroll: u16) {
        self.issue_comments_max_scroll = max_scroll;
        if self.issue_comments_scroll > max_scroll {
            self.issue_comments_scroll = max_scroll;
        }
    }

    pub fn set_issue_recent_comments_max_scroll(&mut self, max_scroll: u16) {
        self.issue_recent_comments_max_scroll = max_scroll;
        if self.issue_recent_comments_scroll > max_scroll {
            self.issue_recent_comments_scroll = max_scroll;
        }
    }

    pub fn add_comment_default(&mut self, preset: CommentDefault) {
        self.config.comment_defaults.push(preset);
        self.preset_choice = 0;
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.save()
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

    pub fn set_pull_request_files_syncing(&mut self, syncing: bool) {
        self.pull_request_files_syncing = syncing;
    }

    pub fn request_comment_sync(&mut self) {
        self.comment_sync_requested = true;
    }

    pub fn take_comment_sync_request(&mut self) -> bool {
        let requested = self.comment_sync_requested;
        self.comment_sync_requested = false;
        requested
    }

    pub fn request_pull_request_files_sync(&mut self) {
        self.pull_request_files_sync_requested = true;
    }

    pub fn take_pull_request_files_sync_request(&mut self) -> bool {
        let requested = self.pull_request_files_sync_requested;
        self.pull_request_files_sync_requested = false;
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

    pub fn set_current_repo_with_path(&mut self, owner: &str, repo: &str, path: Option<&str>) {
        self.current_owner = Some(owner.to_string());
        self.current_repo = Some(repo.to_string());
        self.current_repo_path = path.map(ToString::to_string);
        self.current_issue_id = None;
        self.current_issue_number = None;
        self.pull_request_files_issue_id = None;
        self.pull_request_files.clear();
        self.repo_search_mode = false;
        self.assignee_filter = AssigneeFilter::All;
        self.work_item_mode = WorkItemMode::Issues;
        self.issue_query.clear();
        self.issue_search_mode = false;
    }

    pub fn set_current_issue(&mut self, issue_id: i64, issue_number: i64) {
        self.current_issue_id = Some(issue_id);
        self.current_issue_number = Some(issue_number);
        if self.pull_request_files_issue_id != Some(issue_id) {
            self.pull_request_files_issue_id = None;
            self.pull_request_files.clear();
        }
    }

    pub fn update_issue_state_by_number(&mut self, issue_number: i64, state: &str) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.state = state.to_string();
            }
        }
        self.rebuild_issue_filter();
        if self.selected_issue >= self.filtered_issue_indices.len() {
            self.selected_issue = self.filtered_issue_indices.len().saturating_sub(1);
        }
    }

    pub fn update_issue_labels_by_number(&mut self, issue_number: i64, labels: &str) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.labels = labels.to_string();
            }
        }
        self.rebuild_issue_filter();
    }

    pub fn update_issue_assignees_by_number(&mut self, issue_number: i64, assignees: &str) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.assignees = assignees.to_string();
            }
        }
        self.rebuild_issue_filter();
    }

    pub fn update_issue_comments_count_by_number(&mut self, issue_number: i64, count: i64) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.comments_count = count;
            }
        }
    }

    pub fn update_comment_body_by_id(&mut self, comment_id: i64, body: &str) {
        for comment in &mut self.comments {
            if comment.id == comment_id {
                comment.body = body.to_string();
                return;
            }
        }
    }

    pub fn remove_comment_by_id(&mut self, comment_id: i64) {
        let removed_index = self
            .comments
            .iter()
            .position(|comment| comment.id == comment_id);
        let removed_index = match removed_index {
            Some(index) => index,
            None => return,
        };

        self.comments.remove(removed_index);
        if self.comments.is_empty() {
            self.selected_comment = 0;
            self.issue_comments_scroll = 0;
            return;
        }

        if self.selected_comment >= self.comments.len() {
            self.selected_comment = self.comments.len() - 1;
            return;
        }
        if removed_index <= self.selected_comment && self.selected_comment > 0 {
            self.selected_comment -= 1;
        }
    }

    pub fn editor(&self) -> &CommentEditorState {
        &self.comment_editor
    }

    pub fn editor_mode(&self) -> EditorMode {
        self.comment_editor.mode()
    }

    pub fn editor_mut(&mut self) -> &mut CommentEditorState {
        &mut self.comment_editor
    }

    pub fn open_close_comment_editor(&mut self) {
        self.editing_comment_id = None;
        self.comment_editor.reset_for_close();
        self.editor_cancel_view = View::CommentPresetPicker;
        self.set_view(View::CommentEditor);
    }

    pub fn open_issue_comment_editor(&mut self, return_view: View) {
        self.editing_comment_id = None;
        self.comment_editor.reset_for_comment();
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_comment_edit_editor(
        &mut self,
        return_view: View,
        comment_id: i64,
        body: &str,
    ) {
        self.editing_comment_id = Some(comment_id);
        self.comment_editor.reset_for_comment_edit(body);
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_label_picker(
        &mut self,
        return_view: View,
        mut options: Vec<String>,
        current_labels: &str,
    ) {
        self.editor_cancel_view = return_view;
        options.sort_by_key(|value| value.to_ascii_lowercase());
        options.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        self.label_options = options;
        self.selected_label_option = 0;
        self.label_query.clear();
        self.label_selected = Self::csv_set(current_labels);
        self.set_view(View::LabelPicker);
    }

    pub fn open_assignee_picker(
        &mut self,
        return_view: View,
        mut options: Vec<String>,
        current_assignees: &str,
    ) {
        self.editor_cancel_view = return_view;
        options.sort_by_key(|value| value.to_ascii_lowercase());
        options.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        self.assignee_options = options;
        self.selected_assignee_option = 0;
        self.assignee_query.clear();
        self.assignee_selected = Self::csv_set(current_assignees);
        self.set_view(View::AssigneePicker);
    }

    pub fn merge_label_options(&mut self, labels: Vec<String>) {
        let mut merged = self.label_options.clone();
        for label in labels {
            if label.trim().is_empty() {
                continue;
            }
            if merged
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(label.as_str()))
            {
                continue;
            }
            merged.push(label);
        }
        merged.sort_by_key(|value| value.to_ascii_lowercase());
        self.label_options = merged;
        if let Some(index) = self.filtered_label_indices().first() {
            self.selected_label_option = *index;
        }
    }

    pub fn selected_labels_csv(&self) -> String {
        let mut values = self
            .label_options
            .iter()
            .filter(|label| self.label_option_selected(label.as_str()))
            .cloned()
            .collect::<Vec<String>>();
        values.sort_by_key(|value| value.to_ascii_lowercase());
        values.join(",")
    }

    pub fn selected_labels(&self) -> Vec<String> {
        self.selected_labels_csv()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<String>>()
    }

    pub fn selected_assignees_csv(&self) -> String {
        let mut values = self
            .assignee_options
            .iter()
            .filter(|assignee| self.assignee_option_selected(assignee.as_str()))
            .cloned()
            .collect::<Vec<String>>();
        values.sort_by_key(|value| value.to_ascii_lowercase());
        values.join(",")
    }

    pub fn selected_assignees(&self) -> Vec<String> {
        self.selected_assignees_csv()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<String>>()
    }

    pub fn editor_cancel_view(&self) -> View {
        self.editor_cancel_view
    }

    pub fn take_editing_comment_id(&mut self) -> Option<i64> {
        self.editing_comment_id.take()
    }

    pub fn current_issue_id(&self) -> Option<i64> {
        self.current_issue_id
    }

    pub fn current_issue_number(&self) -> Option<i64> {
        self.current_issue_number
    }

    pub fn set_pending_issue_action(&mut self, issue_number: i64, action: PendingIssueAction) {
        self.pending_issue_actions.insert(issue_number, action);
    }

    pub fn clear_pending_issue_action(&mut self, issue_number: i64) {
        self.pending_issue_actions.remove(&issue_number);
    }

    pub fn pending_issue_badge(&self, issue_number: i64) -> Option<&'static str> {
        self.pending_issue_actions
            .get(&issue_number)
            .copied()
            .map(PendingIssueAction::label)
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
                if self.focus == Focus::IssuesPreview {
                    self.issues_preview_scroll = self.issues_preview_scroll.saturating_sub(1);
                    return;
                }
                if self.selected_issue > 0 {
                    self.selected_issue -= 1;
                    self.issues_preview_scroll = 0;
                }
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    self.issue_recent_comments_scroll =
                        self.issue_recent_comments_scroll.saturating_sub(1);
                    return;
                }
                self.issue_detail_scroll = self.issue_detail_scroll.saturating_sub(1);
            }
            View::IssueComments => {
                self.jump_prev_comment();
            }
            View::CommentPresetPicker => {
                if self.preset_choice > 0 {
                    self.preset_choice -= 1;
                }
            }
            View::LabelPicker => {
                let filtered = self.filtered_label_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.selected_label_option)
                    .unwrap_or(0);
                let next = current.saturating_sub(1);
                self.selected_label_option = filtered[next];
            }
            View::AssigneePicker => {
                let filtered = self.filtered_assignee_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.selected_assignee_option)
                    .unwrap_or(0);
                let next = current.saturating_sub(1);
                self.selected_assignee_option = filtered[next];
            }
            View::CommentPresetName
            | View::CommentEditor
                => {}
        }
    }

    fn move_selection_down(&mut self) {
        match self.view {
            View::RepoPicker => {
                if self.selected_repo + 1 < self.filtered_repo_indices.len() {
                    self.selected_repo += 1;
                }
            }
            View::RemoteChooser => {
                if self.selected_remote + 1 < self.remotes.len() {
                    self.selected_remote += 1;
                }
            }
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    let max = self.issues_preview_max_scroll;
                    self.issues_preview_scroll =
                        self.issues_preview_scroll.saturating_add(1).min(max);
                    return;
                }
                if self.selected_issue + 1 < self.filtered_issue_indices.len() {
                    self.selected_issue += 1;
                    self.issues_preview_scroll = 0;
                }
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    let max = self.issue_recent_comments_max_scroll;
                    self.issue_recent_comments_scroll =
                        self.issue_recent_comments_scroll.saturating_add(1).min(max);
                    return;
                }
                let max = self.issue_detail_max_scroll;
                self.issue_detail_scroll = self.issue_detail_scroll.saturating_add(1).min(max);
            }
            View::IssueComments => {
                self.jump_next_comment();
            }
            View::CommentPresetPicker => {
                let max = self.preset_items_len();
                if self.preset_choice + 1 < max {
                    self.preset_choice += 1;
                }
            }
            View::LabelPicker => {
                let filtered = self.filtered_label_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.selected_label_option)
                    .unwrap_or(0);
                let next = (current + 1).min(filtered.len() - 1);
                self.selected_label_option = filtered[next];
            }
            View::AssigneePicker => {
                let filtered = self.filtered_assignee_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.selected_assignee_option)
                    .unwrap_or(0);
                let next = (current + 1).min(filtered.len() - 1);
                self.selected_assignee_option = filtered[next];
            }
            View::CommentPresetName
            | View::CommentEditor
                => {}
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
            View::IssueDetail => {
                self.reset_issue_comments_scroll();
                self.set_view(View::IssueComments);
            }
            View::IssueComments => {}
            View::CommentPresetPicker => {
                self.action = Some(AppAction::PickPreset);
            }
            View::CommentPresetName
            | View::CommentEditor
            | View::LabelPicker
            | View::AssigneePicker => {}
        }
    }

    fn jump_top(&mut self) {
        match self.view {
            View::RepoPicker => self.selected_repo = 0,
            View::RemoteChooser => self.selected_remote = 0,
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    self.issues_preview_scroll = 0;
                    return;
                }
                self.selected_issue = 0;
                self.issues_preview_scroll = 0;
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    self.issue_recent_comments_scroll = 0;
                    return;
                }
                self.issue_detail_scroll = 0;
            }
            View::IssueComments => {
                self.selected_comment = 0;
                self.issue_comments_scroll = 0;
            }
            View::CommentPresetPicker => self.preset_choice = 0,
            View::LabelPicker => {
                if let Some(index) = self.filtered_label_indices().first() {
                    self.selected_label_option = *index;
                }
            }
            View::AssigneePicker => {
                if let Some(index) = self.filtered_assignee_indices().first() {
                    self.selected_assignee_option = *index;
                }
            }
            View::CommentPresetName
            | View::CommentEditor
                => {}
        }
    }

    fn jump_bottom(&mut self) {
        match self.view {
            View::RepoPicker => {
                if !self.filtered_repo_indices.is_empty() {
                    self.selected_repo = self.filtered_repo_indices.len() - 1;
                }
            }
            View::RemoteChooser => {
                if !self.remotes.is_empty() {
                    self.selected_remote = self.remotes.len() - 1;
                }
            }
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    self.issues_preview_scroll = self.issues_preview_max_scroll;
                    return;
                }
                if !self.filtered_issue_indices.is_empty() {
                    self.selected_issue = self.filtered_issue_indices.len() - 1;
                    self.issues_preview_scroll = 0;
                }
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    self.issue_recent_comments_scroll = self.issue_recent_comments_max_scroll;
                    return;
                }
                self.issue_detail_scroll = self.issue_detail_max_scroll;
            }
            View::IssueComments => {
                if !self.comments.is_empty() {
                    self.selected_comment = self.comments.len() - 1;
                }
                self.issue_comments_scroll = self.issue_comments_max_scroll;
            }
            View::CommentPresetPicker => {
                let max = self.preset_items_len();
                if max > 0 {
                    self.preset_choice = max - 1;
                }
            }
            View::LabelPicker => {
                let filtered = self.filtered_label_indices();
                if !filtered.is_empty() {
                    self.selected_label_option = *filtered.last().unwrap_or(&0);
                }
            }
            View::AssigneePicker => {
                let filtered = self.filtered_assignee_indices();
                if !filtered.is_empty() {
                    self.selected_assignee_option = *filtered.last().unwrap_or(&0);
                }
            }
            View::CommentPresetName
            | View::CommentEditor
                => {}
        }
    }

    fn page_up(&mut self) {
        for _ in 0..10 {
            self.move_selection_up();
        }
    }

    fn page_down(&mut self) {
        for _ in 0..10 {
            self.move_selection_down();
        }
    }

    fn jump_next_comment(&mut self) {
        let offsets = self.comment_offsets();
        if offsets.is_empty() || self.selected_comment + 1 >= offsets.len() {
            return;
        }
        self.selected_comment += 1;
        self.issue_comments_scroll = offsets[self.selected_comment].min(self.issue_comments_max_scroll);
        self.status = format!("Comment {}/{}", self.selected_comment + 1, offsets.len());
    }

    fn jump_prev_comment(&mut self) {
        let offsets = self.comment_offsets();
        if offsets.is_empty() || self.selected_comment == 0 {
            return;
        }
        self.selected_comment -= 1;
        self.issue_comments_scroll = offsets[self.selected_comment];
        self.status = format!("Comment {}/{}", self.selected_comment + 1, offsets.len());
    }

    fn comment_offsets(&self) -> Vec<u16> {
        let mut offsets = Vec::new();
        let mut line = 0usize;
        for comment in &self.comments {
            offsets.push(line.min(u16::MAX as usize) as u16);
            line += 1;
            line += markdown::render(comment.body.as_str()).lines.len().max(1);
            line += 1;
        }
        offsets
    }

    fn current_view_issue_is_closed(&self) -> bool {
        if self.view == View::Issues {
            return self
                .selected_issue_row()
                .is_some_and(|issue| issue.state.eq_ignore_ascii_case("closed"));
        }

        self.current_issue_row()
            .is_some_and(|issue| issue.state.eq_ignore_ascii_case("closed"))
    }

    fn current_view_issue_is_pull_request(&self) -> bool {
        if self.view == View::Issues {
            return self.selected_issue_row().is_some_and(|issue| issue.is_pr);
        }
        self.current_issue_row().is_some_and(|issue| issue.is_pr)
    }

    fn rebuild_issue_filter(&mut self) {
        let query = self.issue_query.trim().to_ascii_lowercase();
        self.filtered_issue_indices = self
            .issues
            .iter()
            .enumerate()
            .filter_map(|(index, issue)| {
                if self.work_item_mode.matches(issue)
                    && self.issue_filter.matches(issue)
                    && self.assignee_filter_matches(issue)
                    && Self::issue_matches_query(issue, query.as_str())
                {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<usize>>();

        self.filtered_issue_indices
            .sort_by(|left_index, right_index| {
                let left = self.issues.get(*left_index);
                let right = self.issues.get(*right_index);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        if self.issue_filter == IssueFilter::Closed {
                            let updated_cmp = right.updated_at.cmp(&left.updated_at);
                            if updated_cmp != std::cmp::Ordering::Equal {
                                return updated_cmp;
                            }
                        }
                        right.number.cmp(&left.number)
                    }
                    _ => std::cmp::Ordering::Equal,
                }
            });

        if self.selected_issue >= self.filtered_issue_indices.len() {
            self.selected_issue = self.filtered_issue_indices.len().saturating_sub(1);
        }
    }

    fn issue_matches_query(issue: &IssueRow, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }

        let title = issue.title.to_ascii_lowercase();
        let body = issue.body.to_ascii_lowercase();
        let labels = issue.labels.to_ascii_lowercase();
        let assignees = issue.assignees.to_ascii_lowercase();
        let number = issue.number.to_string();
        let state = issue.state.to_ascii_lowercase();

        query.split_whitespace().all(|token| {
            if let Some(value) = token.strip_prefix("is:") {
                if value == "pr" || value == "pull" || value == "pull-request" {
                    return issue.is_pr;
                }
                if value == "issue" {
                    return !issue.is_pr;
                }
                return value == state;
            }
            if let Some(value) = token.strip_prefix("label:") {
                return labels.contains(value);
            }
            if let Some(value) = token.strip_prefix("assignee:") {
                let value = value.strip_prefix('@').unwrap_or(value);
                if value == "none" || value == "unassigned" {
                    return issue.assignees.trim().is_empty();
                }
                return Self::issue_has_assignee(issue.assignees.as_str(), value);
            }
            if let Some(value) = token.strip_prefix('#') {
                return value.parse::<i64>().ok().is_some_and(|parsed| issue.number == parsed);
            }
            title.contains(token)
                || body.contains(token)
                || labels.contains(token)
                || assignees.contains(token)
                || number.contains(token)
        })
    }

    fn cycle_assignee_filter(&mut self, forward: bool) {
        let options = self.assignee_filter_options();
        if options.is_empty() {
            self.assignee_filter = AssigneeFilter::All;
            self.rebuild_issue_filter();
            return;
        }

        let current = options
            .iter()
            .position(|option| *option == self.assignee_filter)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % options.len()
        } else if current == 0 {
            options.len() - 1
        } else {
            current - 1
        };

        self.assignee_filter = options[next].clone();
        self.rebuild_issue_filter();
        self.issues_preview_scroll = 0;
        self.status = format!(
            "Assignee: {} ({} items)",
            self.assignee_filter.label(),
            self.filtered_issue_indices.len()
        );
    }

    fn assignee_filter_options(&self) -> Vec<AssigneeFilter> {
        let mut users = self
            .issues
            .iter()
            .filter(|issue| self.work_item_mode.matches(issue))
            .flat_map(|issue| issue.assignees.split(','))
            .map(str::trim)
            .filter(|assignee| !assignee.is_empty())
            .map(|assignee| assignee.to_string())
            .collect::<Vec<String>>();
        users.sort_by_key(|user| user.to_ascii_lowercase());
        users.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

        let has_unassigned = self
            .issues
            .iter()
            .filter(|issue| self.work_item_mode.matches(issue))
            .any(|issue| issue.assignees.trim().is_empty());

        let mut options = vec![AssigneeFilter::All];
        if has_unassigned {
            options.push(AssigneeFilter::Unassigned);
        }
        for user in users {
            options.push(AssigneeFilter::User(user));
        }
        options
    }

    fn assignee_filter_matches(&self, issue: &IssueRow) -> bool {
        match &self.assignee_filter {
            AssigneeFilter::All => true,
            AssigneeFilter::Unassigned => issue.assignees.trim().is_empty(),
            AssigneeFilter::User(user) => Self::issue_has_assignee(issue.assignees.as_str(), user),
        }
    }

    fn issue_has_assignee(issue_assignees: &str, user: &str) -> bool {
        issue_assignees
            .split(',')
            .map(str::trim)
            .any(|assignee| assignee.eq_ignore_ascii_case(user))
    }

    fn csv_set(input: &str) -> HashSet<String> {
        input
            .split(',')
            .map(str::trim)
            .map(|value| value.to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<String>>()
    }

    fn toggle_selected_label(&mut self) {
        if !self
            .filtered_label_indices()
            .contains(&self.selected_label_option)
        {
            return;
        }
        let label = match self.label_options.get(self.selected_label_option) {
            Some(label) => label.to_ascii_lowercase(),
            None => return,
        };
        if self.label_selected.contains(label.as_str()) {
            self.label_selected.remove(label.as_str());
            return;
        }
        self.label_selected.insert(label);
    }

    fn toggle_selected_assignee(&mut self) {
        if !self
            .filtered_assignee_indices()
            .contains(&self.selected_assignee_option)
        {
            return;
        }
        let assignee = match self.assignee_options.get(self.selected_assignee_option) {
            Some(assignee) => assignee.to_ascii_lowercase(),
            None => return,
        };
        if self.assignee_selected.contains(assignee.as_str()) {
            self.assignee_selected.remove(assignee.as_str());
            return;
        }
        self.assignee_selected.insert(assignee);
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        match self.view {
            View::CommentPresetName => match key.code {
                KeyCode::Esc => {
                    self.set_view(View::CommentPresetPicker);
                }
                KeyCode::Enter => {
                    if self.comment_editor.name().is_empty() {
                        self.status = "Preset name required".to_string();
                        return;
                    }
                    self.editor_cancel_view = View::CommentPresetPicker;
                    self.set_view(View::CommentEditor);
                }
                KeyCode::Backspace => self.comment_editor.backspace_name(),
                KeyCode::Char(ch) => self.comment_editor.append_name(ch),
                _ => {}
            },
            View::CommentEditor => {
                match key.code {
                    KeyCode::Esc => {
                        self.editing_comment_id = None;
                        self.set_view(self.editor_cancel_view);
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        if self.comment_editor.mode().allows_multiline() {
                            self.comment_editor.newline()
                        }
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        if self.comment_editor.mode().allows_multiline() {
                            self.comment_editor.newline()
                        }
                    }
                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if self.comment_editor.mode().allows_multiline() {
                            self.comment_editor.newline()
                        }
                    }
                    KeyCode::Enter => match self.comment_editor.mode() {
                        EditorMode::CloseIssue => {
                            self.action = Some(AppAction::SubmitComment);
                        }
                        EditorMode::AddComment => {
                            self.action = Some(AppAction::SubmitIssueComment);
                        }
                        EditorMode::EditComment => {
                            self.action = Some(AppAction::SubmitEditedComment);
                        }
                        EditorMode::AddPreset => {
                            self.action = Some(AppAction::SavePreset);
                        }
                    },
                    KeyCode::Backspace => self.comment_editor.backspace_text(),
                    KeyCode::Char(ch) => self.comment_editor.append_text(ch),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn handle_issue_search_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            self.issue_query.clear();
            self.rebuild_issue_filter();
            self.issues_preview_scroll = 0;
            self.update_search_status();
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                self.issue_search_mode = false;
                self.issue_query.clear();
                self.rebuild_issue_filter();
                self.issues_preview_scroll = 0;
                self.status = "Search cleared".to_string();
            }
            KeyCode::Enter => {
                self.issue_search_mode = false;
                self.update_search_status();
            }
            KeyCode::Backspace => {
                self.issue_query.pop();
                self.rebuild_issue_filter();
                self.issues_preview_scroll = 0;
                self.update_search_status();
            }
            KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.issue_query.push(ch);
                self.rebuild_issue_filter();
                self.issues_preview_scroll = 0;
                self.update_search_status();
            }
            _ => {}
        }
        true
    }

    fn handle_repo_search_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            self.repo_query.clear();
            self.rebuild_repo_picker_filter();
            self.selected_repo = 0;
            self.status = "Repo search cleared".to_string();
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                self.repo_search_mode = false;
                self.repo_query.clear();
                self.rebuild_repo_picker_filter();
                self.selected_repo = 0;
                self.status = String::new();
            }
            KeyCode::Enter => {
                self.repo_search_mode = false;
                self.status = format!("{} repos", self.filtered_repo_indices.len());
            }
            KeyCode::Backspace => {
                self.repo_query.pop();
                self.rebuild_repo_picker_filter();
                self.selected_repo = 0;
            }
            KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                self.repo_query.push(ch);
                self.rebuild_repo_picker_filter();
                self.selected_repo = 0;
            }
            _ => {}
        }
        true
    }

    fn handle_popup_filter_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            if self.view == View::LabelPicker {
                self.label_query.clear();
                if let Some(index) = self.filtered_label_indices().first() {
                    self.selected_label_option = *index;
                }
                return true;
            }
            if self.view == View::AssigneePicker {
                self.assignee_query.clear();
                if let Some(index) = self.filtered_assignee_indices().first() {
                    self.selected_assignee_option = *index;
                }
                return true;
            }
        }

        match key.code {
            KeyCode::Backspace => {
                if self.view == View::LabelPicker {
                    self.label_query.pop();
                    if let Some(index) = self.filtered_label_indices().first() {
                        self.selected_label_option = *index;
                    }
                    return true;
                }
                if self.view == View::AssigneePicker {
                    self.assignee_query.pop();
                    if let Some(index) = self.filtered_assignee_indices().first() {
                        self.selected_assignee_option = *index;
                    }
                    return true;
                }
            }
            KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                if self.view == View::LabelPicker {
                    if self.label_query.is_empty() && matches!(ch, 'j' | 'k' | 'g' | 'G') {
                        return false;
                    }
                    self.label_query.push(ch);
                    if let Some(index) = self.filtered_label_indices().first() {
                        self.selected_label_option = *index;
                    }
                    return true;
                }
                if self.view == View::AssigneePicker {
                    if self.assignee_query.is_empty() && matches!(ch, 'j' | 'k' | 'g' | 'G') {
                        return false;
                    }
                    self.assignee_query.push(ch);
                    if let Some(index) = self.filtered_assignee_indices().first() {
                        self.selected_assignee_option = *index;
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn update_search_status(&mut self) {
        if self.issue_query.trim().is_empty() {
            self.status = format!(
                "Filter: {} | assignee: {}",
                self.issue_filter.label(),
                self.assignee_filter.label()
            );
            return;
        }
        self.status = format!(
            "Search: {} | assignee: {} ({} results)",
            self.issue_query,
            self.assignee_filter.label(),
            self.filtered_issue_indices.len()
        );
    }

    fn handle_focus_key(&mut self, code: KeyCode) -> bool {
        match self.view {
            View::Issues => match code {
                KeyCode::Char('h') | KeyCode::Char('k') => {
                    self.focus = Focus::IssuesList;
                    true
                }
                KeyCode::Char('l') | KeyCode::Char('j') => {
                    self.focus = Focus::IssuesPreview;
                    true
                }
                _ => false,
            },
            View::IssueDetail => match code {
                KeyCode::Char('h') | KeyCode::Char('k') => {
                    self.focus = Focus::IssueBody;
                    true
                }
                KeyCode::Char('l') | KeyCode::Char('j') => {
                    self.focus = Focus::IssueRecentComments;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn rebuild_repo_picker_filter(&mut self) {
        let query = self.repo_query.trim().to_ascii_lowercase();
        self.filtered_repo_indices = self
            .repos
            .iter()
            .enumerate()
            .filter_map(|(index, repo)| {
                if query.is_empty() {
                    return Some(index);
                }
                let haystack = format!(
                    "{} {} {} {} {}",
                    repo.owner, repo.repo, repo.path, repo.remote_name, repo.url
                )
                .to_ascii_lowercase();
                if haystack.contains(query.as_str()) {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<usize>>();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    CloseIssue,
    AddComment,
    EditComment,
    AddPreset,
}

impl EditorMode {
    fn allows_multiline(self) -> bool {
        matches!(
            self,
            Self::CloseIssue | Self::AddComment | Self::EditComment | Self::AddPreset
        )
    }
}

#[derive(Debug, Clone)]
pub struct CommentEditorState {
    mode: EditorMode,
    name: String,
    text: String,
}

impl Default for CommentEditorState {
    fn default() -> Self {
        Self {
            mode: EditorMode::CloseIssue,
            name: String::new(),
            text: String::new(),
        }
    }
}

impl CommentEditorState {
    pub fn mode(&self) -> EditorMode {
        self.mode
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn reset_for_close(&mut self) {
        self.mode = EditorMode::CloseIssue;
        self.text.clear();
    }

    pub fn reset_for_comment(&mut self) {
        self.mode = EditorMode::AddComment;
        self.text.clear();
    }

    pub fn reset_for_comment_edit(&mut self, body: &str) {
        self.mode = EditorMode::EditComment;
        self.text = body.to_string();
    }

    pub fn reset_for_preset_name(&mut self) {
        self.mode = EditorMode::AddPreset;
        self.name.clear();
        self.text.clear();
    }

    pub fn append_name(&mut self, ch: char) {
        self.name.push(ch);
    }

    pub fn backspace_name(&mut self) {
        self.name.pop();
    }

    pub fn append_text(&mut self, ch: char) {
        self.text.push(ch);
    }

    pub fn newline(&mut self) {
        self.text.push('\n');
    }

    pub fn backspace_text(&mut self) {
        self.text.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::{App, AppAction, Focus, IssueFilter, View, WorkItemMode};
    use crate::config::Config;
    use crate::store::{CommentRow, IssueRow, LocalRepoRow};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn dd_triggers_close_issue_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::CloseIssue));
    }

    #[test]
    fn dd_triggers_close_issue_action_in_detail_view() {
        let mut app = App::new(Config::default());
        app.set_issues(vec![IssueRow {
            id: 42,
            repo_id: 1,
            number: 7,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);
        app.set_current_issue(42, 7);
        app.set_view(View::IssueDetail);

        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::CloseIssue));
    }

    #[test]
    fn f_cycles_issue_filter() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 1,
                state: "open".to_string(),
                title: "Open".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 2,
                state: "closed".to_string(),
                title: "Closed".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);

        assert_eq!(app.issue_filter(), IssueFilter::Open);
        assert_eq!(app.issues_for_view().len(), 1);

        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Closed);
        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));

        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Open);
        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(1));
    }

    #[test]
    fn p_toggles_between_issue_and_pr_modes() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 1,
                state: "open".to_string(),
                title: "Issue".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 2,
                state: "open".to_string(),
                title: "PR".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: true,
            },
        ]);

        assert_eq!(app.work_item_mode(), WorkItemMode::Issues);
        assert_eq!(app.issues_for_view().len(), 1);
        assert!(!app.issues_for_view()[0].is_pr);

        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        assert_eq!(app.work_item_mode(), WorkItemMode::PullRequests);
        assert_eq!(app.issues_for_view().len(), 1);
        assert!(app.issues_for_view()[0].is_pr);
    }

    #[test]
    fn v_triggers_checkout_pull_request_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::CheckoutPullRequest));
    }

    #[test]
    fn r_requests_pull_request_files_sync_for_pr_detail() {
        let mut app = App::new(Config::default());
        app.set_view(View::IssueDetail);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 10,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        }]);
        app.set_current_issue(1, 10);

        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        assert!(app.take_pull_request_files_sync_request());
    }

    #[test]
    fn issue_filter_uses_1_and_2_shortcuts() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Closed);

        app.on_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Open);
    }

    #[test]
    fn bracket_keys_do_not_change_issue_filter() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Closed);

        app.on_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Closed);

        app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Closed);
    }

    #[test]
    fn ctrl_l_moves_focus_to_preview_and_j_scrolls_preview() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Open".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        assert_eq!(app.focus(), Focus::IssuesList);
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(app.focus(), Focus::IssuesPreview);

        app.set_issues_preview_max_scroll(5);
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.issues_preview_scroll(), 1);
    }

    #[test]
    fn a_cycles_assignee_filter() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 1,
                state: "open".to_string(),
                title: "One".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: "alex".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 2,
                state: "open".to_string(),
                title: "Two".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: "sam".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 3,
                repo_id: 1,
                number: 3,
                state: "open".to_string(),
                title: "Three".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);

        assert_eq!(app.assignee_filter_label(), "all");
        assert_eq!(app.issues_for_view().len(), 3);

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.assignee_filter_label(), "unassigned");
        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(3));

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.assignee_filter_label(), "alex");
        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(1));

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.assignee_filter_label(), "sam");

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.assignee_filter_label(), "all");
    }

    #[test]
    fn slash_search_filters_and_escape_clears() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 101,
                state: "open".to_string(),
                title: "Login bug".to_string(),
                body: "Fails for SSO users".to_string(),
                labels: "bug,auth".to_string(),
                assignees: "alex".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 202,
                state: "open".to_string(),
                title: "Docs polish".to_string(),
                body: "Update README".to_string(),
                labels: "docs".to_string(),
                assignees: "sam".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);

        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(app.issue_search_mode());

        app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));

        assert_eq!(app.issue_query(), "bug");
        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(101));

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.issue_search_mode());
        assert_eq!(app.issue_query(), "");
        assert_eq!(app.issues_for_view().len(), 2);
    }

    #[test]
    fn slash_search_matches_issue_number() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 777,
            state: "open".to_string(),
            title: "Telemetry".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('#'), KeyModifiers::SHIFT));
        app.on_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));

        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(777));
    }

    #[test]
    fn reopen_action_for_closed_issue() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 9,
            repo_id: 1,
            number: 99,
            state: "closed".to_string(),
            title: "Closed".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);
        app.set_issue_filter(IssueFilter::Closed);

        app.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::ReopenIssue));
    }

    #[test]
    fn comment_action_on_issue() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 9,
            repo_id: 1,
            number: 99,
            state: "open".to_string(),
            title: "Open".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::AddIssueComment));
    }

    #[test]
    fn e_triggers_edit_comment_action_in_comments_view() {
        let mut app = App::new(Config::default());
        app.set_view(View::IssueComments);
        app.set_comments(vec![CommentRow {
            id: 300,
            issue_id: 20,
            author: "dev".to_string(),
            body: "hello".to_string(),
            created_at: Some("2024-01-02T01:00:00Z".to_string()),
            last_accessed_at: None,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::EditIssueComment));
    }

    #[test]
    fn x_triggers_delete_comment_action_in_comments_view() {
        let mut app = App::new(Config::default());
        app.set_view(View::IssueComments);
        app.set_comments(vec![CommentRow {
            id: 301,
            issue_id: 20,
            author: "dev".to_string(),
            body: "hello".to_string(),
            created_at: Some("2024-01-02T01:00:00Z".to_string()),
            last_accessed_at: None,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::DeleteIssueComment));
    }

    #[test]
    fn j_and_k_navigate_comments_in_full_comments_view() {
        let mut app = App::new(Config::default());
        app.set_view(View::IssueComments);
        app.set_comments(vec![
            CommentRow {
                id: 401,
                issue_id: 20,
                author: "dev".to_string(),
                body: "one".to_string(),
                created_at: Some("2024-01-02T01:00:00Z".to_string()),
                last_accessed_at: None,
            },
            CommentRow {
                id: 402,
                issue_id: 20,
                author: "dev".to_string(),
                body: "two".to_string(),
                created_at: Some("2024-01-02T01:01:00Z".to_string()),
                last_accessed_at: None,
            },
        ]);

        assert_eq!(app.selected_comment(), 0);
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.selected_comment(), 1);

        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.selected_comment(), 0);
    }

    #[test]
    fn enter_submits_edited_comment_editor() {
        let mut app = App::new(Config::default());
        app.open_comment_edit_editor(View::IssueComments, 99, "existing");

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitEditedComment));
    }

    #[test]
    fn slash_search_supports_qualifier_tokens() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 11,
                state: "open".to_string(),
                title: "Auth".to_string(),
                body: String::new(),
                labels: "bug,security".to_string(),
                assignees: "alex".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 22,
                state: "closed".to_string(),
                title: "Docs".to_string(),
                body: String::new(),
                labels: "docs".to_string(),
                assignees: "sam".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);
        app.set_issue_filter(IssueFilter::Closed);

        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for ch in "is:closed label:docs".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(22));
    }

    #[test]
    fn assignee_qualifier_matches_exact_user() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 11,
                state: "open".to_string(),
                title: "One".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: "alex,sam".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 12,
                state: "open".to_string(),
                title: "Two".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: "samiam".to_string(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);

        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for ch in "assignee:sam".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(11));
    }

    #[test]
    fn is_pr_query_matches_pull_requests() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 11,
                state: "open".to_string(),
                title: "Issue".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 12,
                state: "open".to_string(),
                title: "PR".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: true,
            },
        ]);

        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for ch in "is:pr".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(12));
    }

    #[test]
    fn enter_submits_comment_editor() {
        let mut app = App::new(Config::default());
        app.open_issue_comment_editor(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitIssueComment));
    }

    #[test]
    fn shift_enter_adds_newline_in_comment_editor() {
        let mut app = App::new(Config::default());
        app.open_issue_comment_editor(View::Issues);
        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
        app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));

        assert_eq!(app.editor().text(), "a\nb");
        assert_eq!(app.take_action(), None);
    }

    #[test]
    fn ctrl_j_adds_newline_in_comment_editor() {
        let mut app = App::new(Config::default());
        app.open_issue_comment_editor(View::Issues);
        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));

        assert_eq!(app.editor().text(), "a\nb");
        assert_eq!(app.take_action(), None);
    }

    #[test]
    fn set_issues_preserves_selected_issue_when_still_present() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 1,
                state: "open".to_string(),
                title: "One".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 2,
                state: "open".to_string(),
                title: "Two".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);

        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));

        app.set_issues(vec![
            IssueRow {
                id: 10,
                repo_id: 1,
                number: 2,
                state: "open".to_string(),
                title: "Two refreshed".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
            IssueRow {
                id: 11,
                repo_id: 1,
                number: 3,
                state: "open".to_string(),
                title: "Three".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: false,
            },
        ]);

        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));
    }

    #[test]
    fn update_issue_state_rebuilds_filtered_view() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 10,
            state: "open".to_string(),
            title: "One".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        assert_eq!(app.issues_for_view().len(), 1);
        app.update_issue_state_by_number(10, "closed");
        assert_eq!(app.issues_for_view().len(), 0);
    }

    #[test]
    fn closed_filter_sorts_by_recently_closed() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![
            IssueRow {
                id: 1,
                repo_id: 1,
                number: 10,
                state: "closed".to_string(),
                title: "older close".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: Some("2024-01-01T00:00:00Z".to_string()),
                is_pr: false,
            },
            IssueRow {
                id: 2,
                repo_id: 1,
                number: 11,
                state: "closed".to_string(),
                title: "newer close".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: Some("2024-01-02T00:00:00Z".to_string()),
                is_pr: false,
            },
        ]);

        app.set_issue_filter(IssueFilter::Closed);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(11));
    }

    #[test]
    fn repo_picker_keeps_distinct_rows() {
        let mut app = App::new(Config::default());
        app.set_repos(vec![
            LocalRepoRow {
                path: "/tmp/one".to_string(),
                remote_name: "origin".to_string(),
                owner: "acme".to_string(),
                repo: "glyph".to_string(),
                url: "https://github.com/acme/glyph.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
            LocalRepoRow {
                path: "/tmp/two".to_string(),
                remote_name: "upstream".to_string(),
                owner: "acme".to_string(),
                repo: "glyph".to_string(),
                url: "https://github.com/acme/glyph.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
            LocalRepoRow {
                path: "/tmp/three".to_string(),
                remote_name: "origin".to_string(),
                owner: "other".to_string(),
                repo: "core".to_string(),
                url: "https://github.com/other/core.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
        ]);

        assert_eq!(app.filtered_repo_rows().len(), 3);
        assert_eq!(app.filtered_repo_rows()[0].owner, "acme");
        assert_eq!(app.filtered_repo_rows()[0].repo, "glyph");
        assert_eq!(app.filtered_repo_rows()[1].remote_name, "upstream");
    }

    #[test]
    fn repo_picker_search_filters_entries() {
        let mut app = App::new(Config::default());
        app.set_repos(vec![
            LocalRepoRow {
                path: "/tmp/one".to_string(),
                remote_name: "origin".to_string(),
                owner: "acme".to_string(),
                repo: "glyph".to_string(),
                url: "https://github.com/acme/glyph.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
            LocalRepoRow {
                path: "/tmp/two".to_string(),
                remote_name: "origin".to_string(),
                owner: "other".to_string(),
                repo: "core".to_string(),
                url: "https://github.com/other/core.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
        ]);

        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for ch in "acme".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert_eq!(app.filtered_repo_rows().len(), 1);
        assert_eq!(app.filtered_repo_rows()[0].owner, "acme");

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.filtered_repo_rows().len(), 2);
    }

    #[test]
    fn ctrl_g_resets_repo_picker_query_when_reopened() {
        let mut app = App::new(Config::default());
        app.set_repos(vec![
            LocalRepoRow {
                path: "/tmp/one".to_string(),
                remote_name: "origin".to_string(),
                owner: "acme".to_string(),
                repo: "glyph".to_string(),
                url: "https://github.com/acme/glyph.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
            LocalRepoRow {
                path: "/tmp/two".to_string(),
                remote_name: "origin".to_string(),
                owner: "other".to_string(),
                repo: "core".to_string(),
                url: "https://github.com/other/core.git".to_string(),
                last_seen: None,
                last_scanned: None,
            },
        ]);

        app.set_view(View::Issues);
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for ch in "acme".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert_eq!(app.repo_query(), "acme");
        assert_eq!(app.filtered_repo_rows().len(), 1);

        app.set_view(View::Issues);
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));

        assert_eq!(app.repo_query(), "");
        assert_eq!(app.filtered_repo_rows().len(), 2);
        assert!(!app.repo_search_mode());
    }

    #[test]
    fn l_triggers_edit_labels_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: "bug".to_string(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::EditLabels));
    }

    #[test]
    fn shift_a_triggers_edit_assignees_action_in_detail() {
        let mut app = App::new(Config::default());
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 1,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: "alex".to_string(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);
        app.set_current_issue(1, 1);
        app.set_view(View::IssueDetail);

        app.on_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(app.take_action(), Some(AppAction::EditAssignees));
    }

    #[test]
    fn shift_a_triggers_edit_assignees_action_in_issues() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(app.take_action(), Some(AppAction::EditAssignees));
    }

    #[test]
    fn labels_picker_enter_submits() {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "triage".to_string()],
            "bug,triage",
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
    }

    #[test]
    fn labels_picker_enter_selects_highlighted_when_none_selected() {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "docs".to_string()],
            "",
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
        assert_eq!(app.selected_labels(), vec!["bug".to_string()]);
    }

    #[test]
    fn labels_picker_enter_adds_highlighted_when_existing_labels_present() {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "docs".to_string()],
            "bug",
        );

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
        assert_eq!(app.selected_labels(), vec!["bug".to_string(), "docs".to_string()]);
    }

    #[test]
    fn assignee_picker_enter_selects_highlighted_when_none_selected() {
        let mut app = App::new(Config::default());
        app.open_assignee_picker(
            View::Issues,
            vec!["alex".to_string(), "sam".to_string()],
            "",
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
        assert_eq!(app.selected_assignees(), vec!["alex".to_string()]);
    }

    #[test]
    fn assignee_picker_enter_adds_highlighted_when_existing_assignees_present() {
        let mut app = App::new(Config::default());
        app.open_assignee_picker(
            View::Issues,
            vec!["alex".to_string(), "sam".to_string()],
            "alex",
        );

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
        assert_eq!(
            app.selected_assignees(),
            vec!["alex".to_string(), "sam".to_string()]
        );
    }

    #[test]
    fn labels_picker_enter_removes_highlighted_when_already_selected() {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "docs".to_string()],
            "bug,docs",
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
        assert_eq!(app.selected_labels(), vec!["docs".to_string()]);
    }

    #[test]
    fn assignee_picker_enter_removes_highlighted_when_already_selected() {
        let mut app = App::new(Config::default());
        app.open_assignee_picker(
            View::Issues,
            vec!["alex".to_string(), "sam".to_string()],
            "alex,sam",
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::SubmitAssignees));
        assert_eq!(app.selected_assignees(), vec!["sam".to_string()]);
    }

    #[test]
    fn label_picker_type_filter_can_match_c_prefix() {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "customer".to_string(), "docs".to_string()],
            "",
        );

        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        assert_eq!(app.label_query(), "c");
        assert_eq!(app.filtered_label_indices().len(), 2);
        assert_eq!(app.selected_label_option(), app.filtered_label_indices()[0]);

    }

    #[test]
    fn merge_label_options_dedupes_case_insensitive() {
        let mut app = App::new(Config::default());
        app.open_label_picker(
            View::Issues,
            vec!["bug".to_string(), "Docs".to_string()],
            "",
        );

        app.merge_label_options(vec![
            "docs".to_string(),
            "enhancement".to_string(),
            "BUG".to_string(),
        ]);

        assert_eq!(app.label_options(), &["bug", "Docs", "enhancement"]);
    }
}
