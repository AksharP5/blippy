use super::*;

impl App {
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

    pub(super) fn handle_mouse_scroll(&mut self, target: Option<MouseTarget>, down: bool) {
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

    pub(super) fn handle_mouse_scroll_horizontal(
        &mut self,
        target: Option<MouseTarget>,
        right: bool,
    ) {
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

    pub(super) fn handle_mouse_click_target(&mut self, target: Option<MouseTarget>) {
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
                if self.view == View::LinkedPicker {
                    self.cancel_linked_picker();
                    return;
                }
                if matches!(self.view, View::LabelPicker | View::AssigneePicker) {
                    self.set_view(self.editor_flow.cancel_view);
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
                self.navigation.selected_repo =
                    index.min(self.search.filtered_repo_indices.len().saturating_sub(1));
                self.interaction.action = Some(AppAction::PickRepo);
            }
            Some(MouseTarget::RepoListPane) => {}
            Some(MouseTarget::RemoteRow(index)) => {
                self.navigation.selected_remote = index.min(self.remotes.len().saturating_sub(1));
                self.interaction.action = Some(AppAction::PickRemote);
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
                self.navigation.selected_issue =
                    index.min(self.search.filtered_issue_indices.len().saturating_sub(1));
                self.navigation.issues_preview_scroll = 0;
                self.interaction.action = Some(AppAction::PickIssue);
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
                self.interaction.action = Some(AppAction::OpenLinkedPullRequestInTui);
            }
            Some(MouseTarget::LinkedPullRequestWebButton) => {
                self.focus = Focus::IssuesPreview;
                self.interaction.action = Some(AppAction::OpenLinkedPullRequestInBrowser);
            }
            Some(MouseTarget::LinkedIssueTuiButton) => {
                self.focus = Focus::IssuesPreview;
                self.interaction.action = Some(AppAction::OpenLinkedIssueInTui);
            }
            Some(MouseTarget::LinkedIssueWebButton) => {
                self.focus = Focus::IssuesPreview;
                self.interaction.action = Some(AppAction::OpenLinkedIssueInBrowser);
            }
            Some(MouseTarget::CommentsPane) => {}
            Some(MouseTarget::CommentRow(index)) => {
                self.navigation.selected_comment = index.min(self.comments.len().saturating_sub(1));
            }
            Some(MouseTarget::PullRequestFocusFiles) | Some(MouseTarget::PullRequestFilesPane) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Files);
            }
            Some(MouseTarget::PullRequestFocusDiff) | Some(MouseTarget::PullRequestDiffPane) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
            }
            Some(MouseTarget::PullRequestFileRow(index)) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Files);
                self.pull_request.selected_pull_request_file =
                    index.min(self.pull_request.pull_request_files.len().saturating_sub(1));
                self.reset_pull_request_diff_view_for_file_selection();
                self.sync_selected_pull_request_review_comment();
            }
            Some(MouseTarget::PullRequestDiffRow(index, side)) => {
                self.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
                self.pull_request.pull_request_review_side = side;
                self.pull_request.selected_pull_request_diff_line = index;
                self.sync_selected_pull_request_review_comment();
            }
            Some(MouseTarget::LabelOption(index)) => {
                if let Some(filtered_index) = self.filtered_label_indices().get(index).copied() {
                    self.metadata_picker.selected_label_option = filtered_index;
                    self.toggle_selected_label();
                }
            }
            Some(MouseTarget::LabelApply) => {
                self.interaction.action = Some(AppAction::SubmitLabels);
            }
            Some(MouseTarget::LabelCancel) => {
                self.set_view(self.editor_flow.cancel_view);
            }
            Some(MouseTarget::AssigneeOption(index)) => {
                if let Some(filtered_index) = self.filtered_assignee_indices().get(index).copied() {
                    self.metadata_picker.selected_assignee_option = filtered_index;
                    self.toggle_selected_assignee();
                }
            }
            Some(MouseTarget::AssigneeApply) => {
                self.interaction.action = Some(AppAction::SubmitAssignees);
            }
            Some(MouseTarget::AssigneeCancel) => {
                self.set_view(self.editor_flow.cancel_view);
            }
            Some(MouseTarget::PresetOption(index)) => {
                self.preset.choice = index.min(self.preset_items_len().saturating_sub(1));
                self.interaction.action = Some(AppAction::PickPreset);
            }
            Some(MouseTarget::LinkedPickerOption(index)) => {
                self.set_selected_linked_picker_index(index);
                self.interaction.action = Some(AppAction::PickLinkedItem);
            }
            Some(MouseTarget::LinkedPickerCancel) => {
                self.cancel_linked_picker();
            }
            None => {}
        }
    }

    pub fn clear_mouse_regions(&mut self) {
        self.interaction.mouse_regions.clear();
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
        self.interaction.mouse_regions.push(MouseRegion {
            target,
            x,
            y,
            width,
            height,
        });
    }

    pub(super) fn mouse_target_at(&self, column: u16, row: u16) -> Option<MouseTarget> {
        let mut index = self.interaction.mouse_regions.len();
        while index > 0 {
            index -= 1;
            let region = self.interaction.mouse_regions[index];
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
}
