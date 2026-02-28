use super::*;
use std::time::{Duration, Instant};

impl App {
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn set_repos(&mut self, repos: Vec<LocalRepoRow>) {
        self.repos = repos;
        self.rebuild_repo_picker_filter();
        if self.navigation.selected_repo >= self.search.filtered_repo_indices.len() {
            self.navigation.selected_repo =
                self.search.filtered_repo_indices.len().saturating_sub(1);
        }
    }

    pub fn set_remotes(&mut self, remotes: Vec<RemoteInfo>) {
        self.remotes = remotes;
        self.navigation.selected_remote = 0;
    }

    pub fn set_issues(&mut self, issues: Vec<IssueRow>) {
        let selected_issue_number = self.selected_issue_row().map(|issue| issue.number);
        let current_issue_number = self.context.issue_number;
        self.issues = issues;
        self.rebuild_issue_filter();
        self.navigation.selected_issue = selected_issue_number
            .and_then(|number| {
                self.search.filtered_issue_indices.iter().position(|index| {
                    self.issues
                        .get(*index)
                        .is_some_and(|issue| issue.number == number)
                })
            })
            .unwrap_or(0);
        if let Some(number) = current_issue_number
            && let Some(issue) = self.issues.iter().find(|issue| issue.number == number)
        {
            self.context.issue_id = Some(issue.id);
        }
        self.navigation.issues_preview_scroll = 0;
        self.navigation.issues_preview_max_scroll = 0;
    }

    pub fn set_comments(&mut self, comments: Vec<CommentRow>) {
        let selected_comment_id = self.selected_comment_row().map(|comment| comment.id);
        self.comments = comments;
        if self.comments.is_empty() {
            self.navigation.selected_comment = 0;
            self.navigation.issue_comments_scroll = 0;
            self.navigation.issue_recent_comments_scroll = 0;
            self.navigation.issue_comments_max_scroll = 0;
            self.navigation.issue_recent_comments_max_scroll = 0;
            return;
        }
        self.navigation.selected_comment = selected_comment_id
            .and_then(|comment_id| {
                self.comments
                    .iter()
                    .position(|comment| comment.id == comment_id)
            })
            .unwrap_or(0);
        self.navigation.issue_comments_scroll = 0;
        self.navigation.issue_recent_comments_scroll = 0;
        self.navigation.issue_comments_max_scroll = 0;
        self.navigation.issue_recent_comments_max_scroll = 0;
    }

    pub(super) fn back_from_issue_detail(&mut self) {
        if self.restore_linked_navigation_origin() {
            return;
        }
        self.set_view(View::Issues);
    }

    pub fn reset_issue_detail_scroll(&mut self) {
        self.navigation.issue_detail_scroll = 0;
    }

    pub fn set_issue_detail_max_scroll(&mut self, max_scroll: u16) {
        self.navigation.issue_detail_max_scroll = max_scroll;
        if self.navigation.issue_detail_scroll > max_scroll {
            self.navigation.issue_detail_scroll = max_scroll;
        }
    }

    pub fn set_issues_preview_max_scroll(&mut self, max_scroll: u16) {
        self.navigation.issues_preview_max_scroll = max_scroll;
        if self.navigation.issues_preview_scroll > max_scroll {
            self.navigation.issues_preview_scroll = max_scroll;
        }
    }

    pub fn reset_issue_comments_scroll(&mut self) {
        self.navigation.issue_comments_scroll = 0;
    }

    pub fn set_issue_comments_max_scroll(&mut self, max_scroll: u16) {
        self.navigation.issue_comments_max_scroll = max_scroll;
        if self.navigation.issue_comments_scroll > max_scroll {
            self.navigation.issue_comments_scroll = max_scroll;
        }
    }

    pub fn set_issue_recent_comments_max_scroll(&mut self, max_scroll: u16) {
        self.navigation.issue_recent_comments_max_scroll = max_scroll;
        if self.navigation.issue_recent_comments_scroll > max_scroll {
            self.navigation.issue_recent_comments_scroll = max_scroll;
        }
    }

