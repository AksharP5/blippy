use std::collections::{HashMap, HashSet};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::config::{CommentDefault, Config};
use crate::git::RemoteInfo;
use crate::keybinds::Keybinds;
use crate::markdown;
use crate::pr_diff::{DiffKind, parse_patch};
use crate::store::{CommentRow, IssueRow, LocalRepoRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    RepoPicker,
    RemoteChooser,
    Issues,
    IssueDetail,
    IssueComments,
    PullRequestFiles,
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
    OpenLinkedPullRequestInBrowser,
    OpenLinkedPullRequestInTui,
    OpenLinkedIssueInBrowser,
    OpenLinkedIssueInTui,
    CloseIssue,
    ReopenIssue,
    AddIssueComment,
    SubmitIssueComment,
    EditIssueComment,
    DeleteIssueComment,
    SubmitEditedComment,
    AddPullRequestReviewComment,
    SubmitPullRequestReviewComment,
    EditPullRequestReviewComment,
    DeletePullRequestReviewComment,
    ResolvePullRequestReviewComment,
    TogglePullRequestFileViewed,
    SubmitEditedPullRequestReviewComment,
    EditLabels,
    EditAssignees,
    SubmitLabels,
    SubmitAssignees,
    PickPreset,
    SavePreset,
    SubmitComment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    RepoPicker,
    Back,
    RepoListPane,
    RepoRow(usize),
    RemoteListPane,
    RemoteRow(usize),
    IssueTabOpen,
    IssueTabClosed,
    IssueRow(usize),
    IssuesListPane,
    IssuesPreviewPane,
    IssueBodyPane,
    IssueSidePane,
    LinkedPullRequestTuiButton,
    LinkedPullRequestWebButton,
    LinkedIssueTuiButton,
    LinkedIssueWebButton,
    CommentRow(usize),
    CommentsPane,
    PullRequestFilesPane,
    PullRequestDiffPane,
    PullRequestFileRow(usize),
    PullRequestDiffRow(usize, ReviewSide),
    PullRequestFocusFiles,
    PullRequestFocusDiff,
    LabelOption(usize),
    LabelApply,
    LabelCancel,
    AssigneeOption(usize),
    AssigneeApply,
    AssigneeCancel,
    PresetOption(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MouseRegion {
    target: MouseTarget,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
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
pub enum PullRequestReviewFocus {
    Files,
    Diff,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSide {
    Left,
    Right,
}

impl ReviewSide {
    pub fn as_api_side(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReviewComment {
    pub id: i64,
    pub thread_id: Option<String>,
    pub resolved: bool,
    pub anchored: bool,
    pub path: String,
    pub line: i64,
    pub side: ReviewSide,
    pub body: String,
    pub author: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReviewTarget {
    pub path: String,
    pub line: i64,
    pub side: ReviewSide,
    pub start_line: Option<i64>,
    pub start_side: Option<ReviewSide>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffHunkRange {
    start: usize,
    end: usize,
}

fn pull_request_hunk_end(rows: &[crate::pr_diff::DiffRow], hunk_start: usize) -> Option<usize> {
    let row = rows.get(hunk_start)?;
    if row.kind != DiffKind::Hunk {
        return None;
    }
    let mut index = hunk_start + 1;
    while index < rows.len() {
        if rows[index].kind == DiffKind::Hunk {
            return Some(index.saturating_sub(1));
        }
        index += 1;
    }
    Some(rows.len().saturating_sub(1))
}

fn pull_request_hunk_range_for_row(
    rows: &[crate::pr_diff::DiffRow],
    row_index: usize,
) -> Option<DiffHunkRange> {
    if rows.is_empty() {
        return None;
    }
    let row_index = row_index.min(rows.len() - 1);
    let mut hunk_start = None;
    let mut index = row_index;
    loop {
        if rows[index].kind == DiffKind::Hunk {
            hunk_start = Some(index);
            break;
        }
        if index == 0 {
            break;
        }
        index -= 1;
    }
    let hunk_start = hunk_start?;
    let hunk_end = pull_request_hunk_end(rows, hunk_start)?;
    if row_index > hunk_end {
        return None;
    }
    Some(DiffHunkRange {
        start: hunk_start,
        end: hunk_end,
    })
}

impl PendingIssueAction {
    fn label(self) -> &'static str {
        match self {
            Self::Closing => "closing",
            Self::Reopening => "reopening",
            Self::UpdatingLabels => "updating labels",
            Self::UpdatingAssignees => "updating assignees",
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

#[derive(Debug, Default)]
struct SyncState {
    scanning: bool,
    syncing: bool,
    repo_permissions_syncing: bool,
    repo_permissions_sync_requested: bool,
    repo_issue_metadata_editable: Option<bool>,
    repo_labels_syncing: bool,
    repo_labels_sync_requested: bool,
    comment_syncing: bool,
    pull_request_files_syncing: bool,
    pull_request_review_comments_syncing: bool,
    comment_sync_requested: bool,
    pull_request_files_sync_requested: bool,
    pull_request_review_comments_sync_requested: bool,
    sync_requested: bool,
    rescan_requested: bool,
}

#[derive(Debug, Default)]
struct LinkedState {
    pull_requests: HashMap<i64, Option<i64>>,
    issues: HashMap<i64, Option<i64>>,
    pull_request_lookups: HashSet<i64>,
    issue_lookups: HashSet<i64>,
    navigation_origin: Option<(i64, WorkItemMode)>,
}

#[derive(Debug, Default)]
struct RepoContextState {
    owner: Option<String>,
    repo: Option<String>,
    path: Option<String>,
    issue_id: Option<i64>,
    issue_number: Option<i64>,
}

#[derive(Debug)]
struct PullRequestState {
    pull_request_files_issue_id: Option<i64>,
    pull_request_id: Option<String>,
    pull_request_files: Vec<PullRequestFile>,
    pull_request_viewed_files: HashSet<String>,
    pull_request_collapsed_hunks: HashMap<String, HashSet<usize>>,
    pull_request_review_comments: Vec<PullRequestReviewComment>,
    pull_request_review_focus: PullRequestReviewFocus,
    selected_pull_request_file: usize,
    selected_pull_request_diff_line: usize,
    pull_request_diff_scroll: u16,
    pull_request_diff_max_scroll: u16,
    pull_request_diff_horizontal_scroll: u16,
    pull_request_diff_horizontal_max: u16,
    pull_request_diff_expanded: bool,
    pull_request_review_side: ReviewSide,
    pull_request_visual_mode: bool,
    pull_request_visual_anchor: Option<usize>,
    selected_pull_request_review_comment_id: Option<i64>,
    editing_pull_request_review_comment_id: Option<i64>,
    pending_review_target: Option<PullRequestReviewTarget>,
}

impl Default for PullRequestState {
    fn default() -> Self {
        Self {
            pull_request_files_issue_id: None,
            pull_request_id: None,
            pull_request_files: Vec::new(),
            pull_request_viewed_files: HashSet::new(),
            pull_request_collapsed_hunks: HashMap::new(),
            pull_request_review_comments: Vec::new(),
            pull_request_review_focus: PullRequestReviewFocus::Files,
            selected_pull_request_file: 0,
            selected_pull_request_diff_line: 0,
            pull_request_diff_scroll: 0,
            pull_request_diff_max_scroll: 0,
            pull_request_diff_horizontal_scroll: 0,
            pull_request_diff_horizontal_max: 0,
            pull_request_diff_expanded: false,
            pull_request_review_side: ReviewSide::Right,
            pull_request_visual_mode: false,
            pull_request_visual_anchor: None,
            selected_pull_request_review_comment_id: None,
            editing_pull_request_review_comment_id: None,
            pending_review_target: None,
        }
    }
}

#[derive(Debug, Default)]
struct MetadataPickerState {
    label_options: Vec<String>,
    label_selected: HashSet<String>,
    selected_label_option: usize,
    label_query: String,
    assignee_options: Vec<String>,
    assignee_selected: HashSet<String>,
    selected_assignee_option: usize,
    assignee_query: String,
}

#[derive(Debug, Default)]
struct NavigationState {
    selected_repo: usize,
    selected_remote: usize,
    selected_issue: usize,
    selected_comment: usize,
    issue_detail_scroll: u16,
    issue_detail_max_scroll: u16,
    issues_preview_scroll: u16,
    issues_preview_max_scroll: u16,
    issue_comments_scroll: u16,
    issue_comments_max_scroll: u16,
    issue_recent_comments_scroll: u16,
    issue_recent_comments_max_scroll: u16,
}

#[derive(Debug, Default)]
struct SearchState {
    repo_query: String,
    repo_search_mode: bool,
    filtered_repo_indices: Vec<usize>,
    issue_query: String,
    issue_search_mode: bool,
    filtered_issue_indices: Vec<usize>,
    help_overlay_visible: bool,
}

#[derive(Debug, Default)]
struct InteractionState {
    action: Option<AppAction>,
    pending_issue_actions: HashMap<i64, PendingIssueAction>,
    pending_g: bool,
    pending_d: bool,
    mouse_regions: Vec<MouseRegion>,
}

#[derive(Debug)]
struct EditorFlowState {
    cancel_view: View,
    editing_comment_id: Option<i64>,
}

impl Default for EditorFlowState {
    fn default() -> Self {
        Self {
            cancel_view: View::Issues,
            editing_comment_id: None,
        }
    }
}

#[derive(Debug, Default)]
struct PresetState {
    choice: usize,
}

mod editor;
mod metadata;
mod preset;

mod navigation_keyboard;
mod navigation_mouse;
mod pull_request;
mod search;

mod linked;
mod state;

mod accessors;
mod input;

pub struct App {
    should_quit: bool,
    config: Config,
    keybinds: Keybinds,
    view: View,
    focus: Focus,
    navigation: NavigationState,
    repos: Vec<LocalRepoRow>,
    remotes: Vec<RemoteInfo>,
    issues: Vec<IssueRow>,
    comments: Vec<CommentRow>,
    issue_filter: IssueFilter,
    work_item_mode: WorkItemMode,
    assignee_filter: AssigneeFilter,
    search: SearchState,
    status: String,
    sync: SyncState,
    repo_label_colors: HashMap<String, String>,
    interaction: InteractionState,
    context: RepoContextState,
    linked: LinkedState,
    pull_request: PullRequestState,
    comment_editor: CommentEditorState,
    editor_flow: EditorFlowState,
    metadata_picker: MetadataPickerState,
    preset: PresetState,
}

impl App {
    pub fn new(config: Config) -> Self {
        let keybinds = Keybinds::from_overrides(&config.keybinds);
        Self {
            should_quit: false,
            config,
            keybinds,
            view: View::RepoPicker,
            focus: Focus::IssuesList,
            navigation: NavigationState::default(),
            repos: Vec::new(),
            remotes: Vec::new(),
            issues: Vec::new(),
            comments: Vec::new(),
            issue_filter: IssueFilter::Open,
            work_item_mode: WorkItemMode::Issues,
            assignee_filter: AssigneeFilter::All,
            search: SearchState::default(),
            status: String::new(),
            sync: SyncState::default(),
            repo_label_colors: HashMap::new(),
            interaction: InteractionState::default(),
            context: RepoContextState::default(),
            linked: LinkedState::default(),
            pull_request: PullRequestState::default(),
            comment_editor: CommentEditorState::default(),
            editor_flow: EditorFlowState::default(),
            metadata_picker: MetadataPickerState::default(),
            preset: PresetState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    CloseIssue,
    AddComment,
    EditComment,
    AddPullRequestReviewComment,
    EditPullRequestReviewComment,
    AddPreset,
}

impl EditorMode {
    fn allows_multiline(self) -> bool {
        matches!(
            self,
            Self::CloseIssue
                | Self::AddComment
                | Self::EditComment
                | Self::AddPullRequestReviewComment
                | Self::EditPullRequestReviewComment
                | Self::AddPreset
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

    pub fn reset_for_pull_request_review_comment(&mut self) {
        self.mode = EditorMode::AddPullRequestReviewComment;
        self.text.clear();
    }

    pub fn reset_for_pull_request_review_comment_edit(&mut self, body: &str) {
        self.mode = EditorMode::EditPullRequestReviewComment;
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
mod tests;
