use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::collections::{HashMap, HashSet};

use crate::config::{CommentDefault, Config};
use crate::git::RemoteInfo;
use crate::keybinds::Keybinds;
use crate::markdown;
use crate::pr_diff::{DiffKind, parse_patch};
use crate::store::{CommentRow, IssueRow, LocalRepoRow};
use anyhow::Result;

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
    CopyStatus,
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

pub struct App {
    should_quit: bool,
    config: Config,
    keybinds: Keybinds,
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
    help_overlay_visible: bool,
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
    repo_permissions_syncing: bool,
    repo_permissions_sync_requested: bool,
    repo_issue_metadata_editable: Option<bool>,
    repo_labels_syncing: bool,
    repo_labels_sync_requested: bool,
    repo_label_colors: HashMap<String, String>,
    comment_syncing: bool,
    pull_request_files_syncing: bool,
    pull_request_review_comments_syncing: bool,
    comment_sync_requested: bool,
    pull_request_files_sync_requested: bool,
    pull_request_review_comments_sync_requested: bool,
    sync_requested: bool,
    rescan_requested: bool,
    action: Option<AppAction>,
    current_owner: Option<String>,
    current_repo: Option<String>,
    current_repo_path: Option<String>,
    current_issue_id: Option<i64>,
    current_issue_number: Option<i64>,
    linked_pull_requests: HashMap<i64, Option<i64>>,
    linked_issues: HashMap<i64, Option<i64>>,
    linked_pull_request_lookups: HashSet<i64>,
    linked_issue_lookups: HashSet<i64>,
    linked_navigation_origin: Option<(i64, WorkItemMode)>,
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
    pending_issue_actions: HashMap<i64, PendingIssueAction>,
    pending_g: bool,
    pending_d: bool,
    mouse_regions: Vec<MouseRegion>,
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
        let keybinds = Keybinds::from_overrides(&config.keybinds);
        Self {
            should_quit: false,
            config,
            keybinds,
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
            help_overlay_visible: false,
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
            repo_permissions_syncing: false,
            repo_permissions_sync_requested: false,
            repo_issue_metadata_editable: None,
            repo_labels_syncing: false,
            repo_labels_sync_requested: false,
            repo_label_colors: HashMap::new(),
            comment_syncing: false,
            pull_request_files_syncing: false,
            pull_request_review_comments_syncing: false,
            comment_sync_requested: false,
            pull_request_files_sync_requested: false,
            pull_request_review_comments_sync_requested: false,
            sync_requested: false,
            rescan_requested: false,
            action: None,
            current_owner: None,
            current_repo: None,
            current_repo_path: None,
            current_issue_id: None,
            current_issue_number: None,
            linked_pull_requests: HashMap::new(),
            linked_issues: HashMap::new(),
            linked_pull_request_lookups: HashSet::new(),
            linked_issue_lookups: HashSet::new(),
            linked_navigation_origin: None,
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
            pending_issue_actions: HashMap::new(),
            pending_g: false,
            pending_d: false,
            mouse_regions: Vec::new(),
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

    pub fn issue_by_number(&self, issue_number: i64) -> Option<&IssueRow> {
        self.issues
            .iter()
            .find(|issue| issue.number == issue_number)
    }

    pub fn linked_pull_request_for_issue(&self, issue_number: i64) -> Option<i64> {
        self.linked_pull_requests
            .get(&issue_number)
            .and_then(|pull_number| *pull_number)
    }

    pub fn linked_issue_for_pull_request(&self, pull_number: i64) -> Option<i64> {
        self.linked_issues
            .get(&pull_number)
            .and_then(|issue_number| *issue_number)
    }

    pub fn linked_pull_request_known(&self, issue_number: i64) -> bool {
        self.linked_pull_requests.contains_key(&issue_number)
    }

    pub fn linked_issue_known(&self, pull_number: i64) -> bool {
        self.linked_issues.contains_key(&pull_number)
    }

    pub fn begin_linked_pull_request_lookup(&mut self, issue_number: i64) -> bool {
        if self.linked_pull_request_known(issue_number) {
            return false;
        }
        self.linked_pull_request_lookups.insert(issue_number)
    }

    pub fn begin_linked_issue_lookup(&mut self, pull_number: i64) -> bool {
        if self.linked_issue_known(pull_number) {
            return false;
        }
        self.linked_issue_lookups.insert(pull_number)
    }

    pub fn end_linked_pull_request_lookup(&mut self, issue_number: i64) {
        self.linked_pull_request_lookups.remove(&issue_number);
    }

    pub fn end_linked_issue_lookup(&mut self, pull_number: i64) {
        self.linked_issue_lookups.remove(&pull_number);
    }

    pub fn set_linked_pull_request(&mut self, issue_number: i64, pull_number: Option<i64>) {
        self.end_linked_pull_request_lookup(issue_number);
        if pull_number.is_none()
            && self
                .linked_pull_requests
                .get(&issue_number)
                .is_some_and(|existing| existing.is_some())
        {
            return;
        }
        self.linked_pull_requests.insert(issue_number, pull_number);
        if let Some(pull_number) = pull_number {
            self.linked_issues.insert(pull_number, Some(issue_number));
            self.end_linked_issue_lookup(pull_number);
        }
    }

    pub fn set_linked_issue_for_pull_request(
        &mut self,
        pull_number: i64,
        issue_number: Option<i64>,
    ) {
        self.end_linked_issue_lookup(pull_number);
        if issue_number.is_none()
            && self
                .linked_issues
                .get(&pull_number)
                .is_some_and(|existing| existing.is_some())
        {
            return;
        }
        self.linked_issues.insert(pull_number, issue_number);
        if let Some(issue_number) = issue_number {
            self.linked_pull_requests
                .insert(issue_number, Some(pull_number));
            self.end_linked_pull_request_lookup(issue_number);
        }
    }

    pub fn capture_linked_navigation_origin(&mut self) {
        let issue = match self.current_or_selected_issue() {
            Some(issue) => issue,
            None => return,
        };
        let mode = if issue.is_pr {
            WorkItemMode::PullRequests
        } else {
            WorkItemMode::Issues
        };
        self.linked_navigation_origin = Some((issue.number, mode));
    }

    pub fn clear_linked_navigation_origin(&mut self) {
        self.linked_navigation_origin = None;
    }

    pub fn selected_issue_has_known_linked_pr(&self) -> bool {
        let issue = match self.current_or_selected_issue() {
            Some(issue) => issue,
            None => return false,
        };
        if issue.is_pr {
            return false;
        }
        self.linked_pull_request_for_issue(issue.number).is_some()
    }

    pub fn selected_pull_request_has_known_linked_issue(&self) -> bool {
        let issue = match self.current_or_selected_issue() {
            Some(issue) => issue,
            None => return false,
        };
        if !issue.is_pr {
            return false;
        }
        self.linked_issue_for_pull_request(issue.number).is_some()
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

    pub fn set_work_item_mode(&mut self, mode: WorkItemMode) {
        self.work_item_mode = mode;
        self.rebuild_issue_filter();
        self.selected_issue = 0;
        self.issues_preview_scroll = 0;
    }

    pub fn select_issue_by_number(&mut self, issue_number: i64) -> bool {
        let selected = self.filtered_issue_indices.iter().position(|index| {
            self.issues
                .get(*index)
                .is_some_and(|issue| issue.number == issue_number)
        });
        let selected = match selected {
            Some(selected) => selected,
            None => return false,
        };
        self.selected_issue = selected;
        self.issues_preview_scroll = 0;
        true
    }

    pub fn issue_query(&self) -> &str {
        self.issue_query.as_str()
    }

    pub fn issue_search_mode(&self) -> bool {
        self.issue_search_mode
    }

    pub fn help_overlay_visible(&self) -> bool {
        self.help_overlay_visible
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

    pub fn theme_name(&self) -> Option<&str> {
        self.config.theme.as_deref()
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

    pub fn repo_permissions_syncing(&self) -> bool {
        self.repo_permissions_syncing
    }

    pub fn repo_labels_syncing(&self) -> bool {
        self.repo_labels_syncing
    }

    pub fn repo_issue_metadata_editable(&self) -> Option<bool> {
        self.repo_issue_metadata_editable
    }

    pub fn repo_label_color(&self, label: &str) -> Option<&str> {
        let key = label.trim().to_ascii_lowercase();
        self.repo_label_colors.get(&key).map(String::as_str)
    }

    pub fn comment_syncing(&self) -> bool {
        self.comment_syncing
    }

    pub fn pull_request_files_syncing(&self) -> bool {
        self.pull_request_files_syncing
    }

    pub fn pull_request_review_comments_syncing(&self) -> bool {
        self.pull_request_review_comments_syncing
    }

    pub fn pull_request_files(&self) -> &[PullRequestFile] {
        &self.pull_request_files
    }

    pub fn pull_request_id(&self) -> Option<&str> {
        self.pull_request_id.as_deref()
    }

    pub fn pull_request_file_is_viewed(&self, file_path: &str) -> bool {
        self.pull_request_viewed_files.contains(file_path)
    }

    pub fn pull_request_hunk_is_collapsed(&self, file_path: &str, hunk_start: usize) -> bool {
        self.pull_request_collapsed_hunks
            .get(file_path)
            .is_some_and(|collapsed| collapsed.contains(&hunk_start))
    }

    pub fn pull_request_diff_row_hidden(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> bool {
        self.pull_request_diff_row_hidden_for_file(file_path, rows, row_index)
    }

    pub fn pull_request_hunk_hidden_line_count(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        hunk_start: usize,
    ) -> usize {
        if !self.pull_request_hunk_is_collapsed(file_path, hunk_start) {
            return 0;
        }
        let hunk_end = match pull_request_hunk_end(rows, hunk_start) {
            Some(hunk_end) => hunk_end,
            None => return 0,
        };
        hunk_end.saturating_sub(hunk_start)
    }

    pub fn set_pull_request_file_viewed(&mut self, file_path: &str, viewed: bool) {
        if viewed {
            self.pull_request_viewed_files.insert(file_path.to_string());
            return;
        }
        self.pull_request_viewed_files.remove(file_path);
    }

    pub fn selected_pull_request_file_view_toggle(&self) -> Option<(String, bool)> {
        let file = self.selected_pull_request_file_row()?;
        let viewed = self.pull_request_file_is_viewed(file.filename.as_str());
        Some((file.filename.clone(), !viewed))
    }

    pub fn set_pull_request_view_state(
        &mut self,
        pull_request_id: Option<String>,
        viewed_files: HashSet<String>,
    ) {
        self.pull_request_id = pull_request_id;
        self.pull_request_viewed_files = viewed_files;
        self.pull_request_viewed_files.retain(|file_path| {
            self.pull_request_files
                .iter()
                .any(|file| file.filename == *file_path)
        });
    }

    pub fn pull_request_review_comments(&self) -> &[PullRequestReviewComment] {
        &self.pull_request_review_comments
    }

    pub fn pull_request_review_focus(&self) -> PullRequestReviewFocus {
        self.pull_request_review_focus
    }

    pub fn selected_pull_request_file(&self) -> usize {
        self.selected_pull_request_file
    }

    pub fn selected_pull_request_diff_line(&self) -> usize {
        self.selected_pull_request_diff_line
    }

    pub fn pull_request_review_side(&self) -> ReviewSide {
        self.pull_request_review_side
    }

    pub fn pull_request_visual_mode(&self) -> bool {
        self.pull_request_visual_mode
    }

    pub fn pull_request_visual_anchor(&self) -> Option<usize> {
        self.pull_request_visual_anchor
    }

    pub fn pull_request_visual_range(&self) -> Option<(usize, usize)> {
        if !self.pull_request_visual_mode {
            return None;
        }
        Some(self.selected_pull_request_diff_range())
    }

    pub fn selected_pull_request_review_comment_id(&self) -> Option<i64> {
        self.selected_pull_request_review_comment_id
    }

    pub fn pull_request_diff_scroll(&self) -> u16 {
        self.pull_request_diff_scroll
    }

    pub fn pull_request_diff_horizontal_scroll(&self) -> u16 {
        self.pull_request_diff_horizontal_scroll
    }

    pub fn pull_request_diff_horizontal_max(&self) -> u16 {
        self.pull_request_diff_horizontal_max
    }

    pub fn pull_request_diff_expanded(&self) -> bool {
        self.pull_request_diff_expanded
    }

    pub fn selected_pull_request_file_row(&self) -> Option<&PullRequestFile> {
        self.pull_request_files.get(self.selected_pull_request_file)
    }

    pub fn selected_pull_request_diff_rows_len(&self) -> usize {
        let file = match self.selected_pull_request_file_row() {
            Some(file) => file,
            None => return 0,
        };
        parse_patch(file.patch.as_deref()).len()
    }

    pub fn selected_pull_request_review_target(&self) -> Option<PullRequestReviewTarget> {
        let file = self.selected_pull_request_file_row()?;
        let rows = parse_patch(file.patch.as_deref());
        self.review_target_for_rows(file.filename.as_str(), rows.as_slice())
    }

    pub fn pull_request_comments_for_path_and_line(
        &self,
        path: &str,
        side: ReviewSide,
        line: i64,
    ) -> Vec<&PullRequestReviewComment> {
        self.pull_request_review_comments
            .iter()
            .filter(|comment| {
                comment.anchored
                    && comment.path == path
                    && comment.side == side
                    && comment.line == line
            })
            .collect::<Vec<&PullRequestReviewComment>>()
    }

    pub fn pull_request_unanchored_comments_for_path(
        &self,
        path: &str,
    ) -> Vec<&PullRequestReviewComment> {
        self.pull_request_review_comments
            .iter()
            .filter(|comment| !comment.anchored && comment.path == path)
            .collect::<Vec<&PullRequestReviewComment>>()
    }

    pub fn pull_request_comments_count_for_path(&self, path: &str) -> usize {
        self.pull_request_review_comments
            .iter()
            .filter(|comment| comment.path == path)
            .count()
    }

    pub fn selected_pull_request_review_comment(&self) -> Option<&PullRequestReviewComment> {
        let target = self.selected_pull_request_review_target()?;
        let mut comments = self
            .pull_request_review_comments
            .iter()
            .filter(|comment| {
                comment.anchored
                    && comment.path == target.path
                    && comment.side == target.side
                    && comment.line == target.line
            })
            .collect::<Vec<&PullRequestReviewComment>>();
        comments.sort_by_key(|comment| comment.id);

        if let Some(comment_id) = self.selected_pull_request_review_comment_id {
            if let Some(comment) = comments.iter().find(|comment| comment.id == comment_id) {
                return Some(*comment);
            }
        }
        comments.first().copied()
    }

    pub fn update_pull_request_review_comment_body_by_id(&mut self, comment_id: i64, body: &str) {
        for comment in &mut self.pull_request_review_comments {
            if comment.id != comment_id {
                continue;
            }
            comment.body = body.to_string();
            return;
        }
    }

    pub fn remove_pull_request_review_comment_by_id(&mut self, comment_id: i64) {
        self.pull_request_review_comments
            .retain(|comment| comment.id != comment_id);
        if self.selected_pull_request_review_comment_id == Some(comment_id) {
            self.selected_pull_request_review_comment_id = None;
            self.sync_selected_pull_request_review_comment();
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        let key = match self.keybinds.remap_key(key) {
            Some(key) => key,
            None => return,
        };
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
                self.status = "Scanning".to_string();
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('y') {
                self.action = Some(AppAction::CopyStatus);
                return;
            }
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

        if key.code == KeyCode::Char('?') {
            self.help_overlay_visible = !self.help_overlay_visible;
            return;
        }
        if self.help_overlay_visible && key.code == KeyCode::Esc {
            self.help_overlay_visible = false;
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_repo_picker();
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && self.view == View::RepoPicker => {
                self.repo_search_mode = true;
                self.status = "Search repos".to_string();
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.issue_search_mode = true;
                self.status = "Search issues".to_string();
            }
            KeyCode::Tab if key.modifiers.is_empty() && self.view == View::Issues => {
                self.set_issue_filter(self.issue_filter.next());
            }
            KeyCode::BackTab if self.view == View::Issues => {
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
            KeyCode::Char('a')
                if key.modifiers == KeyModifiers::CONTROL && self.view == View::Issues =>
            {
                self.reset_assignee_filter();
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
                self.status = "Syncing".to_string();
            }
            KeyCode::Char('r')
                if key.modifiers.is_empty()
                    && matches!(
                        self.view,
                        View::IssueDetail | View::IssueComments | View::PullRequestFiles
                    ) =>
            {
                self.request_comment_sync();
                self.request_sync();
                if self.current_view_issue_is_pull_request() {
                    self.request_pull_request_files_sync();
                    self.request_pull_request_review_comments_sync();
                }
                self.status = "Syncing issue and comments".to_string();
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
                    && matches!(
                        self.view,
                        View::Issues
                            | View::IssueDetail
                            | View::IssueComments
                            | View::PullRequestFiles
                    ) =>
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
            KeyCode::Char('c')
                if self.view == View::PullRequestFiles
                    && self.pull_request_review_focus == PullRequestReviewFocus::Diff =>
            {
                self.toggle_selected_pull_request_hunk_collapsed();
            }
            KeyCode::Char('c') if self.view == View::IssueDetail => {
                self.reset_issue_comments_scroll();
                self.set_view(View::IssueComments);
            }
            KeyCode::Char('m')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments
                ) =>
            {
                self.action = Some(AppAction::AddIssueComment);
            }
            KeyCode::Char('w') if self.view == View::PullRequestFiles => {
                self.action = Some(AppAction::TogglePullRequestFileViewed);
            }
            KeyCode::Char('m') if self.view == View::PullRequestFiles => {
                self.action = Some(AppAction::AddPullRequestReviewComment);
            }
            KeyCode::Char('e') if self.view == View::PullRequestFiles => {
                self.action = Some(AppAction::EditPullRequestReviewComment);
            }
            KeyCode::Char('x') if self.view == View::PullRequestFiles => {
                self.action = Some(AppAction::DeletePullRequestReviewComment);
            }
            KeyCode::Char('R') if self.view == View::PullRequestFiles => {
                self.action = Some(AppAction::ResolvePullRequestReviewComment);
            }
            KeyCode::Char('n') if self.view == View::PullRequestFiles => {
                self.cycle_pull_request_review_comment(true);
            }
            KeyCode::Char('p') if self.view == View::PullRequestFiles => {
                self.cycle_pull_request_review_comment(false);
            }
            KeyCode::Char('h') if self.view == View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Diff {
                    self.pull_request_review_side = ReviewSide::Left;
                    self.sync_selected_pull_request_review_comment();
                }
            }
            KeyCode::Char('l') if self.view == View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Diff {
                    self.pull_request_review_side = ReviewSide::Right;
                    self.sync_selected_pull_request_review_comment();
                }
            }
            KeyCode::Char('V') if self.view == View::PullRequestFiles => {
                self.toggle_pull_request_visual_mode();
            }
            KeyCode::Char('[') if self.view == View::PullRequestFiles => {
                self.scroll_pull_request_diff_horizontal(-4);
            }
            KeyCode::Char(']') if self.view == View::PullRequestFiles => {
                self.scroll_pull_request_diff_horizontal(4);
            }
            KeyCode::Char('0') if self.view == View::PullRequestFiles => {
                self.reset_pull_request_diff_horizontal_scroll();
            }
            KeyCode::Char('e') if self.view == View::IssueComments => {
                self.action = Some(AppAction::EditIssueComment);
            }
            KeyCode::Char('x') if self.view == View::IssueComments => {
                self.action = Some(AppAction::DeleteIssueComment);
            }
            KeyCode::Char('l')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
            {
                self.action = Some(AppAction::EditLabels);
            }
            KeyCode::Char('A')
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(
                        self.view,
                        View::Issues
                            | View::IssueDetail
                            | View::IssueComments
                            | View::PullRequestFiles
                    ) =>
            {
                self.action = Some(AppAction::EditAssignees);
            }
            KeyCode::Char('u')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
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
                self.back_from_issue_detail();
            }
            KeyCode::Char('b') if self.view == View::IssueComments => {
                self.set_view(View::IssueDetail);
            }
            KeyCode::Char('b') if self.view == View::PullRequestFiles => {
                self.back_from_pull_request_files();
            }
            KeyCode::Esc if self.view == View::IssueDetail => {
                self.back_from_issue_detail();
            }
            KeyCode::Esc if self.view == View::IssueComments => {
                self.set_view(View::IssueDetail);
            }
            KeyCode::Esc if self.view == View::PullRequestFiles => {
                self.back_from_pull_request_files();
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
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
            {
                self.action = Some(AppAction::OpenInBrowser);
            }
            KeyCode::Char('O')
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(
                        self.view,
                        View::Issues
                            | View::IssueDetail
                            | View::IssueComments
                            | View::PullRequestFiles
                    ) =>
            {
                if self
                    .current_or_selected_issue()
                    .is_some_and(|issue| issue.is_pr)
                {
                    self.action = Some(AppAction::OpenLinkedIssueInBrowser);
                    return;
                }
                self.action = Some(AppAction::OpenLinkedPullRequestInBrowser);
            }
            KeyCode::Char('P')
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(
                        self.view,
                        View::Issues
                            | View::IssueDetail
                            | View::IssueComments
                            | View::PullRequestFiles
                    ) =>
            {
                if self
                    .current_or_selected_issue()
                    .is_some_and(|issue| issue.is_pr)
                {
                    self.action = Some(AppAction::OpenLinkedIssueInTui);
                    return;
                }
                self.action = Some(AppAction::OpenLinkedPullRequestInTui);
            }
            KeyCode::Char('v')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
            {
                self.action = Some(AppAction::CheckoutPullRequest);
            }
            _ => {}
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent) {
        let target = self.mouse_target_at(event.column, event.row);
        match event.kind {
            MouseEventKind::ScrollUp => {
                self.handle_mouse_scroll(target, false);
            }
            MouseEventKind::ScrollDown => {
                self.handle_mouse_scroll(target, true);
            }
            MouseEventKind::ScrollLeft => {
                self.handle_mouse_scroll_horizontal(target, false);
            }
            MouseEventKind::ScrollRight => {
                self.handle_mouse_scroll_horizontal(target, true);
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                self.handle_mouse_click_target(target);
            }
            _ => {}
        }
    }

    fn handle_mouse_scroll(&mut self, target: Option<MouseTarget>, down: bool) {
        let Some(target) = target else {
            return;
        };
        if matches!(target, MouseTarget::RepoListPane | MouseTarget::RepoRow(_)) {
            if self.view == View::RepoPicker {
                if down {
                    self.move_selection_down();
                    return;
                }
                self.move_selection_up();
            }
            return;
        }
        if matches!(
            target,
            MouseTarget::RemoteListPane | MouseTarget::RemoteRow(_)
        ) {
            if self.view == View::RemoteChooser {
                if down {
                    self.move_selection_down();
                    return;
                }
                self.move_selection_up();
            }
            return;
        }
        if matches!(
            target,
            MouseTarget::IssuesListPane | MouseTarget::IssueRow(_)
        ) {
            self.focus = Focus::IssuesList;
        }
        if matches!(target, MouseTarget::IssuesPreviewPane) {
            self.focus = Focus::IssuesPreview;
        }
        if matches!(target, MouseTarget::IssueBodyPane) {
            self.focus = Focus::IssueBody;
        }
        if matches!(target, MouseTarget::IssueSidePane) {
            self.focus = Focus::IssueRecentComments;
        }
        if matches!(
            target,
            MouseTarget::PullRequestFilesPane | MouseTarget::PullRequestFileRow(_)
        ) {
            self.set_pull_request_review_focus(PullRequestReviewFocus::Files);
        }
        if matches!(
            target,
            MouseTarget::PullRequestDiffPane | MouseTarget::PullRequestDiffRow(_, _)
        ) {
            self.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        }

        if down {
            self.move_selection_down();
            return;
        }
        self.move_selection_up();
    }

    fn handle_mouse_scroll_horizontal(&mut self, target: Option<MouseTarget>, right: bool) {
        if !matches!(
            target,
            Some(MouseTarget::PullRequestDiffPane | MouseTarget::PullRequestDiffRow(_, _))
        ) {
            return;
        }
        self.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        if right {
            self.scroll_pull_request_diff_horizontal(4);
            return;
        }
        self.scroll_pull_request_diff_horizontal(-4);
    }

    fn handle_mouse_click_target(&mut self, target: Option<MouseTarget>) {
        match target {
            Some(MouseTarget::Back) => {
                if self.view == View::IssueDetail {
                    self.back_from_issue_detail();
                    return;
                }
                if self.view == View::IssueComments {
                    self.set_view(View::IssueDetail);
                    return;
                }
                if self.view == View::PullRequestFiles {
                    self.back_from_pull_request_files();
                    return;
                }
                if matches!(self.view, View::LabelPicker | View::AssigneePicker) {
                    self.set_view(self.editor_cancel_view);
                    return;
                }
                if self.view == View::CommentPresetPicker {
                    self.set_view(View::Issues);
                }
            }
            Some(MouseTarget::RepoPicker) => {
                self.open_repo_picker();
            }
            Some(MouseTarget::RepoRow(index)) => {
                self.selected_repo = index.min(self.filtered_repo_indices.len().saturating_sub(1));
                self.action = Some(AppAction::PickRepo);
            }
            Some(MouseTarget::RepoListPane) => {}
            Some(MouseTarget::RemoteRow(index)) => {
                self.selected_remote = index.min(self.remotes.len().saturating_sub(1));
                self.action = Some(AppAction::PickRemote);
            }
            Some(MouseTarget::RemoteListPane) => {}
            Some(MouseTarget::IssueTabOpen) => {
                self.set_issue_filter(IssueFilter::Open);
            }
            Some(MouseTarget::IssueTabClosed) => {
                self.set_issue_filter(IssueFilter::Closed);
            }
            Some(MouseTarget::IssuesListPane) => {
                self.focus = Focus::IssuesList;
            }
            Some(MouseTarget::IssuesPreviewPane) => {
                self.focus = Focus::IssuesPreview;
            }
            Some(MouseTarget::IssueRow(index)) => {
                self.focus = Focus::IssuesList;
                self.selected_issue =
                    index.min(self.filtered_issue_indices.len().saturating_sub(1));
                self.issues_preview_scroll = 0;
                self.action = Some(AppAction::PickIssue);
            }
            Some(MouseTarget::IssueBodyPane) => {
                self.focus = Focus::IssueBody;
            }
            Some(MouseTarget::IssueSidePane) => {
                self.focus = Focus::IssueRecentComments;
                if self.current_issue_row().is_some_and(|issue| issue.is_pr) {
                    self.set_view(View::PullRequestFiles);
                    return;
                }
                self.reset_issue_comments_scroll();
                self.set_view(View::IssueComments);
            }
            Some(MouseTarget::LinkedPullRequestTuiButton) => {
                self.focus = Focus::IssuesPreview;
                self.action = Some(AppAction::OpenLinkedPullRequestInTui);
            }
            Some(MouseTarget::LinkedPullRequestWebButton) => {
                self.focus = Focus::IssuesPreview;
                self.action = Some(AppAction::OpenLinkedPullRequestInBrowser);
            }
            Some(MouseTarget::LinkedIssueTuiButton) => {
                self.focus = Focus::IssuesPreview;
                self.action = Some(AppAction::OpenLinkedIssueInTui);
            }
            Some(MouseTarget::LinkedIssueWebButton) => {
                self.focus = Focus::IssuesPreview;
                self.action = Some(AppAction::OpenLinkedIssueInBrowser);
            }
            Some(MouseTarget::CommentsPane) => {}
            Some(MouseTarget::CommentRow(index)) => {
                self.selected_comment = index.min(self.comments.len().saturating_sub(1));
            }
            Some(MouseTarget::PullRequestFocusFiles) | Some(MouseTarget::PullRequestFilesPane) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Files);
            }
            Some(MouseTarget::PullRequestFocusDiff) | Some(MouseTarget::PullRequestDiffPane) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
            }
            Some(MouseTarget::PullRequestFileRow(index)) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Files);
                self.selected_pull_request_file =
                    index.min(self.pull_request_files.len().saturating_sub(1));
                self.selected_pull_request_diff_line = 0;
                self.pull_request_diff_scroll = 0;
                self.pull_request_diff_horizontal_scroll = 0;
                self.pull_request_diff_horizontal_max = 0;
                self.pull_request_diff_expanded = false;
                self.pull_request_visual_mode = false;
                self.pull_request_visual_anchor = None;
                self.sync_selected_pull_request_review_comment();
            }
            Some(MouseTarget::PullRequestDiffRow(index, side)) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
                self.pull_request_review_side = side;
                self.selected_pull_request_diff_line = index;
                self.sync_selected_pull_request_review_comment();
            }
            Some(MouseTarget::LabelOption(index)) => {
                if let Some(filtered_index) = self.filtered_label_indices().get(index).copied() {
                    self.selected_label_option = filtered_index;
                    self.toggle_selected_label();
                }
            }
            Some(MouseTarget::LabelApply) => {
                self.action = Some(AppAction::SubmitLabels);
            }
            Some(MouseTarget::LabelCancel) => {
                self.set_view(self.editor_cancel_view);
            }
            Some(MouseTarget::AssigneeOption(index)) => {
                if let Some(filtered_index) = self.filtered_assignee_indices().get(index).copied() {
                    self.selected_assignee_option = filtered_index;
                    self.toggle_selected_assignee();
                }
            }
            Some(MouseTarget::AssigneeApply) => {
                self.action = Some(AppAction::SubmitAssignees);
            }
            Some(MouseTarget::AssigneeCancel) => {
                self.set_view(self.editor_cancel_view);
            }
            Some(MouseTarget::PresetOption(index)) => {
                self.preset_choice = index.min(self.preset_items_len().saturating_sub(1));
                self.action = Some(AppAction::PickPreset);
            }
            None => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn open_repo_picker(&mut self) {
        self.repo_query.clear();
        self.repo_search_mode = false;
        self.rebuild_repo_picker_filter();
        self.set_view(View::RepoPicker);
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
        self.help_overlay_visible = false;
        if self.view != View::PullRequestFiles {
            self.pull_request_diff_expanded = false;
        }
        match self.view {
            View::Issues => self.focus = Focus::IssuesList,
            View::IssueDetail => self.focus = Focus::IssueBody,
            View::PullRequestFiles => {
                self.pull_request_review_focus = PullRequestReviewFocus::Files;
            }
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
                self.filtered_issue_indices.iter().position(|index| {
                    self.issues
                        .get(*index)
                        .is_some_and(|issue| issue.number == number)
                })
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
            .and_then(|comment_id| {
                self.comments
                    .iter()
                    .position(|comment| comment.id == comment_id)
            })
            .unwrap_or(0);
        self.issue_comments_scroll = 0;
        self.issue_recent_comments_scroll = 0;
        self.issue_comments_max_scroll = 0;
        self.issue_recent_comments_max_scroll = 0;
    }

    pub fn set_pull_request_files(&mut self, issue_id: i64, files: Vec<PullRequestFile>) {
        self.pull_request_files_issue_id = Some(issue_id);
        self.pull_request_id = None;
        self.pull_request_files = files;
        let mut active_file_paths = HashSet::new();
        for file in &self.pull_request_files {
            active_file_paths.insert(file.filename.clone());
        }
        self.pull_request_viewed_files.clear();
        self.pull_request_collapsed_hunks
            .retain(|file_path, _| active_file_paths.contains(file_path));
        self.selected_pull_request_file = 0;
        self.selected_pull_request_diff_line = 0;
        self.pull_request_diff_scroll = 0;
        self.pull_request_diff_horizontal_scroll = 0;
        self.pull_request_diff_max_scroll = 0;
        self.pull_request_diff_horizontal_max = 0;
        self.pull_request_diff_expanded = false;
        self.pull_request_review_focus = PullRequestReviewFocus::Files;
        self.pull_request_visual_mode = false;
        self.pull_request_visual_anchor = None;
        self.selected_pull_request_review_comment_id = None;
    }

    pub fn set_pull_request_review_comments(
        &mut self,
        mut comments: Vec<PullRequestReviewComment>,
    ) {
        comments.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line.cmp(&right.line))
                .then(left.id.cmp(&right.id))
        });
        self.pull_request_review_comments = comments;
        self.selected_pull_request_review_comment_id = self
            .selected_pull_request_review_comment()
            .map(|comment| comment.id);
    }