    pub fn add_comment_default(&mut self, preset: CommentDefault) {
        self.config.comment_defaults.push(preset);
        self.preset.choice = 0;
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.save()
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.status_expires_at = None;
    }

    pub fn set_transient_status(&mut self, status: impl Into<String>, duration: Duration) {
        self.status = status.into();
        if self.status.is_empty() {
            self.status_expires_at = None;
            return;
        }
        self.status_expires_at = Some(Instant::now() + duration);
    }

    pub fn clear_status_if_expired(&mut self) {
        self.clear_status_if_expired_at(Instant::now());
    }

    pub fn clear_status_if_expired_at(&mut self, now: Instant) {
        let expires_at = match self.status_expires_at {
            Some(expires_at) => expires_at,
            None => return,
        };
        if now < expires_at {
            return;
        }
        self.status.clear();
        self.status_expires_at = None;
    }

    pub fn set_scanning(&mut self, scanning: bool) {
        self.sync.scanning = scanning;
    }

    pub fn set_syncing(&mut self, syncing: bool) {
        self.sync.syncing = syncing;
    }

    pub fn set_repo_permissions_syncing(&mut self, syncing: bool) {
        self.sync.repo_permissions_syncing = syncing;
    }

    pub fn set_repo_labels_syncing(&mut self, syncing: bool) {
        self.sync.repo_labels_syncing = syncing;
    }

    pub fn set_repo_issue_metadata_editable(&mut self, editable: Option<bool>) {
        self.sync.repo_issue_metadata_editable = editable;
    }

    pub fn set_comment_syncing(&mut self, syncing: bool) {
        self.sync.comment_syncing = syncing;
    }

    pub fn set_pull_request_files_syncing(&mut self, syncing: bool) {
        self.sync.pull_request_files_syncing = syncing;
    }

    pub fn set_pull_request_review_comments_syncing(&mut self, syncing: bool) {
        self.sync.pull_request_review_comments_syncing = syncing;
    }

    pub fn request_comment_sync(&mut self) {
        self.sync.comment_sync_requested = true;
    }

    pub fn take_comment_sync_request(&mut self) -> bool {
        let requested = self.sync.comment_sync_requested;
        self.sync.comment_sync_requested = false;
        requested
    }

    pub fn request_pull_request_files_sync(&mut self) {
        self.sync.pull_request_files_sync_requested = true;
    }

    pub fn take_pull_request_files_sync_request(&mut self) -> bool {
        let requested = self.sync.pull_request_files_sync_requested;
        self.sync.pull_request_files_sync_requested = false;
        requested
    }

    pub fn request_pull_request_review_comments_sync(&mut self) {
        self.sync.pull_request_review_comments_sync_requested = true;
    }

    pub fn take_pull_request_review_comments_sync_request(&mut self) -> bool {
        let requested = self.sync.pull_request_review_comments_sync_requested;
        self.sync.pull_request_review_comments_sync_requested = false;
        requested
    }

    pub fn request_sync(&mut self) {
        self.sync.sync_requested = true;
    }

    pub fn request_repo_permissions_sync(&mut self) {
        self.sync.repo_permissions_sync_requested = true;
    }

    pub fn take_repo_permissions_sync_request(&mut self) -> bool {
        let requested = self.sync.repo_permissions_sync_requested;
        self.sync.repo_permissions_sync_requested = false;
        requested
    }

    pub fn request_repo_labels_sync(&mut self) {
        self.sync.repo_labels_sync_requested = true;
    }

    pub fn take_repo_labels_sync_request(&mut self) -> bool {
        let requested = self.sync.repo_labels_sync_requested;
        self.sync.repo_labels_sync_requested = false;
        requested
    }

    pub fn take_sync_request(&mut self) -> bool {
        let requested = self.sync.sync_requested;
        self.sync.sync_requested = false;
        requested
    }

