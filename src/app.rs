use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use anyhow::Result;
use crate::config::{CommentDefault, Config};
use crate::git::RemoteInfo;
use crate::store::{CommentRow, IssueRow, LocalRepoRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    RepoPicker,
    RemoteChooser,
    Issues,
    IssueDetail,
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
    pending_g: bool,
    pending_d: bool,
    comment_editor: CommentEditorState,
    preset_choice: usize,
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
            pending_g: false,
            pending_d: false,
            comment_editor: CommentEditorState::default(),
            preset_choice: 0,
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

    pub fn comment_defaults(&self) -> &[CommentDefault] {
        &self.config.comment_defaults
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
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            if self.view == View::RepoPicker {
                self.rescan_requested = true;
                self.scanning = true;
                self.status = "Scanning...".to_string();
            }
            return;
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
                self.view = View::RepoPicker;
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.request_sync();
                self.status = "Syncing...".to_string();
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && self.view == View::IssueDetail => {
                self.request_comment_sync();
                self.status = "Syncing comments...".to_string();
            }
            KeyCode::Char('g') if key.modifiers.is_empty() => {
                if self.pending_g {
                    self.jump_top();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('d') if key.modifiers.is_empty() && self.view == View::Issues => {
                if self.issues.is_empty() {
                    self.pending_d = false;
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
            KeyCode::Char('b') if self.view == View::IssueDetail => {
                self.view = View::Issues;
            }
            KeyCode::Esc if self.view == View::IssueDetail => {
                self.view = View::Issues;
            }
            KeyCode::Esc if self.view == View::CommentPresetPicker => {
                self.view = View::Issues;
            }
            KeyCode::Char('k') | KeyCode::Up => self.move_selection_up(),
            KeyCode::Char('j') | KeyCode::Down => self.move_selection_down(),
            KeyCode::Enter => self.activate_selection(),
            KeyCode::Char('o') if matches!(self.view, View::Issues | View::IssueDetail) => {
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
        if self.comments.is_empty() {
            self.selected_comment = 0;
            return;
        }
        self.selected_comment = self.comments.len() - 1;
    }

    pub fn set_comment_defaults(&mut self, defaults: Vec<CommentDefault>) {
        self.config.comment_defaults = defaults;
        self.preset_choice = 0;
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
    }

    pub fn set_current_issue(&mut self, issue_id: i64, issue_number: i64) {
        self.current_issue_id = Some(issue_id);
        self.current_issue_number = Some(issue_number);
    }

    pub fn editor(&self) -> &CommentEditorState {
        &self.comment_editor
    }

    pub fn editor_mut(&mut self) -> &mut CommentEditorState {
        &mut self.comment_editor
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
            View::IssueDetail => {}
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
            View::Issues => self.selected_issue = 0,
            View::IssueDetail => self.selected_comment = 0,
            View::CommentPresetPicker => self.preset_choice = 0,
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    fn jump_bottom(&mut self) {
        match self.view {
            View::RepoPicker => {
                if !self.repos.is_empty() {
                    self.selected_repo = self.repos.len() - 1;
                }
            }
            View::RemoteChooser => {
                if !self.remotes.is_empty() {
                    self.selected_remote = self.remotes.len() - 1;
                }
            }
            View::Issues => {
                if !self.issues.is_empty() {
                    self.selected_issue = self.issues.len() - 1;
                }
            }
            View::IssueDetail => {
                if !self.comments.is_empty() {
                    self.selected_comment = self.comments.len() - 1;
                }
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

    fn handle_editor_key(&mut self, key: KeyEvent) {
        match self.view {
            View::CommentPresetName => match key.code {
                KeyCode::Esc => {
                    self.view = View::CommentPresetPicker;
                }
                KeyCode::Enter => {
                    if self.comment_editor.name().is_empty() {
                        self.status = "Preset name required".to_string();
                        return;
                    }
                    self.view = View::CommentEditor;
                }
                KeyCode::Backspace => self.comment_editor.backspace_name(),
                KeyCode::Char(ch) => self.comment_editor.append_name(ch),
                _ => {}
            },
            View::CommentEditor => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Enter)
                {
                    match self.comment_editor.mode() {
                        EditorMode::CloseIssue => {
                            self.action = Some(AppAction::SubmitComment);
                        }
                        EditorMode::AddPreset => {
                            self.action = Some(AppAction::SavePreset);
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.view = View::CommentPresetPicker;
                    }
                    KeyCode::Enter => self.comment_editor.newline(),
                    KeyCode::Backspace => self.comment_editor.backspace_text(),
                    KeyCode::Char(ch) => self.comment_editor.append_text(ch),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    CloseIssue,
    AddPreset,
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
    use super::{App, AppAction, View};
    use crate::config::Config;
    use crate::store::IssueRow;
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
            updated_at: None,
            is_pr: false,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::CloseIssue));
    }
}
