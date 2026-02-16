use super::*;

impl App {
    pub(super) fn move_selection_up(&mut self) {
        match self.view {
            View::RepoPicker => {
                if self.navigation.selected_repo > 0 {
                    self.navigation.selected_repo -= 1;
                }
            }
            View::RemoteChooser => {
                if self.navigation.selected_remote > 0 {
                    self.navigation.selected_remote -= 1;
                }
            }
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    self.navigation.issues_preview_scroll =
                        self.navigation.issues_preview_scroll.saturating_sub(1);
                    return;
                }
                if self.navigation.selected_issue > 0 {
                    self.navigation.selected_issue -= 1;
                    self.navigation.issues_preview_scroll = 0;
                }
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    self.navigation.issue_recent_comments_scroll = self
                        .navigation
                        .issue_recent_comments_scroll
                        .saturating_sub(1);
                    return;
                }
                self.navigation.issue_detail_scroll =
                    self.navigation.issue_detail_scroll.saturating_sub(1);
            }
            View::IssueComments => {
                self.jump_prev_comment();
            }
            View::PullRequestFiles => {
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Files {
                    if self.pull_request.selected_pull_request_file > 0 {
                        self.pull_request.selected_pull_request_file -= 1;
                        self.reset_pull_request_diff_view_for_file_selection();
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
                    self.pull_request.selected_pull_request_diff_line,
                );
                self.pull_request.selected_pull_request_diff_line = current;
                if let Some(previous) = self.previous_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    current,
                ) {
                    self.pull_request.selected_pull_request_diff_line = previous;
                }
                self.sync_selected_pull_request_review_comment();
            }
            View::CommentPresetPicker => {
                if self.preset.choice > 0 {
                    self.preset.choice -= 1;
                }
            }
            View::LinkedPicker => {
                if self.linked_picker.selected > 0 {
                    self.linked_picker.selected -= 1;
                }
            }
            View::LabelPicker => {
                let filtered = self.filtered_label_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.metadata_picker.selected_label_option)
                    .unwrap_or(0);
                let next = current.saturating_sub(1);
                self.metadata_picker.selected_label_option = filtered[next];
            }
            View::AssigneePicker => {
                let filtered = self.filtered_assignee_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.metadata_picker.selected_assignee_option)
                    .unwrap_or(0);
                let next = current.saturating_sub(1);
                self.metadata_picker.selected_assignee_option = filtered[next];
            }
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    pub(super) fn move_selection_down(&mut self) {
        match self.view {
            View::RepoPicker => {
                if self.navigation.selected_repo + 1 < self.search.filtered_repo_indices.len() {
                    self.navigation.selected_repo += 1;
                }
            }
            View::RemoteChooser => {
                if self.navigation.selected_remote + 1 < self.remotes.len() {
                    self.navigation.selected_remote += 1;
                }
            }
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    let max = self.navigation.issues_preview_max_scroll;
                    self.navigation.issues_preview_scroll = self
                        .navigation
                        .issues_preview_scroll
                        .saturating_add(1)
                        .min(max);
                    return;
                }
                if self.navigation.selected_issue + 1 < self.search.filtered_issue_indices.len() {
                    self.navigation.selected_issue += 1;
                    self.navigation.issues_preview_scroll = 0;
                }
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    let max = self.navigation.issue_recent_comments_max_scroll;
                    self.navigation.issue_recent_comments_scroll = self
                        .navigation
                        .issue_recent_comments_scroll
                        .saturating_add(1)
                        .min(max);
                    return;
                }
                let max = self.navigation.issue_detail_max_scroll;
                self.navigation.issue_detail_scroll = self
                    .navigation
                    .issue_detail_scroll
                    .saturating_add(1)
                    .min(max);
            }
            View::IssueComments => {
                self.jump_next_comment();
            }
            View::PullRequestFiles => {
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Files {
                    if self.pull_request.selected_pull_request_file + 1
                        < self.pull_request.pull_request_files.len()
                    {
                        self.pull_request.selected_pull_request_file += 1;
                        self.reset_pull_request_diff_view_for_file_selection();
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
                    self.pull_request.selected_pull_request_diff_line,
                );
                self.pull_request.selected_pull_request_diff_line = current;
                if let Some(next) = self.next_visible_pull_request_diff_line(
                    file_path.as_str(),
                    rows.as_slice(),
                    current,
                ) {
                    self.pull_request.selected_pull_request_diff_line = next;
                }
                self.sync_selected_pull_request_review_comment();
            }
            View::CommentPresetPicker => {
                let max = self.preset_items_len();
                if self.preset.choice + 1 < max {
                    self.preset.choice += 1;
                }
            }
            View::LinkedPicker => {
                if self.linked_picker.selected + 1 < self.linked_picker.options.len() {
                    self.linked_picker.selected += 1;
                }
            }
            View::LabelPicker => {
                let filtered = self.filtered_label_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.metadata_picker.selected_label_option)
                    .unwrap_or(0);
                let next = (current + 1).min(filtered.len() - 1);
                self.metadata_picker.selected_label_option = filtered[next];
            }
            View::AssigneePicker => {
                let filtered = self.filtered_assignee_indices();
                if filtered.is_empty() {
                    return;
                }
                let current = filtered
                    .iter()
                    .position(|index| *index == self.metadata_picker.selected_assignee_option)
                    .unwrap_or(0);
                let next = (current + 1).min(filtered.len() - 1);
                self.metadata_picker.selected_assignee_option = filtered[next];
            }
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    pub(super) fn activate_selection(&mut self) {
        match self.view {
            View::RepoPicker => {
                self.interaction.action = Some(AppAction::PickRepo);
            }
            View::RemoteChooser => {
                self.interaction.action = Some(AppAction::PickRemote);
            }
            View::Issues => {
                self.interaction.action = Some(AppAction::PickIssue);
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
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Files {
                    self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Diff;
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                self.toggle_pull_request_diff_expanded();
            }
            View::CommentPresetPicker => {
                self.interaction.action = Some(AppAction::PickPreset);
            }
            View::LinkedPicker => {
                self.interaction.action = Some(AppAction::PickLinkedItem);
            }
            View::CommentPresetName
            | View::CommentEditor
            | View::LabelPicker
            | View::AssigneePicker => {}
        }
    }

    pub(super) fn jump_top(&mut self) {
        match self.view {
            View::RepoPicker => self.navigation.selected_repo = 0,
            View::RemoteChooser => self.navigation.selected_remote = 0,
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    self.navigation.issues_preview_scroll = 0;
                    return;
                }
                self.navigation.selected_issue = 0;
                self.navigation.issues_preview_scroll = 0;
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    self.navigation.issue_recent_comments_scroll = 0;
                    return;
                }
                self.navigation.issue_detail_scroll = 0;
            }
            View::IssueComments => {
                self.navigation.selected_comment = 0;
                self.navigation.issue_comments_scroll = 0;
            }
            View::PullRequestFiles => {
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Files {
                    self.pull_request.selected_pull_request_file = 0;
                    self.reset_pull_request_diff_position();
                    self.sync_selected_pull_request_review_comment();
                    return;
                }
                self.pull_request.selected_pull_request_diff_line = 0;
                self.pull_request.pull_request_diff_scroll = 0;
                self.pull_request.pull_request_diff_horizontal_scroll = 0;
                self.pull_request.pull_request_diff_horizontal_max = 0;
                self.sync_selected_pull_request_review_comment();
            }
            View::CommentPresetPicker => self.preset.choice = 0,
            View::LinkedPicker => self.linked_picker.selected = 0,
            View::LabelPicker => {
                if let Some(index) = self.filtered_label_indices().first() {
                    self.metadata_picker.selected_label_option = *index;
                }
            }
            View::AssigneePicker => {
                if let Some(index) = self.filtered_assignee_indices().first() {
                    self.metadata_picker.selected_assignee_option = *index;
                }
            }
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    pub(super) fn jump_bottom(&mut self) {
        match self.view {
            View::RepoPicker => {
                if !self.search.filtered_repo_indices.is_empty() {
                    self.navigation.selected_repo = self.search.filtered_repo_indices.len() - 1;
                }
            }
            View::RemoteChooser => {
                if !self.remotes.is_empty() {
                    self.navigation.selected_remote = self.remotes.len() - 1;
                }
            }
            View::Issues => {
                if self.focus == Focus::IssuesPreview {
                    self.navigation.issues_preview_scroll =
                        self.navigation.issues_preview_max_scroll;
                    return;
                }
                if !self.search.filtered_issue_indices.is_empty() {
                    self.navigation.selected_issue = self.search.filtered_issue_indices.len() - 1;
                    self.navigation.issues_preview_scroll = 0;
                }
            }
            View::IssueDetail => {
                if self.focus == Focus::IssueRecentComments {
                    self.navigation.issue_recent_comments_scroll =
                        self.navigation.issue_recent_comments_max_scroll;
                    return;
                }
                self.navigation.issue_detail_scroll = self.navigation.issue_detail_max_scroll;
            }
            View::IssueComments => {
                if !self.comments.is_empty() {
                    self.navigation.selected_comment = self.comments.len() - 1;
                }
                self.navigation.issue_comments_scroll = self.navigation.issue_comments_max_scroll;
            }
            View::PullRequestFiles => {
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Files {
                    if !self.pull_request.pull_request_files.is_empty() {
                        self.pull_request.selected_pull_request_file =
                            self.pull_request.pull_request_files.len() - 1;
                        self.reset_pull_request_diff_position();
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
                        self.pull_request.selected_pull_request_diff_line = last_visible;
                    }
                    self.pull_request.pull_request_diff_scroll =
                        self.pull_request.pull_request_diff_max_scroll;
                }
                self.sync_selected_pull_request_review_comment();
            }
            View::CommentPresetPicker => {
                let max = self.preset_items_len();
                if max > 0 {
                    self.preset.choice = max - 1;
                }
            }
            View::LinkedPicker => {
                if !self.linked_picker.options.is_empty() {
                    self.linked_picker.selected = self.linked_picker.options.len() - 1;
                }
            }
            View::LabelPicker => {
                let filtered = self.filtered_label_indices();
                if !filtered.is_empty() {
                    self.metadata_picker.selected_label_option = *filtered.last().unwrap_or(&0);
                }
            }
            View::AssigneePicker => {
                let filtered = self.filtered_assignee_indices();
                if !filtered.is_empty() {
                    self.metadata_picker.selected_assignee_option = *filtered.last().unwrap_or(&0);
                }
            }
            View::CommentPresetName | View::CommentEditor => {}
        }
    }

    pub(super) fn jump_next_comment(&mut self) {
        let offsets = self.comment_offsets();
        if offsets.is_empty() || self.navigation.selected_comment + 1 >= offsets.len() {
            return;
        }
        self.navigation.selected_comment += 1;
        self.navigation.issue_comments_scroll = offsets[self.navigation.selected_comment]
            .min(self.navigation.issue_comments_max_scroll);
        self.status = format!(
            "Comment {}/{}",
            self.navigation.selected_comment + 1,
            offsets.len()
        );
    }

    pub(super) fn jump_prev_comment(&mut self) {
        let offsets = self.comment_offsets();
        if offsets.is_empty() || self.navigation.selected_comment == 0 {
            return;
        }
        self.navigation.selected_comment -= 1;
        self.navigation.issue_comments_scroll = offsets[self.navigation.selected_comment];
        self.status = format!(
            "Comment {}/{}",
            self.navigation.selected_comment + 1,
            offsets.len()
        );
    }

    pub(super) fn comment_offsets(&self) -> Vec<u16> {
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

    pub(super) fn handle_focus_key(&mut self, code: KeyCode) -> bool {
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
                    self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Files;
                    self.pull_request.pull_request_visual_mode = false;
                    self.pull_request.pull_request_visual_anchor = None;
                    self.sync_selected_pull_request_review_comment();
                    true
                }
                KeyCode::Char('l') | KeyCode::Char('j') => {
                    self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Diff;
                    self.sync_selected_pull_request_review_comment();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }
}
