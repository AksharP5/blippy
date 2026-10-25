use super::*;
use std::time::{Duration, Instant};

fn repository_key(owner: &str, repo: &str) -> String {
    let mut key = format!("{owner}/{repo}");
    key.make_ascii_lowercase();
    key
}

impl App {
    fn resolved_repository_key(&self, owner: &str, repo: &str) -> String {
        let key = repository_key(owner, repo);
        self.context.aliases.get(&key).cloned().unwrap_or(key)
    }

    fn current_repository_request_key(&self) -> Option<String> {
        Some(repository_key(
            self.context.owner.as_deref()?,
            self.context.repo.as_deref()?,
        ))
    }

    pub(super) fn repository_key_is_current(&self, key: &str) -> bool {
        let resolved = self
            .context
            .aliases
            .get(key)
            .map(String::as_str)
            .unwrap_or(key);
        self.context.key.as_deref() == Some(resolved)
    }

    pub fn repository_is_current(&self, owner: &str, repo: &str) -> bool {
        self.repository_key_is_current(&repository_key(owner, repo))
    }

    pub fn repository_request_is_current(&self, owner: &str, repo: &str, id: Option<i64>) -> bool {
        self.repository_request_key_is_current(&repository_key(owner, repo), id)
    }

    pub(super) fn repository_request_key_is_current(&self, key: &str, id: Option<i64>) -> bool {
        match id {
            Some(requested) => self.context.id == Some(requested),
            None => self.repository_key_is_current(key),
        }
    }

    pub fn clear_repository_requests(&mut self, repo_id: i64) {
        self.interaction.pending_issue_actions.retain(|_, actions| {
            actions.retain(|_, (id, _)| *id != Some(repo_id));
            !actions.is_empty()
        });
        self.sync
            .repo_permissions_syncing
            .retain(|_, id| *id != Some(repo_id));
        self.sync
            .repo_labels_syncing
            .retain(|_, id| *id != Some(repo_id));
    }

    pub fn set_current_repo_id(&mut self, id: i64) {
        self.context.id = Some(id);
        if let Some(key) = self.current_repository_request_key() {
            // A cached repository owns this name; an earlier redirect may now be obsolete.
            self.context.aliases.remove(&key);
            self.context.key = Some(key);
        }
    }

