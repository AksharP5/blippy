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
    CloseIssue,
    ReopenIssue,
    AddIssueComment,
    SubmitIssueComment,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPickerEntry {
    pub owner: String,
    pub repo: String,
    pub paths: usize,
    pub remotes: usize,
    pub last_seen: Option<String>,
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

    fn prev(self) -> Self {
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
    assignee_filter: AssigneeFilter,
    repo_query: String,
    repo_search_mode: bool,
    repo_entries: Vec<RepoPickerEntry>,
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
    comment_sync_requested: bool,
    sync_requested: bool,
    rescan_requested: bool,
    action: Option<AppAction>,
    current_owner: Option<String>,
    current_repo: Option<String>,
    current_issue_id: Option<i64>,
    current_issue_number: Option<i64>,
    pending_issue_actions: HashMap<i64, PendingIssueAction>,
    pending_g: bool,
    pending_d: bool,
    comment_editor: CommentEditorState,
    editor_cancel_view: View,
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
            assignee_filter: AssigneeFilter::All,
            repo_query: String::new(),
            repo_search_mode: false,
            repo_entries: Vec::new(),
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
            comment_sync_requested: false,
            sync_requested: false,
            rescan_requested: false,
            action: None,
            current_owner: None,
            current_repo: None,
            current_issue_id: None,
            current_issue_number: None,
            pending_issue_actions: HashMap::new(),
            pending_g: false,
            pending_d: false,
            comment_editor: CommentEditorState::default(),
            editor_cancel_view: View::Issues,
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

    pub fn repo_picker_entries(&self) -> &[RepoPickerEntry] {
        &self.repo_entries
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

    pub fn assignee_filter_label(&self) -> String {
        self.assignee_filter.label()
    }

    pub fn has_assignee_filter(&self) -> bool {
        !matches!(self.assignee_filter, AssigneeFilter::All)
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
            .filter(|issue| issue.state.eq_ignore_ascii_case("open"))
            .count();
        let closed = self
            .issues
            .iter()
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

    pub fn selected_repo_slug(&self) -> Option<(String, String)> {
        let entry = self.repo_entries.get(self.selected_repo)?;
        Some((entry.owner.clone(), entry.repo.clone()))
    }

    pub fn selected_remote(&self) -> usize {
        self.selected_remote
    }

    pub fn selected_issue(&self) -> usize {
        self.selected_issue
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
            KeyCode::Char('a') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.cycle_assignee_filter(true);
            }
            KeyCode::Char('A') if self.view == View::Issues => {
                self.cycle_assignee_filter(false);
            }
            KeyCode::Char('[') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.set_issue_filter(self.issue_filter.prev());
            }
            KeyCode::Char(']') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.set_issue_filter(self.issue_filter.next());
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
            KeyCode::Char('n') if self.view == View::IssueComments => self.jump_next_comment(),
            KeyCode::Char('p') if self.view == View::IssueComments => self.jump_prev_comment(),
            KeyCode::Char('m')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::AddIssueComment);
            }
            KeyCode::Char('l')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::EditLabels);
            }
            KeyCode::Char('A') if self.view == View::Issues => {
                self.action = Some(AppAction::EditAssignees);
            }
            KeyCode::Char('a') if matches!(self.view, View::IssueDetail | View::IssueComments) => {
                self.action = Some(AppAction::EditAssignees);
            }
            KeyCode::Char('u')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::ReopenIssue);
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
            KeyCode::Char('k') | KeyCode::Up => self.move_selection_up(),
            KeyCode::Char('j') | KeyCode::Down => self.move_selection_down(),
            KeyCode::Enter => self.activate_selection(),
            KeyCode::Char('o')
                if matches!(self.view, View::Issues | View::IssueDetail | View::IssueComments) =>
            {
                self.action = Some(AppAction::OpenInBrowser);
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
        self.rebuild_repo_picker_entries();
        if self.selected_repo >= self.repo_entries.len() {
            self.selected_repo = self.repo_entries.len().saturating_sub(1);
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
        self.comments = comments;
        if self.comments.is_empty() {
            self.selected_comment = 0;
            self.issue_comments_scroll = 0;
            self.issue_recent_comments_scroll = 0;
            self.issue_comments_max_scroll = 0;
            self.issue_recent_comments_max_scroll = 0;
            return;
        }
        self.selected_comment = self.comments.len() - 1;
        self.issue_comments_scroll = 0;
        self.issue_recent_comments_scroll = 0;
        self.issue_comments_max_scroll = 0;
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
        self.current_issue_id = None;
        self.current_issue_number = None;
        self.repo_search_mode = false;
        self.assignee_filter = AssigneeFilter::All;
        self.issue_query.clear();
        self.issue_search_mode = false;
    }

    pub fn set_current_issue(&mut self, issue_id: i64, issue_number: i64) {
        self.current_issue_id = Some(issue_id);
        self.current_issue_number = Some(issue_number);
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
        self.comment_editor.reset_for_close();
        self.editor_cancel_view = View::CommentPresetPicker;
        self.set_view(View::CommentEditor);
    }

    pub fn open_issue_comment_editor(&mut self, return_view: View) {
        self.comment_editor.reset_for_comment();
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_issue_labels_editor(&mut self, return_view: View, labels: &str) {
        self.comment_editor.reset_for_labels(labels);
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_issue_assignees_editor(&mut self, return_view: View, assignees: &str) {
        self.comment_editor.reset_for_assignees(assignees);
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn editor_cancel_view(&self) -> View {
        self.editor_cancel_view
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
                self.issue_comments_scroll = self.issue_comments_scroll.saturating_sub(1);
            }
            View::CommentPresetPicker => {
                if self.preset_choice > 0 {
                    self.preset_choice -= 1;
                }
            }
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    fn move_selection_down(&mut self) {
        match self.view {
            View::RepoPicker => {
                if self.selected_repo + 1 < self.repo_entries.len() {
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
                let max = self.issue_comments_max_scroll;
                self.issue_comments_scroll = self.issue_comments_scroll.saturating_add(1).min(max);
            }
            View::CommentPresetPicker => {
                let max = self.preset_items_len();
                if self.preset_choice + 1 < max {
                    self.preset_choice += 1;
                }
            }
            View::CommentPresetName | View::CommentEditor => {}
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
            View::CommentPresetName | View::CommentEditor => {}
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
            View::IssueComments => self.issue_comments_scroll = 0,
            View::CommentPresetPicker => self.preset_choice = 0,
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    fn jump_bottom(&mut self) {
        match self.view {
            View::RepoPicker => {
                if !self.repo_entries.is_empty() {
                    self.selected_repo = self.repo_entries.len() - 1;
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
                self.issue_comments_scroll = self.issue_comments_max_scroll;
            }
            View::CommentPresetPicker => {
                let max = self.preset_items_len();
                if max > 0 {
                    self.preset_choice = max - 1;
                }
            }
            View::CommentPresetName | View::CommentEditor => {}
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
        if offsets.is_empty() {
            return;
        }
        let current = self.issue_comments_scroll;
        let mut target = self.issue_comments_max_scroll;
        let mut index = offsets.len() - 1;
        for (offset_index, offset) in offsets.iter().enumerate() {
            if *offset > current {
                target = *offset;
                index = offset_index;
                break;
            }
        }
        self.selected_comment = index;
        self.issue_comments_scroll = target.min(self.issue_comments_max_scroll);
    }

    fn jump_prev_comment(&mut self) {
        let offsets = self.comment_offsets();
        if offsets.is_empty() {
            return;
        }
        let current = self.issue_comments_scroll;
        let mut target = 0;
        let mut index = 0;
        for (offset_index, offset) in offsets.iter().enumerate() {
            if *offset >= current {
                break;
            }
            target = *offset;
            index = offset_index;
        }
        self.selected_comment = index;
        self.issue_comments_scroll = target;
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

    fn rebuild_issue_filter(&mut self) {
        let query = self.issue_query.trim().to_ascii_lowercase();
        self.filtered_issue_indices = self
            .issues
            .iter()
            .enumerate()
            .filter_map(|(index, issue)| {
                if self.issue_filter.matches(issue)
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
        let options = self.assignee_options();
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
            "Assignee: {} ({} issues)",
            self.assignee_filter.label(),
            self.filtered_issue_indices.len()
        );
    }

    fn assignee_options(&self) -> Vec<AssigneeFilter> {
        let mut users = self
            .issues
            .iter()
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
                        EditorMode::AddPreset => {
                            self.action = Some(AppAction::SavePreset);
                        }
                        EditorMode::EditLabels => {
                            self.action = Some(AppAction::SubmitLabels);
                        }
                        EditorMode::EditAssignees => {
                            self.action = Some(AppAction::SubmitAssignees);
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
            KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
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
            self.rebuild_repo_picker_entries();
            self.selected_repo = 0;
            self.status = "Repo search cleared".to_string();
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                self.repo_search_mode = false;
                self.repo_query.clear();
                self.rebuild_repo_picker_entries();
                self.selected_repo = 0;
                self.status = String::new();
            }
            KeyCode::Enter => {
                self.repo_search_mode = false;
                self.status = format!("{} repos", self.repo_entries.len());
            }
            KeyCode::Backspace => {
                self.repo_query.pop();
                self.rebuild_repo_picker_entries();
                self.selected_repo = 0;
            }
            KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                self.repo_query.push(ch);
                self.rebuild_repo_picker_entries();
                self.selected_repo = 0;
            }
            _ => {}
        }
        true
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

    fn rebuild_repo_picker_entries(&mut self) {
        #[derive(Default)]
        struct RepoGroup {
            owner: String,
            repo: String,
            paths: HashSet<String>,
            remotes: HashSet<String>,
            last_seen: Option<String>,
            order: usize,
            matches_query: bool,
        }

        let query = self.repo_query.trim().to_ascii_lowercase();
        let mut groups = HashMap::<String, RepoGroup>::new();

        for (index, repo) in self.repos.iter().enumerate() {
            let key = format!("{}/{}", repo.owner, repo.repo);
            let group = groups.entry(key).or_insert_with(|| RepoGroup {
                owner: repo.owner.clone(),
                repo: repo.repo.clone(),
                order: index,
                ..RepoGroup::default()
            });

            group.paths.insert(repo.path.clone());
            group.remotes.insert(repo.remote_name.clone());
            if group.last_seen.is_none() {
                group.last_seen = repo.last_seen.clone();
            }

            if query.is_empty() {
                group.matches_query = true;
                continue;
            }

            let haystack = format!(
                "{} {} {} {} {}",
                repo.owner, repo.repo, repo.path, repo.remote_name, repo.url
            )
            .to_ascii_lowercase();
            if haystack.contains(query.as_str()) {
                group.matches_query = true;
            }
        }

        let mut entries = groups
            .into_values()
            .filter(|group| group.matches_query)
            .collect::<Vec<RepoGroup>>();
        entries.sort_by_key(|group| group.order);

        self.repo_entries = entries
            .into_iter()
            .map(|group| RepoPickerEntry {
                owner: group.owner,
                repo: group.repo,
                paths: group.paths.len(),
                remotes: group.remotes.len(),
                last_seen: group.last_seen,
            })
            .collect::<Vec<RepoPickerEntry>>();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    CloseIssue,
    AddComment,
    AddPreset,
    EditLabels,
    EditAssignees,
}

impl EditorMode {
    fn allows_multiline(self) -> bool {
        matches!(self, Self::CloseIssue | Self::AddComment | Self::AddPreset)
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

    pub fn reset_for_preset_name(&mut self) {
        self.mode = EditorMode::AddPreset;
        self.name.clear();
        self.text.clear();
    }

    pub fn reset_for_labels(&mut self, labels: &str) {
        self.mode = EditorMode::EditLabels;
        self.text = labels.to_string();
    }

    pub fn reset_for_assignees(&mut self, assignees: &str) {
        self.mode = EditorMode::EditAssignees;
        self.text = assignees.to_string();
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
    use super::{App, AppAction, Focus, IssueFilter, View};
    use crate::config::Config;
    use crate::store::{IssueRow, LocalRepoRow};
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

        app.on_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(app.assignee_filter_label(), "unassigned");
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
    fn repo_picker_groups_same_owner_repo() {
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

        assert_eq!(app.repo_picker_entries().len(), 2);
        assert_eq!(app.repo_picker_entries()[0].owner, "acme");
        assert_eq!(app.repo_picker_entries()[0].repo, "glyph");
        assert_eq!(app.repo_picker_entries()[0].paths, 2);
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

        assert_eq!(app.repo_picker_entries().len(), 1);
        assert_eq!(app.repo_picker_entries()[0].owner, "acme");

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.repo_picker_entries().len(), 2);
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
    fn a_triggers_edit_assignees_action_in_detail() {
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

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::EditAssignees));
    }

    #[test]
    fn labels_editor_enter_submits() {
        let mut app = App::new(Config::default());
        app.open_issue_labels_editor(View::Issues, "bug,triage");

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.take_action(), Some(AppAction::SubmitLabels));
    }
}