    pub fn set_pull_request_review_focus(&mut self, focus: PullRequestReviewFocus) {
        self.pull_request_review_focus = focus;
        if focus == PullRequestReviewFocus::Files {
            self.pull_request_diff_expanded = false;
            self.pull_request_visual_mode = false;
            self.pull_request_visual_anchor = None;
        }
        if focus == PullRequestReviewFocus::Diff {
            let selected_file = self
                .selected_pull_request_file_row()
                .map(|file| (file.filename.clone(), file.patch.clone()));
            if let Some((file_path, patch)) = selected_file {
                let rows = parse_patch(patch.as_deref());
                self.selected_pull_request_diff_line = self.nearest_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    self.selected_pull_request_diff_line,
                );
            }
        }
        self.sync_selected_pull_request_review_comment();
    }

    pub fn set_pull_request_diff_max_scroll(&mut self, max_scroll: u16) {
        self.pull_request_diff_max_scroll = max_scroll;
        if self.pull_request_diff_scroll > max_scroll {
            self.pull_request_diff_scroll = max_scroll;
        }
    }

    pub fn set_pull_request_diff_scroll(&mut self, scroll: u16) {
        self.pull_request_diff_scroll = scroll.min(self.pull_request_diff_max_scroll);
    }

    pub fn reset_pull_request_diff_horizontal_scroll(&mut self) {
        self.pull_request_diff_horizontal_scroll = 0;
    }

    pub fn set_pull_request_diff_horizontal_max(&mut self, max_scroll: u16) {
        self.pull_request_diff_horizontal_max = max_scroll;
        if self.pull_request_diff_horizontal_scroll > max_scroll {
            self.pull_request_diff_horizontal_scroll = max_scroll;
        }
    }

    fn toggle_pull_request_diff_expanded(&mut self) {
        if self.view != View::PullRequestFiles
            || self.pull_request_review_focus != PullRequestReviewFocus::Diff
        {
            return;
        }
        self.pull_request_diff_expanded = !self.pull_request_diff_expanded;
        if self.pull_request_diff_expanded {
            self.status = "Expanded diff view".to_string();
            return;
        }
        self.status = "Split files and diff view".to_string();
    }

    fn back_from_pull_request_files(&mut self) {
        if self.pull_request_diff_expanded {
            self.pull_request_diff_expanded = false;
            self.status = "Split files and diff view".to_string();
            return;
        }
        self.set_view(View::IssueDetail);
    }

    fn back_from_issue_detail(&mut self) {
        if self.restore_linked_navigation_origin() {
            return;
        }
        self.set_view(View::Issues);
    }

    fn restore_linked_navigation_origin(&mut self) -> bool {
        let (issue_number, mode) = match self.linked_navigation_origin {
            Some(origin) => origin,
            None => return false,
        };
        self.linked_navigation_origin = None;

        self.set_view(View::Issues);
        self.set_work_item_mode(mode);
        let try_filters = [IssueFilter::Open, IssueFilter::Closed];
        for filter in try_filters {
            self.set_issue_filter(filter);
            if !self.select_issue_by_number(issue_number) {
                continue;
            }
            self.status = format!("Returned to #{}", issue_number);
            return true;
        }

        self.status = format!("Could not return to #{}", issue_number);
        false
    }

    fn scroll_pull_request_diff_horizontal(&mut self, delta: i16) {
        if self.view != View::PullRequestFiles
            || self.pull_request_review_focus != PullRequestReviewFocus::Diff
        {
            return;
        }
        let amount = delta.unsigned_abs();
        if delta.is_negative() {
            self.pull_request_diff_horizontal_scroll = self
                .pull_request_diff_horizontal_scroll
                .saturating_sub(amount);
            return;
        }
        self.pull_request_diff_horizontal_scroll = self
            .pull_request_diff_horizontal_scroll
            .saturating_add(amount)
            .min(self.pull_request_diff_horizontal_max);
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

    pub fn set_repo_permissions_syncing(&mut self, syncing: bool) {
        self.repo_permissions_syncing = syncing;
    }

    pub fn set_repo_labels_syncing(&mut self, syncing: bool) {
        self.repo_labels_syncing = syncing;
    }

    pub fn set_repo_issue_metadata_editable(&mut self, editable: Option<bool>) {
        self.repo_issue_metadata_editable = editable;
    }

    pub fn set_comment_syncing(&mut self, syncing: bool) {
        self.comment_syncing = syncing;
    }

    pub fn set_pull_request_files_syncing(&mut self, syncing: bool) {
        self.pull_request_files_syncing = syncing;
    }

    pub fn set_pull_request_review_comments_syncing(&mut self, syncing: bool) {
        self.pull_request_review_comments_syncing = syncing;
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

    pub fn request_pull_request_review_comments_sync(&mut self) {
        self.pull_request_review_comments_sync_requested = true;
    }

    pub fn take_pull_request_review_comments_sync_request(&mut self) -> bool {
        let requested = self.pull_request_review_comments_sync_requested;
        self.pull_request_review_comments_sync_requested = false;
        requested
    }

    pub fn request_sync(&mut self) {
        self.sync_requested = true;
    }

    pub fn request_repo_permissions_sync(&mut self) {
        self.repo_permissions_sync_requested = true;
    }

    pub fn take_repo_permissions_sync_request(&mut self) -> bool {
        let requested = self.repo_permissions_sync_requested;
        self.repo_permissions_sync_requested = false;
        requested
    }

    pub fn request_repo_labels_sync(&mut self) {
        self.repo_labels_sync_requested = true;
    }

    pub fn take_repo_labels_sync_request(&mut self) -> bool {
        let requested = self.repo_labels_sync_requested;
        self.repo_labels_sync_requested = false;
        requested
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
        self.repo_permissions_syncing = false;
        self.repo_permissions_sync_requested = true;
        self.repo_issue_metadata_editable = None;
        self.repo_labels_syncing = false;
        self.repo_labels_sync_requested = true;
        self.repo_label_colors.clear();
        self.linked_pull_requests.clear();
        self.linked_issues.clear();
        self.linked_pull_request_lookups.clear();
        self.linked_issue_lookups.clear();
        self.linked_navigation_origin = None;
        self.pull_request_files_issue_id = None;
        self.pull_request_id = None;
        self.pull_request_files.clear();
        self.pull_request_viewed_files.clear();
        self.pull_request_collapsed_hunks.clear();
        self.pull_request_review_comments.clear();
        self.selected_pull_request_file = 0;
        self.selected_pull_request_diff_line = 0;
        self.pull_request_diff_scroll = 0;
        self.pull_request_diff_horizontal_scroll = 0;
        self.pull_request_diff_max_scroll = 0;
        self.pull_request_diff_horizontal_max = 0;
        self.pull_request_diff_expanded = false;
        self.pull_request_review_focus = PullRequestReviewFocus::Files;
        self.pull_request_review_side = ReviewSide::Right;
        self.pull_request_visual_mode = false;
        self.pull_request_visual_anchor = None;
        self.selected_pull_request_review_comment_id = None;
        self.editing_pull_request_review_comment_id = None;
        self.pending_review_target = None;
        self.repo_search_mode = false;
        self.assignee_filter = AssigneeFilter::All;
        self.work_item_mode = WorkItemMode::Issues;
        self.issue_query.clear();
        self.issue_search_mode = false;
    }

    pub fn set_current_issue(&mut self, issue_id: i64, issue_number: i64) {
        self.current_issue_id = Some(issue_id);
        self.current_issue_number = Some(issue_number);
        self.pending_review_target = None;
        if self.pull_request_files_issue_id != Some(issue_id) {
            self.pull_request_files_issue_id = None;
            self.pull_request_id = None;
            self.pull_request_files.clear();
            self.pull_request_viewed_files.clear();
            self.pull_request_collapsed_hunks.clear();
            self.pull_request_review_comments.clear();
            self.selected_pull_request_file = 0;
            self.selected_pull_request_diff_line = 0;
            self.pull_request_diff_scroll = 0;
            self.pull_request_diff_horizontal_scroll = 0;
            self.pull_request_diff_max_scroll = 0;
            self.pull_request_diff_horizontal_max = 0;
            self.pull_request_diff_expanded = false;
            self.pull_request_review_focus = PullRequestReviewFocus::Files;
            self.pull_request_review_side = ReviewSide::Right;
            self.pull_request_visual_mode = false;
            self.pull_request_visual_anchor = None;
            self.selected_pull_request_review_comment_id = None;
            self.editing_pull_request_review_comment_id = None;
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
        self.editing_pull_request_review_comment_id = None;
        self.pending_review_target = None;
        self.comment_editor.reset_for_comment();
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_comment_edit_editor(&mut self, return_view: View, comment_id: i64, body: &str) {
        self.editing_comment_id = Some(comment_id);
        self.editing_pull_request_review_comment_id = None;
        self.pending_review_target = None;
        self.comment_editor.reset_for_comment_edit(body);
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_pull_request_review_comment_editor(
        &mut self,
        return_view: View,
        target: PullRequestReviewTarget,
    ) {
        self.editing_pull_request_review_comment_id = None;
        self.pending_review_target = Some(target);
        self.comment_editor.reset_for_pull_request_review_comment();
        self.editor_cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_pull_request_review_comment_edit_editor(
        &mut self,
        return_view: View,
        comment_id: i64,
        body: &str,
    ) {
        self.editing_pull_request_review_comment_id = Some(comment_id);
        self.pending_review_target = None;
        self.comment_editor
            .reset_for_pull_request_review_comment_edit(body);
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

    pub fn merge_repo_label_colors(&mut self, labels: Vec<(String, String)>) {
        for (name, color) in labels {
            let key = name.trim().to_ascii_lowercase();
            let value = color.trim().trim_start_matches('#').to_string();
            if key.is_empty() || value.len() != 6 {
                continue;
            }
            self.repo_label_colors.insert(key, value);
        }
    }

    pub fn merge_assignee_options(&mut self, assignees: Vec<String>) {
        let mut merged = self.assignee_options.clone();
        for assignee in assignees {
            if assignee.trim().is_empty() {
                continue;
            }
            if merged
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(assignee.as_str()))
            {
                continue;
            }
            merged.push(assignee);
        }
        merged.sort_by_key(|value| value.to_ascii_lowercase());
        self.assignee_options = merged;
        if let Some(index) = self.filtered_assignee_indices().first() {
            self.selected_assignee_option = *index;
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

    pub fn take_pending_review_target(&mut self) -> Option<PullRequestReviewTarget> {
        self.pending_review_target.take()
    }

    pub fn take_editing_pull_request_review_comment_id(&mut self) -> Option<i64> {
        self.editing_pull_request_review_comment_id.take()
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

    pub fn clear_mouse_regions(&mut self) {
        self.mouse_regions.clear();
    }

    pub fn register_mouse_region(
        &mut self,
        target: MouseTarget,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        self.mouse_regions.push(MouseRegion {
            target,
            x,
            y,
            width,
            height,
        });
    }

    fn mouse_target_at(&self, column: u16, row: u16) -> Option<MouseTarget> {
        let mut index = self.mouse_regions.len();
        while index > 0 {
            index -= 1;
            let region = self.mouse_regions[index];
            if column < region.x || row < region.y {
                continue;
            }
            if column >= region.x.saturating_add(region.width) {
                continue;
            }
            if row >= region.y.saturating_add(region.height) {
                continue;
            }
            return Some(region.target);
        }
        None
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
            View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Files {
                    if self.selected_pull_request_file > 0 {
                        self.selected_pull_request_file -= 1;
                        self.selected_pull_request_diff_line = 0;
                        self.pull_request_diff_scroll = 0;
                        self.pull_request_diff_horizontal_scroll = 0;
                        self.pull_request_diff_horizontal_max = 0;
                        self.pull_request_diff_expanded = false;
                        self.pull_request_visual_mode = false;
                        self.pull_request_visual_anchor = None;
                    }
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                let selected_file = self
                    .selected_pull_request_file_row()
                    .map(|file| (file.filename.clone(), file.patch.clone()));
                let (file_path, patch) = match selected_file {
                    Some(selected_file) => selected_file,
                    None => {
                        self.sync_selected_pull_request_review_comment();
                        return;
                    }
                };
                let rows = parse_patch(patch.as_deref());
                if rows.is_empty() {
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                let current = self.nearest_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    self.selected_pull_request_diff_line,
                );
                self.selected_pull_request_diff_line = current;
                if let Some(previous) = self.previous_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    current,
                ) {
                    self.selected_pull_request_diff_line = previous;
                }
                self.sync_selected_pull_request_review_comment();
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
            View::CommentPresetName | View::CommentEditor => {}
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
            View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Files {
                    if self.selected_pull_request_file + 1 < self.pull_request_files.len() {
                        self.selected_pull_request_file += 1;
                        self.selected_pull_request_diff_line = 0;
                        self.pull_request_diff_scroll = 0;
                        self.pull_request_diff_horizontal_scroll = 0;
                        self.pull_request_diff_horizontal_max = 0;
                        self.pull_request_diff_expanded = false;
                        self.pull_request_visual_mode = false;
                        self.pull_request_visual_anchor = None;
                    }
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                let selected_file = self
                    .selected_pull_request_file_row()
                    .map(|file| (file.filename.clone(), file.patch.clone()));
                let (file_path, patch) = match selected_file {
                    Some(selected_file) => selected_file,
                    None => {
                        self.sync_selected_pull_request_review_comment();
                        return;
                    }
                };
                let rows = parse_patch(patch.as_deref());
                if rows.is_empty() {
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                let current = self.nearest_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    self.selected_pull_request_diff_line,
                );
                self.selected_pull_request_diff_line = current;
                if let Some(next) = self.next_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    current,
                ) {
                    self.selected_pull_request_diff_line = next;
                }
                self.sync_selected_pull_request_review_comment();
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
                if self.focus == Focus::IssueBody {
                    if self.current_view_issue_is_pull_request() {
                        self.reset_issue_comments_scroll();
                        self.set_view(View::IssueComments);
                        return;
                    }
                    self.status =
                        "Focus comments pane (Ctrl+l), then press Enter to open full comments"
                            .to_string();
                    return;
                }
                if self.current_view_issue_is_pull_request()
                    && self.focus == Focus::IssueRecentComments
                {
                    self.set_view(View::PullRequestFiles);
                    return;
                }
                self.reset_issue_comments_scroll();
                self.set_view(View::IssueComments);
            }
            View::IssueComments => {}
            View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Files {
                    self.pull_request_review_focus = PullRequestReviewFocus::Diff;
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                self.toggle_pull_request_diff_expanded();
            }
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
            View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Files {
                    self.selected_pull_request_file = 0;
                    self.selected_pull_request_diff_line = 0;
                    self.pull_request_diff_scroll = 0;
                    self.pull_request_diff_horizontal_scroll = 0;
                    self.pull_request_diff_horizontal_max = 0;
                    self.pull_request_diff_expanded = false;
                    self.pull_request_visual_mode = false;
                    self.pull_request_visual_anchor = None;
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                self.selected_pull_request_diff_line = 0;
                self.pull_request_diff_scroll = 0;
                self.pull_request_diff_horizontal_scroll = 0;
                self.pull_request_diff_horizontal_max = 0;
                self.pull_request_diff_expanded = false;
                self.sync_selected_pull_request_review_comment();
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
            View::CommentPresetName | View::CommentEditor => {}
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
            View::PullRequestFiles => {
                if self.pull_request_review_focus == PullRequestReviewFocus::Files {
                    if !self.pull_request_files.is_empty() {
                        self.selected_pull_request_file = self.pull_request_files.len() - 1;
                        self.selected_pull_request_diff_line = 0;
                        self.pull_request_diff_scroll = 0;
                        self.pull_request_diff_horizontal_scroll = 0;
                        self.pull_request_diff_horizontal_max = 0;
                        self.pull_request_diff_expanded = false;
                        self.pull_request_visual_mode = false;
                        self.pull_request_visual_anchor = None;
                    }
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                let selected_file = self
                    .selected_pull_request_file_row()
                    .map(|file| (file.filename.clone(), file.patch.clone()));
                if let Some((file_path, patch)) = selected_file {
                    let rows = parse_patch(patch.as_deref());
                    if let Some(last_visible) = self
                        .last_visible_pull_request_diff_line(file_path.as_str(), rows.as_slice())
                    {
                        self.selected_pull_request_diff_line = last_visible;
                    }
                    self.pull_request_diff_scroll = self.pull_request_diff_max_scroll;
                }
                self.sync_selected_pull_request_review_comment();
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
        if offsets.is_empty() || self.selected_comment + 1 >= offsets.len() {
            return;
        }
        self.selected_comment += 1;
        self.issue_comments_scroll =
            offsets[self.selected_comment].min(self.issue_comments_max_scroll);
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

    fn pull_request_diff_row_hidden_for_file(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> bool {
        if row_index >= rows.len() {
            return false;
        }
        let collapsed_hunks = match self.pull_request_collapsed_hunks.get(file_path) {
            Some(collapsed_hunks) => collapsed_hunks,
            None => return false,
        };
        for hunk_start in collapsed_hunks {
            let hunk_end = match pull_request_hunk_end(rows, *hunk_start) {
                Some(hunk_end) => hunk_end,
                None => continue,
            };
            if row_index > *hunk_start && row_index <= hunk_end {
                return true;
            }
        }
        false
    }

    fn nearest_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> usize {
        if rows.is_empty() {
            return 0;
        }
        let row_index = row_index.min(rows.len() - 1);
        if !self.pull_request_diff_row_hidden_for_file(file_path, rows, row_index) {
            return row_index;
        }
        let hunk_range = match pull_request_hunk_range_for_row(rows, row_index) {
            Some(hunk_range) => hunk_range,
            None => return row_index,
        };
        hunk_range.start
    }

    fn next_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> Option<usize> {
        if rows.is_empty() {
            return None;
        }
        let mut index = row_index.min(rows.len() - 1).saturating_add(1);
        while index < rows.len() {
            if !self.pull_request_diff_row_hidden_for_file(file_path, rows, index) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn previous_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> Option<usize> {
        if rows.is_empty() {
            return None;
        }
        let mut index = row_index.min(rows.len() - 1);
        while index > 0 {
            index -= 1;
            if !self.pull_request_diff_row_hidden_for_file(file_path, rows, index) {
                return Some(index);
            }
        }
        None
    }

    fn last_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
    ) -> Option<usize> {
        if rows.is_empty() {
            return None;
        }
        let mut index = rows.len();
        while index > 0 {
            index -= 1;
            if !self.pull_request_diff_row_hidden_for_file(file_path, rows, index) {
                return Some(index);
            }
        }
        None
    }

    fn toggle_selected_pull_request_hunk_collapsed(&mut self) {
        if self.pull_request_review_focus != PullRequestReviewFocus::Diff {
            self.status = "Focus the diff pane to collapse sections".to_string();
            return;
        }

        let selected_file = match self.selected_pull_request_file_row() {
            Some(file) => (file.filename.clone(), file.patch.clone()),
            None => {
                self.status = "No file selected".to_string();
                return;
            }
        };
        let file_path = selected_file.0;
        let rows = parse_patch(selected_file.1.as_deref());
        if rows.is_empty() {
            self.status = "No diff section to collapse".to_string();
            return;
        }

        let selected_line = self
            .selected_pull_request_diff_line
            .min(rows.len().saturating_sub(1));
        let hunk_range = match pull_request_hunk_range_for_row(rows.as_slice(), selected_line) {
            Some(hunk_range) => hunk_range,
            None => {
                self.status = "No hunk at this line".to_string();
                return;
            }
        };

        let mut collapsed = true;
        let mut remove_entry = false;
        {
            let collapsed_hunks = self
                .pull_request_collapsed_hunks
                .entry(file_path.clone())
                .or_default();
            if !collapsed_hunks.insert(hunk_range.start) {
                collapsed_hunks.remove(&hunk_range.start);
                collapsed = false;
            }
            if collapsed_hunks.is_empty() {
                remove_entry = true;
            }
        }
        if remove_entry {
            self.pull_request_collapsed_hunks.remove(file_path.as_str());
        }

        self.selected_pull_request_diff_line = hunk_range.start;
        self.pull_request_visual_mode = false;
        self.pull_request_visual_anchor = None;
        self.sync_selected_pull_request_review_comment();

        if collapsed {
            let hidden_lines = hunk_range.end.saturating_sub(hunk_range.start);
            self.status = format!("Collapsed {} lines in {}", hidden_lines, file_path);
            return;
        }
        self.status = format!("Expanded section in {}", file_path);
    }

    fn toggle_pull_request_visual_mode(&mut self) {
        if self.pull_request_review_focus != PullRequestReviewFocus::Diff {
            self.pull_request_review_focus = PullRequestReviewFocus::Diff;
        }
        if self.pull_request_visual_mode {
            self.pull_request_visual_mode = false;
            self.pull_request_visual_anchor = None;
            self.sync_selected_pull_request_review_comment();
            return;
        }
        self.pull_request_visual_mode = true;
        self.pull_request_visual_anchor = Some(self.selected_pull_request_diff_line);
        self.sync_selected_pull_request_review_comment();
    }

    fn selected_pull_request_diff_range(&self) -> (usize, usize) {
        if !self.pull_request_visual_mode {
            return (
                self.selected_pull_request_diff_line,
                self.selected_pull_request_diff_line,
            );
        }
        let anchor = self
            .pull_request_visual_anchor
            .unwrap_or(self.selected_pull_request_diff_line);
        let start = anchor.min(self.selected_pull_request_diff_line);
        let end = anchor.max(self.selected_pull_request_diff_line);
        (start, end)
    }

    fn review_target_for_rows(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
    ) -> Option<PullRequestReviewTarget> {
        if rows.is_empty() {
            return None;
        }
        let (start_index, end_index) = self.selected_pull_request_diff_range();
        let start_index = start_index.min(rows.len() - 1);
        let end_index = end_index.min(rows.len() - 1);

        let side = self.pull_request_review_side;
        let mut selected_lines = Vec::new();
        for row in &rows[start_index..=end_index] {
            let line = match side {
                ReviewSide::Left => row.old_line,
                ReviewSide::Right => row.new_line,
            };
            if line.is_none() {
                continue;
            }
            selected_lines.push(line.unwrap_or_default());
        }

        if selected_lines.is_empty() {
            let row = rows.get(self.selected_pull_request_diff_line)?;
            match row.kind {
                DiffKind::Added | DiffKind::Context => {
                    return Some(PullRequestReviewTarget {
                        path: file_path.to_string(),
                        line: row.new_line?,
                        side: ReviewSide::Right,
                        start_line: None,
                        start_side: None,
                    });
                }
                DiffKind::Removed => {
                    return Some(PullRequestReviewTarget {
                        path: file_path.to_string(),
                        line: row.old_line?,
                        side: ReviewSide::Left,
                        start_line: None,
                        start_side: None,
                    });
                }
                _ => return None,
            }
        }

        let line = *selected_lines.last().unwrap_or(&0);
        let start_line = if selected_lines.len() > 1 {
            selected_lines.first().copied()
        } else {
            None
        };

        Some(PullRequestReviewTarget {
            path: file_path.to_string(),
            line,
            side,
            start_line,
            start_side: start_line.map(|_| side),
        })
    }

    fn cycle_pull_request_review_comment(&mut self, forward: bool) {
        let target = match self.selected_pull_request_review_target() {
            Some(target) => target,
            None => return,
        };
        let mut ids = self
            .pull_request_review_comments
            .iter()
            .filter(|comment| {
                comment.anchored
                    && comment.path == target.path
                    && comment.side == target.side
                    && comment.line == target.line
            })
            .map(|comment| comment.id)
            .collect::<Vec<i64>>();
        ids.sort_unstable();
        if ids.is_empty() {
            self.selected_pull_request_review_comment_id = None;
            return;
        }
        let current_index = self
            .selected_pull_request_review_comment_id
            .and_then(|id| ids.iter().position(|value| *value == id))
            .unwrap_or(0);
        let next_index = if forward {
            (current_index + 1) % ids.len()
        } else if current_index == 0 {
            ids.len() - 1
        } else {
            current_index - 1
        };
        self.selected_pull_request_review_comment_id = Some(ids[next_index]);
    }

    fn sync_selected_pull_request_review_comment(&mut self) {
        let comment_id = self
            .selected_pull_request_review_comment()
            .map(|comment| comment.id);
        self.selected_pull_request_review_comment_id = comment_id;
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
                return value
                    .parse::<i64>()
                    .ok()
                    .is_some_and(|parsed| issue.number == parsed);
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

    fn reset_assignee_filter(&mut self) {
        self.assignee_filter = AssigneeFilter::All;
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
            View::CommentEditor => match key.code {
                KeyCode::Esc => {
                    self.editing_comment_id = None;
                    self.editing_pull_request_review_comment_id = None;
                    self.pending_review_target = None;
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
                    EditorMode::AddPullRequestReviewComment => {
                        self.action = Some(AppAction::SubmitPullRequestReviewComment);
                    }
                    EditorMode::EditPullRequestReviewComment => {
                        self.action = Some(AppAction::SubmitEditedPullRequestReviewComment);
                    }
                    EditorMode::AddPreset => {
                        self.action = Some(AppAction::SavePreset);
                    }
                },
                KeyCode::Backspace => self.comment_editor.backspace_text(),
                KeyCode::Char(ch) => self.comment_editor.append_text(ch),
                _ => {}
            },
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
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
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
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
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
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
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
            View::PullRequestFiles => match code {
                KeyCode::Char('h') | KeyCode::Char('k') => {
                    self.pull_request_review_focus = PullRequestReviewFocus::Files;
                    self.pull_request_visual_mode = false;
                    self.pull_request_visual_anchor = None;
                    self.sync_selected_pull_request_review_comment();
                    true
                }
                KeyCode::Char('l') | KeyCode::Char('j') => {
                    self.pull_request_review_focus = PullRequestReviewFocus::Diff;
                    self.sync_selected_pull_request_review_comment();
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
mod tests {
    use super::{
        App, AppAction, Focus, IssueFilter, MouseTarget, PullRequestFile, PullRequestReviewFocus,
        PullRequestReviewTarget, ReviewSide, View, WorkItemMode,
    };
    use crate::config::Config;
    use crate::store::{CommentRow, IssueRow, LocalRepoRow};
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

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
    fn enter_on_pr_changes_pane_opens_full_pr_changes_view() {
        let mut app = App::new(Config::default());
        app.set_issues(vec![IssueRow {
            id: 43,
            repo_id: 1,
            number: 8,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        }]);
        app.set_current_issue(43, 8);
        app.set_view(View::IssueDetail);

        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.view(), View::PullRequestFiles);
    }

    #[test]
    fn enter_on_issue_side_pane_opens_comments_view() {
        let mut app = App::new(Config::default());
        app.set_issues(vec![IssueRow {
            id: 44,
            repo_id: 1,
            number: 9,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);
        app.set_current_issue(44, 9);
        app.set_view(View::IssueDetail);

        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.view(), View::IssueComments);
    }

    #[test]
    fn enter_on_pr_body_opens_comments_view() {
        let mut app = App::new(Config::default());
        app.set_issues(vec![IssueRow {
            id: 45,
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
        app.set_current_issue(45, 10);
        app.set_view(View::IssueDetail);

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.view(), View::IssueComments);
    }

    #[test]
    fn tab_cycles_issue_filter() {
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

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.issue_filter(), IssueFilter::Closed);
        assert_eq!(app.issues_for_view().len(), 1);
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(2));

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
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
    fn select_issue_by_number_finds_item_in_filtered_mode() {
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
                number: 22,
                state: "closed".to_string(),
                title: "PR".to_string(),
                body: String::new(),
                labels: String::new(),
                assignees: String::new(),
                comments_count: 0,
                updated_at: None,
                is_pr: true,
            },
        ]);

        app.set_work_item_mode(WorkItemMode::PullRequests);
        app.set_issue_filter(IssueFilter::Closed);

        assert!(app.select_issue_by_number(22));
        assert_eq!(app.selected_issue_row().map(|issue| issue.number), Some(22));
        assert!(!app.select_issue_by_number(11));
    }

    #[test]
    fn selected_issue_has_known_linked_pr_when_cached() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 10,
            repo_id: 1,
            number: 55,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);

        assert!(!app.selected_issue_has_known_linked_pr());
        app.set_linked_pull_request(55, Some(99));
        assert!(app.selected_issue_has_known_linked_pr());
    }

    #[test]
    fn v_triggers_checkout_pull_request_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));

        assert_eq!(app.take_action(), Some(AppAction::CheckoutPullRequest));
    }

    #[test]
    fn shift_o_triggers_open_linked_pull_request_in_browser_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT));

        assert_eq!(
            app.take_action(),
            Some(AppAction::OpenLinkedPullRequestInBrowser)
        );
    }

    #[test]
    fn shift_p_triggers_open_linked_pull_request_in_tui_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.on_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT));

        assert_eq!(
            app.take_action(),
            Some(AppAction::OpenLinkedPullRequestInTui)
        );
    }

    #[test]
    fn shift_o_on_selected_pr_triggers_open_linked_issue_in_browser_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_work_item_mode(WorkItemMode::PullRequests);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 42,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT));

        assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInBrowser));
    }

    #[test]
    fn shift_p_on_selected_pr_triggers_open_linked_issue_in_tui_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_work_item_mode(WorkItemMode::PullRequests);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 42,
            state: "open".to_string(),
            title: "PR".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: true,
        }]);

        app.on_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT));

        assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInTui));
    }

    #[test]
    fn ctrl_y_triggers_copy_status_action() {
        let mut app = App::new(Config::default());

        app.on_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));

        assert_eq!(app.take_action(), Some(AppAction::CopyStatus));
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
    fn ctrl_a_resets_assignee_filter_to_all() {
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
        ]);

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.assignee_filter_label(), "alex");

        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert_eq!(app.assignee_filter_label(), "all");
        assert_eq!(app.issues_for_view().len(), 2);
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
        assert_eq!(
            app.selected_issue_row().map(|issue| issue.number),
            Some(101)
        );

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
        assert_eq!(
            app.selected_issue_row().map(|issue| issue.number),
            Some(777)
        );
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
    fn m_triggers_pull_request_review_comment_in_review_view() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,1 +1,2 @@\n-old\n+new\n+more".to_string()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));

        assert_eq!(
            app.take_action(),
            Some(AppAction::AddPullRequestReviewComment)
        );
        let target = app
            .selected_pull_request_review_target()
            .expect("review target");
        assert_eq!(target.path, "src/main.rs");
        assert_eq!(target.line, 1);
        assert_eq!(target.side, ReviewSide::Right);
    }

    #[test]
    fn review_comment_editor_submit_action_is_emitted() {
        let mut app = App::new(Config::default());
        app.open_pull_request_review_comment_editor(
            View::PullRequestFiles,
            PullRequestReviewTarget {
                path: "src/main.rs".to_string(),
                line: 10,
                side: ReviewSide::Right,
                start_line: None,
                start_side: None,
            },
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            app.take_action(),
            Some(AppAction::SubmitPullRequestReviewComment)
        );
    }

    #[test]
    fn shift_r_triggers_resolve_review_comment_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);

        app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));

        assert_eq!(
            app.take_action(),
            Some(AppAction::ResolvePullRequestReviewComment)
        );
    }

    #[test]
    fn w_emits_toggle_pull_request_file_viewed_action() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            }],
        );

        app.on_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        assert_eq!(
            app.take_action(),
            Some(AppAction::TogglePullRequestFileViewed)
        );
    }

    #[test]
    fn custom_quit_keybinding_remaps_and_disables_default() {
        let mut config = Config::default();
        config
            .keybinds
            .insert("quit".to_string(), "ctrl+q".to_string());
        let mut app = App::new(config);

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.should_quit());

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit());
    }

    #[test]
    fn diff_horizontal_scroll_uses_keyboard_and_mouse() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        app.set_pull_request_diff_horizontal_max(20);
        app.register_mouse_region(MouseTarget::PullRequestDiffPane, 0, 0, 120, 40);

        app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert_eq!(app.pull_request_diff_horizontal_scroll(), 4);

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::ScrollRight,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.pull_request_diff_horizontal_scroll(), 8);

        app.on_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
        assert_eq!(app.pull_request_diff_horizontal_scroll(), 0);
    }

    #[test]
    fn mouse_back_click_navigates_to_previous_view() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.register_mouse_region(MouseTarget::Back, 0, 0, 12, 3);

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.view(), View::IssueDetail);
    }

    #[test]
    fn mouse_click_repo_picker_region_opens_repo_picker() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.register_mouse_region(MouseTarget::RepoPicker, 0, 0, 8, 1);

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.view(), View::RepoPicker);
    }

    #[test]
    fn mouse_click_issue_row_selects_and_opens_issue() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.set_issues(vec![IssueRow {
            id: 1,
            repo_id: 1,
            number: 12,
            state: "open".to_string(),
            title: "Issue".to_string(),
            body: String::new(),
            labels: String::new(),
            assignees: String::new(),
            comments_count: 0,
            updated_at: None,
            is_pr: false,
        }]);
        app.register_mouse_region(MouseTarget::IssueRow(0), 0, 0, 50, 2);

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.take_action(), Some(AppAction::PickIssue));
    }

    #[test]
    fn mouse_click_linked_pr_buttons_trigger_actions() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);

        app.register_mouse_region(MouseTarget::LinkedPullRequestTuiButton, 0, 0, 16, 1);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            app.take_action(),
            Some(AppAction::OpenLinkedPullRequestInTui)
        );

        app.register_mouse_region(MouseTarget::LinkedPullRequestWebButton, 0, 1, 10, 1);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            app.take_action(),
            Some(AppAction::OpenLinkedPullRequestInBrowser)
        );

        app.register_mouse_region(MouseTarget::LinkedIssueTuiButton, 0, 2, 16, 1);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInTui));

        app.register_mouse_region(MouseTarget::LinkedIssueWebButton, 0, 3, 10, 1);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 3,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.take_action(), Some(AppAction::OpenLinkedIssueInBrowser));
    }

    #[test]
    fn mouse_click_pr_file_row_selects_file() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![
                PullRequestFile {
                    filename: "src/a.rs".to_string(),
                    status: "modified".to_string(),
                    additions: 1,
                    deletions: 0,
                    patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
                },
                PullRequestFile {
                    filename: "src/b.rs".to_string(),
                    status: "modified".to_string(),
                    additions: 1,
                    deletions: 0,
                    patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
                },
            ],
        );
        app.register_mouse_region(MouseTarget::PullRequestFileRow(1), 0, 0, 50, 1);

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.selected_pull_request_file(), 1);
    }

    #[test]
    fn mouse_click_diff_row_sets_side_and_line() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 2,
                deletions: 1,
                patch: Some("@@ -1,1 +1,2 @@\n-old\n+new\n+more".to_string()),
            }],
        );
        app.register_mouse_region(
            MouseTarget::PullRequestDiffRow(2, ReviewSide::Left),
            0,
            0,
            50,
            1,
        );

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(
            app.pull_request_review_focus(),
            PullRequestReviewFocus::Diff
        );
        assert_eq!(app.pull_request_review_side(), ReviewSide::Left);
        assert_eq!(app.selected_pull_request_diff_line(), 2);
    }

    #[test]
    fn selected_pull_request_file_view_toggle_flips_current_state() {
        let mut app = App::new(Config::default());
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            }],
        );

        let (path, viewed) = app
            .selected_pull_request_file_view_toggle()
            .expect("toggle payload");
        assert_eq!(path, "src/main.rs");
        assert!(viewed);

        app.set_pull_request_file_viewed("src/main.rs", true);
        let (_, viewed) = app
            .selected_pull_request_file_view_toggle()
            .expect("toggle payload");
        assert!(!viewed);
    }

    #[test]
    fn c_collapses_selected_hunk_and_navigation_skips_hidden_rows() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 4,
                deletions: 2,
                patch: Some(
                    "@@ -1,1 +1,4 @@\n old\n+new-a\n+new-b\n+new-c\n@@ -10,1 +10,1 @@\n-old-two\n+new-two"
                        .to_string(),
                ),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.selected_pull_request_diff_line(), 2);

        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        assert_eq!(app.selected_pull_request_diff_line(), 0);
        assert!(app.pull_request_hunk_is_collapsed("src/main.rs", 0));

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.selected_pull_request_diff_line(), 5);

        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        assert!(!app.pull_request_hunk_is_collapsed("src/main.rs", 0));
    }

    #[test]
    fn enter_toggles_pull_request_diff_expanded_mode() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        assert!(!app.pull_request_diff_expanded());

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.pull_request_diff_expanded());

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.pull_request_diff_expanded());
    }

    #[test]
    fn question_mark_toggles_help_overlay() {
        let mut app = App::new(Config::default());
        assert!(!app.help_overlay_visible());

        app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
        assert!(app.help_overlay_visible());

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.help_overlay_visible());
    }

    #[test]
    fn visual_mode_creates_multiline_review_target() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 2,
                deletions: 0,
                patch: Some("@@ -1,1 +1,3 @@\n old\n+new\n+more".to_string()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT));
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));

        let target = app.selected_pull_request_review_target().expect("target");
        assert_eq!(target.side, ReviewSide::Right);
        assert_eq!(target.start_line, Some(1));
        assert_eq!(target.line, 2);
    }

    #[test]
    fn l_sets_review_side_to_new_on_context_row() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,2 +1,2 @@\n old\n-old2\n+new2".to_string()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        let right_target = app
            .selected_pull_request_review_target()
            .expect("right target");
        assert_eq!(right_target.side, ReviewSide::Right);
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
        assert_eq!(
            app.selected_labels(),
            vec!["bug".to_string(), "docs".to_string()]
        );
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
            vec![
                "bug".to_string(),
                "customer".to_string(),
                "docs".to_string(),
            ],
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

    #[test]
    fn merge_assignee_options_dedupes_case_insensitive() {
        let mut app = App::new(Config::default());
        app.open_assignee_picker(
            View::Issues,
            vec!["alex".to_string(), "Sam".to_string()],
            "",
        );

        app.merge_assignee_options(vec![
            "sam".to_string(),
            "jordan".to_string(),
            "ALEX".to_string(),
        ]);

        assert_eq!(app.assignee_options(), &["alex", "jordan", "Sam"]);
    }

    #[test]
    fn back_from_expanded_diff_returns_to_split_review() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            vec![PullRequestFile {
                filename: "src/main.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.pull_request_diff_expanded());

        app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(app.view(), View::PullRequestFiles);
        assert!(!app.pull_request_diff_expanded());
    }

    #[test]
    fn linked_issue_none_does_not_clear_cached_link() {
        let mut app = App::new(Config::default());
        app.set_linked_issue_for_pull_request(42, Some(7));
        app.set_linked_issue_for_pull_request(42, None);

        assert_eq!(app.linked_issue_for_pull_request(42), Some(7));
    }

    #[test]
    fn linked_pull_request_none_does_not_clear_cached_link() {
        let mut app = App::new(Config::default());
        app.set_linked_pull_request(7, Some(42));
        app.set_linked_pull_request(7, None);

        assert_eq!(app.linked_pull_request_for_issue(7), Some(42));
    }
}