    pub fn resolve_repository_alias(
        &mut self,
        requested_owner: &str,
        requested_repo: &str,
        resolved: &RepoRow,
    ) -> bool {
        let requested_key = repository_key(requested_owner, requested_repo);
        let selected_key = self.current_repository_request_key();
        let old_key = self.resolved_repository_key(requested_owner, requested_repo);
        let key = repository_key(&resolved.owner, &resolved.name);
        let current = self.context.id.map_or_else(
            || {
                selected_key
                    .as_ref()
                    .is_some_and(|selected| selected == &requested_key || selected == &key)
            },
            |id| id == resolved.id,
        );
        if selected_key.as_ref() == Some(&requested_key)
            && self.context.id.is_some_and(|id| id != resolved.id)
        {
            return false;
        }
        if old_key != key {
            for target in self.context.aliases.values_mut() {
                if *target == old_key {
                    *target = key.clone();
                }
            }
            self.context.aliases.insert(old_key, key.clone());
        }
        if requested_key != key {
            self.context.aliases.insert(requested_key, key.clone());
        }
        self.context.aliases.retain(|alias, target| alias != target);
        if !current {
            return false;
        }
        if self.current_repository_request_key().as_ref() != Some(&key) {
            self.linked.pull_requests.clear();
            self.linked.issues.clear();
        }
        self.context.id = Some(resolved.id);
        self.context.owner = Some(resolved.owner.clone());
        self.context.repo = Some(resolved.name.clone());
        self.context.key = Some(key);
        true
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn set_repos(&mut self, repos: Vec<LocalRepoRow>) {
        let selected_repo = self
            .search
            .filtered_repo_indices
            .get(self.navigation.selected_repo)
            .and_then(|index| self.repos.get(*index))
            .map(|repo| (repo.path.clone(), repo.remote_name.clone()));
        self.repos = repos;
        self.rebuild_repo_picker_filter();
        self.navigation.selected_repo = selected_repo
            .and_then(|(path, remote_name)| {
                self.search.filtered_repo_indices.iter().position(|index| {
                    let repo = &self.repos[*index];
                    repo.path == path && repo.remote_name == remote_name
                })
            })
            .unwrap_or_else(|| {
                self.navigation
                    .selected_repo
                    .min(self.search.filtered_repo_indices.len().saturating_sub(1))
            });
    }

    pub fn set_remotes(&mut self, remotes: Vec<RemoteInfo>) {
        self.remotes = remotes;
        self.navigation.selected_remote = 0;
    }

    pub fn set_issues(&mut self, issues: Vec<IssueRow>) {
        let selected_issue_id = self.selected_issue_row().map(|issue| issue.id);
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
        if self.selected_issue_row().map(|issue| issue.id) != selected_issue_id {
            self.navigation.issues_preview_scroll = 0;
            self.navigation.issues_preview_max_scroll = 0;
        }
    }

    pub fn set_comments(&mut self, comments: Vec<CommentRow>) {
        self.preserve_issue_comment_reading_position();
        let selected_comment_id = self.selected_comment_row().map(|comment| comment.id);
        self.comments = comments;
        let selected_comment = selected_comment_id.and_then(|comment_id| {
            self.comments
                .iter()
                .position(|comment| comment.id == comment_id)
        });
        if selected_comment != Some(self.navigation.selected_comment) {
            self.navigation.issue_comment_offsets.clear();
        }
        self.navigation.selected_comment = selected_comment.unwrap_or(0);
        if selected_comment.is_none() {
            self.navigation.issue_comment_reading_position = None;
            self.navigation.issue_comments_scroll = 0;
            self.navigation.issue_recent_comments_scroll = 0;
            self.navigation.issue_comments_max_scroll = 0;
            self.navigation.issue_recent_comments_max_scroll = 0;
        }
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
        self.navigation.issue_comment_reading_position = None;
    }

    pub fn set_issue_comments_max_scroll(&mut self, max_scroll: u16) {
        self.preserve_issue_comment_reading_position();
        self.navigation.issue_comments_max_scroll = max_scroll;
        if self.navigation.issue_comments_scroll > max_scroll {
            self.navigation.issue_comments_scroll = max_scroll;
        }
    }

    pub fn set_issue_comment_offsets(&mut self, offsets: Vec<u16>) {
        let selected = self.navigation.selected_comment;
        let previous = self.navigation.issue_comment_offsets.get(selected).copied();
        let current = offsets.get(selected).copied();
        let relative_offset = self
            .navigation
            .issue_comment_reading_position
            .take()
            .filter(|(id, _)| {
                self.selected_comment_row()
                    .is_some_and(|comment| comment.id == *id)
            })
            .map(|(_, offset)| offset)
            .unwrap_or_default();
        if previous != current {
            self.navigation.issue_comments_scroll = (i32::from(current.unwrap_or_default())
                + relative_offset)
                .clamp(0, i32::from(self.navigation.issue_comments_max_scroll))
                as u16;
        }
        self.navigation.issue_comment_offsets = offsets;
    }

    fn preserve_issue_comment_reading_position(&mut self) {
        if self.navigation.issue_comment_reading_position.is_some() {
            return;
        }
        if let Some(comment) = self.selected_comment_row()
            && let Some(offset) = self
                .navigation
                .issue_comment_offsets
                .get(self.navigation.selected_comment)
        {
            self.navigation.issue_comment_reading_position = Some((
                comment.id,
                i32::from(self.navigation.issue_comments_scroll) - i32::from(*offset),
            ));
        }
    }

    pub fn set_issue_recent_comments_max_scroll(&mut self, max_scroll: u16) {
        self.navigation.issue_recent_comments_max_scroll = max_scroll;
        if self.navigation.issue_recent_comments_scroll > max_scroll {
            self.navigation.issue_recent_comments_scroll = max_scroll;
        }
    }

    pub fn save_comment_default(&mut self, preset: CommentDefault) -> Result<()> {
        self.config.save_comment_default(preset)?;
        self.preset.choice = 0;
        Ok(())
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

    pub fn clear_status_if_expired(&mut self) -> bool {
        self.clear_status_if_expired_at(Instant::now())
    }

    pub fn clear_status_if_expired_at(&mut self, now: Instant) -> bool {
        let expires_at = match self.status_expires_at {
            Some(expires_at) => expires_at,
            None => return false,
        };
        if now < expires_at {
            return false;
        }
        self.status.clear();
        self.status_expires_at = None;
        true
    }

    pub fn set_scanning(&mut self, scanning: bool) {
        self.sync.scanning = scanning;
    }

    pub fn begin_repo_sync(&mut self) {
        if let Some(key) = self.current_repository_request_key() {
            self.sync.syncing.insert(key);
        }
    }

    pub fn finish_repo_sync(&mut self, owner: &str, repo: &str) {
        self.sync.syncing.remove(&repository_key(owner, repo));
    }

    pub fn begin_repo_permissions_sync(&mut self) {
        if let Some(key) = self.current_repository_request_key() {
            self.sync
                .repo_permissions_syncing
                .insert(key, self.context.id);
        }
    }

    pub fn finish_repo_permissions_sync(&mut self, owner: &str, repo: &str, id: Option<i64>) {
        let key = repository_key(owner, repo);
        if self.sync.repo_permissions_syncing.get(&key) == Some(&id) {
            self.sync.repo_permissions_syncing.remove(&key);
        }
    }

    pub fn begin_repo_labels_sync(&mut self) {
        if let Some(key) = self.current_repository_request_key() {
            self.sync.repo_labels_syncing.insert(key, self.context.id);
        }
    }

    pub fn finish_repo_labels_sync(&mut self, owner: &str, repo: &str, id: Option<i64>) {
        let key = repository_key(owner, repo);
        if self.sync.repo_labels_syncing.get(&key) == Some(&id) {
            self.sync.repo_labels_syncing.remove(&key);
        }
    }

    pub fn set_repo_issue_metadata_editable(&mut self, editable: Option<bool>) {
        self.sync.repo_issue_metadata_editable = editable;
    }

    pub fn set_repo_pull_request_mergeable(&mut self, mergeable: Option<bool>) {
        self.sync.repo_pull_request_mergeable = mergeable;
    }

    pub fn begin_comment_sync(&mut self) {
        if let Some(id) = self.context.issue_id {
            self.sync.comment_syncing.insert(id);
        }
    }

    pub fn finish_comment_sync(&mut self, issue_id: i64) {
        self.sync.comment_syncing.remove(&issue_id);
    }

    pub fn begin_pull_request_files_sync(&mut self) {
        if let Some(id) = self.context.issue_id {
            self.sync.pull_request_files_syncing.insert(id);
        }
    }

    pub fn finish_pull_request_files_sync(&mut self, issue_id: i64) {
        self.sync.pull_request_files_syncing.remove(&issue_id);
    }

    pub fn begin_pull_request_review_comments_sync(&mut self) {
        if let Some(id) = self.context.issue_id {
            self.sync.pull_request_review_comments_syncing.insert(id);
        }
    }

    pub fn finish_pull_request_review_comments_sync(&mut self, issue_id: i64) {
        self.sync
            .pull_request_review_comments_syncing
            .remove(&issue_id);
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
        self.context.id = None;
        self.context.owner = Some(owner.to_string());
        self.context.repo = Some(repo.to_string());
        self.context.key = Some(self.resolved_repository_key(owner, repo));
        self.context.path = path.map(ToString::to_string);
        self.context.issue_id = None;
        self.context.issue_number = None;
        self.reset_issue_sync_state();
        self.sync.repo_permissions_sync_requested = true;
        self.sync.repo_issue_metadata_editable = None;
        self.sync.repo_pull_request_mergeable = None;
        self.sync.repo_labels_sync_requested = true;
        self.repo_label_colors.clear();
        self.linked.pull_requests.clear();
        self.linked.issues.clear();
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
        let issue_changed = self.context.issue_id != Some(issue_id);
        self.context.issue_id = Some(issue_id);
        self.context.issue_number = Some(issue_number);
        if issue_changed {
            self.reset_issue_sync_state();
            self.reset_pull_request_state();
        }
    }

    fn reset_issue_sync_state(&mut self) {
        self.navigation.issue_comment_offsets.clear();
        self.navigation.issue_comment_reading_position = None;
        self.sync.comment_sync_requested = false;
        self.sync.pull_request_files_sync_requested = false;
        self.sync.pull_request_review_comments_sync_requested = false;
    }

    pub fn update_issue_state_by_number(&mut self, issue_number: i64, state: &str) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.state = state.to_string();
            }
        }
        self.refresh_issue_filter();
    }

    pub fn update_issue_labels_by_number(&mut self, issue_number: i64, labels: &[String]) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.labels = labels.to_vec();
            }
        }
        self.refresh_issue_filter();
    }

    pub fn update_issue_assignees_by_number(&mut self, issue_number: i64, assignees: &str) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.assignees = assignees.to_string();
            }
        }
        self.refresh_issue_filter();
    }

    fn refresh_issue_filter(&mut self) {
        let selected_issue_id = self.selected_issue_row().map(|issue| issue.id);
        self.rebuild_issue_filter();
        if let Some(selected) = selected_issue_id.and_then(|id| {
            self.search
                .filtered_issue_indices
                .iter()
                .position(|index| self.issues[*index].id == id)
        }) {
            self.navigation.selected_issue = selected;
        }
        if self.selected_issue_row().map(|issue| issue.id) != selected_issue_id {
            self.navigation.issues_preview_scroll = 0;
            self.navigation.issues_preview_max_scroll = 0;
        }
    }

    pub fn update_issue_comments_count_by_number(&mut self, issue_number: i64, count: i64) {
        for issue in &mut self.issues {
            if issue.number == issue_number {
                issue.comments_count = count;
            }
        }
    }

    pub fn update_comment_body_by_id(&mut self, comment_id: i64, body: &str) {
        self.preserve_issue_comment_reading_position();
        for comment in &mut self.comments {
            if comment.id == comment_id {
                comment.body = body.to_string();
                self.navigation.issue_comment_offsets.clear();
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

        self.preserve_issue_comment_reading_position();
        self.comments.remove(removed_index);
        self.navigation.issue_comment_offsets.clear();
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
        let Some(key) = self.current_repository_request_key() else {
            return;
        };
        self.interaction
            .pending_issue_actions
            .entry(key)
            .or_default()
            .insert(issue_number, (self.context.id, action));
    }

    pub fn clear_pending_issue_action(
        &mut self,
        owner: &str,
        repo: &str,
        id: Option<i64>,
        issue_number: i64,
    ) {
        let key = repository_key(owner, repo);
        let Some(actions) = self.interaction.pending_issue_actions.get_mut(&key) else {
            return;
        };
        if actions
            .get(&issue_number)
            .is_some_and(|(pending_id, _)| *pending_id == id)
        {
            actions.remove(&issue_number);
        }
        if actions.is_empty() {
            self.interaction.pending_issue_actions.remove(&key);
        }
    }

    pub fn pending_issue_badge(&self, issue_number: i64) -> Option<&'static str> {
        self.interaction
            .pending_issue_actions
            .iter()
            .find_map(|(key, actions)| {
                actions
                    .get(&issue_number)
                    .filter(|(id, _)| self.repository_request_key_is_current(key, *id))
                    .map(|(_, action)| action.label())
            })
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
