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