    pub fn set_current_repo_with_path(&mut self, owner: &str, repo: &str, path: Option<&str>) {
        self.context.owner = Some(owner.to_string());
        self.context.repo = Some(repo.to_string());
        self.context.path = path.map(ToString::to_string);
        self.context.issue_id = None;
        self.context.issue_number = None;
        self.sync.repo_permissions_syncing = false;
        self.sync.repo_permissions_sync_requested = true;
        self.sync.repo_issue_metadata_editable = None;
        self.sync.repo_labels_syncing = false;
        self.sync.repo_labels_sync_requested = true;
        self.repo_label_colors.clear();
        self.linked.pull_requests.clear();
        self.linked.issues.clear();
        self.linked.pull_request_lookups.clear();
        self.linked.issue_lookups.clear();
        self.linked.navigation_origin = None;
        self.clear_linked_picker_state();
        self.reset_pull_request_state();
        self.search.repo_search_mode = false;
        self.assignee_filter = AssigneeFilter::All;
        self.work_item_mode = WorkItemMode::Issues;
        self.search.issue_query.clear();
        self.search.issue_search_mode = false;
    }

    pub fn set_current_issue(&mut self, issue_id: i64, issue_number: i64) {
        self.context.issue_id = Some(issue_id);
        self.context.issue_number = Some(issue_number);
        if self.pull_request.pull_request_files_issue_id != Some(issue_id) {
            self.reset_pull_request_state();
        }
    }

    pub fn update_issue_state_by_number(&mut self, issue_number: i64, state: &str) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.state = state.to_string();
            }
        }
        self.rebuild_issue_filter();
        if self.navigation.selected_issue >= self.search.filtered_issue_indices.len() {
            self.navigation.selected_issue =
                self.search.filtered_issue_indices.len().saturating_sub(1);
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
            self.navigation.selected_comment = 0;
            self.navigation.issue_comments_scroll = 0;
            return;
        }

        if self.navigation.selected_comment >= self.comments.len() {
            self.navigation.selected_comment = self.comments.len() - 1;
            return;
        }
        if removed_index <= self.navigation.selected_comment && self.navigation.selected_comment > 0
        {
            self.navigation.selected_comment -= 1;
        }
    }

    pub fn current_issue_id(&self) -> Option<i64> {
        self.context.issue_id
    }

    pub fn current_issue_number(&self) -> Option<i64> {
        self.context.issue_number
    }

    pub fn set_pending_issue_action(&mut self, issue_number: i64, action: PendingIssueAction) {
        self.interaction
            .pending_issue_actions
            .insert(issue_number, action);
    }

    pub fn clear_pending_issue_action(&mut self, issue_number: i64) {
        self.interaction.pending_issue_actions.remove(&issue_number);
    }

    pub fn pending_issue_badge(&self, issue_number: i64) -> Option<&'static str> {
        self.interaction
            .pending_issue_actions
            .get(&issue_number)
            .copied()
            .map(PendingIssueAction::label)
    }

    pub fn take_rescan_request(&mut self) -> bool {
        let requested = self.sync.rescan_requested;
        self.sync.rescan_requested = false;
        requested
    }

    pub fn take_action(&mut self) -> Option<AppAction> {
        self.interaction.action.take()
    }

    pub(super) fn current_view_issue_is_closed(&self) -> bool {
        if self.view == View::Issues {
            return self
                .selected_issue_row()
                .is_some_and(|issue| issue_state_is_closed(issue.state.as_str()));
        }

        self.current_issue_row()
            .is_some_and(|issue| issue_state_is_closed(issue.state.as_str()))
    }

    pub(super) fn current_view_issue_is_pull_request(&self) -> bool {
        if self.view == View::Issues {
            return self.selected_issue_row().is_some_and(|issue| issue.is_pr);
        }
        self.current_issue_row().is_some_and(|issue| issue.is_pr)
    }
}
