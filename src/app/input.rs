use super::*;

impl App {
    pub fn on_key(&mut self, key: KeyEvent) {
        let key = match self.keybinds.remap_key(key) {
            Some(key) => key,
            None => return,
        };
        if matches!(self.view, View::CommentPresetName | View::CommentEditor) {
            self.handle_editor_key(key);
            return;
        }
        if self.view == View::RepoPicker
            && self.search.repo_search_mode
            && self.handle_repo_search_key(key)
        {
            return;
        }
        if self.view == View::Issues
            && self.search.issue_search_mode
            && self.handle_issue_search_key(key)
        {
            return;
        }
        if matches!(self.view, View::LabelPicker | View::AssigneePicker)
            && self.handle_popup_filter_key(key)
        {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('r')
            && self.view == View::RepoPicker
        {
            self.sync.rescan_requested = true;
            self.sync.scanning = true;
            self.status = "Scanning".to_string();
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && self.handle_focus_key(key.code) {
            return;
        }

        if key.code != KeyCode::Char('g') {
            self.interaction.pending_g = false;
        }
        if key.code != KeyCode::Char('d') {
            self.interaction.pending_d = false;
        }

        if key.code == KeyCode::Char('?') {
            self.search.help_overlay_visible = !self.search.help_overlay_visible;
            return;
        }
        if self.search.help_overlay_visible && key.code == KeyCode::Esc {
            self.search.help_overlay_visible = false;
            return;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_repo_picker();
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && self.view == View::RepoPicker => {
                self.search.repo_search_mode = true;
                self.status = "Search repos".to_string();
            }
            KeyCode::Char('/') if key.modifiers.is_empty() && self.view == View::Issues => {
                self.search.issue_search_mode = true;
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
                self.navigation.issues_preview_scroll = 0;
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
                if self.interaction.pending_g {
                    self.jump_top();
                    self.interaction.pending_g = false;
                } else {
                    self.interaction.pending_g = true;
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
                    !self.search.filtered_issue_indices.is_empty()
                } else {
                    self.context.issue_id.is_some() && self.context.issue_number.is_some()
                };
                if !has_issue {
                    self.interaction.pending_d = false;
                    self.status = "No issue selected".to_string();
                    return;
                }
                if self.current_view_issue_is_closed() {
                    self.interaction.pending_d = false;
                    self.status = "Issue already closed".to_string();
                    return;
                }
                if self.interaction.pending_d {
                    self.interaction.action = Some(AppAction::CloseIssue);
                    self.interaction.pending_d = false;
                } else {
                    self.interaction.pending_d = true;
                }
            }
            KeyCode::Char('G') => self.jump_bottom(),
            KeyCode::Char('c')
                if self.view == View::PullRequestFiles
                    && self.pull_request.pull_request_review_focus
                        == PullRequestReviewFocus::Diff =>
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
                self.interaction.action = Some(AppAction::AddIssueComment);
            }
            KeyCode::Char('w') if self.view == View::PullRequestFiles => {
                self.interaction.action = Some(AppAction::TogglePullRequestFileViewed);
            }
            KeyCode::Char('m') if self.view == View::PullRequestFiles => {
                self.interaction.action = Some(AppAction::AddPullRequestReviewComment);
            }
            KeyCode::Char('e') if self.view == View::PullRequestFiles => {
                self.interaction.action = Some(AppAction::EditPullRequestReviewComment);
            }
            KeyCode::Char('x') if self.view == View::PullRequestFiles => {
                self.interaction.action = Some(AppAction::DeletePullRequestReviewComment);
            }
            KeyCode::Char('R') if self.view == View::PullRequestFiles => {
                self.interaction.action = Some(AppAction::ResolvePullRequestReviewComment);
            }
            KeyCode::Char('n') if self.view == View::PullRequestFiles => {
                self.cycle_pull_request_review_comment(true);
            }
            KeyCode::Char('p') if self.view == View::PullRequestFiles => {
                self.cycle_pull_request_review_comment(false);
            }
            KeyCode::Char('h') if self.view == View::PullRequestFiles => {
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Diff {
                    self.pull_request.pull_request_review_side = ReviewSide::Left;
                    self.sync_selected_pull_request_review_comment();
                }
            }
            KeyCode::Char('l') if self.view == View::PullRequestFiles => {
                if self.pull_request.pull_request_review_focus == PullRequestReviewFocus::Diff {
                    self.pull_request.pull_request_review_side = ReviewSide::Right;
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
                self.interaction.action = Some(AppAction::EditIssueComment);
            }
            KeyCode::Char('x') if self.view == View::IssueComments => {
                self.interaction.action = Some(AppAction::DeleteIssueComment);
            }
            KeyCode::Char('l')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
            {
                self.interaction.action = Some(AppAction::EditLabels);
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
                self.interaction.action = Some(AppAction::EditAssignees);
            }
            KeyCode::Char('u')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
            {
                self.interaction.action = Some(AppAction::ReopenIssue);
            }
            KeyCode::Char(' ') if self.view == View::LabelPicker => {
                self.toggle_selected_label();
            }
            KeyCode::Char(' ') if self.view == View::AssigneePicker => {
                self.toggle_selected_assignee();
            }
            KeyCode::Enter if self.view == View::LabelPicker => {
                self.toggle_selected_label();
                self.interaction.action = Some(AppAction::SubmitLabels);
            }
            KeyCode::Enter if self.view == View::AssigneePicker => {
                self.toggle_selected_assignee();
                self.interaction.action = Some(AppAction::SubmitAssignees);
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
                self.set_view(self.editor_flow.cancel_view);
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
                self.interaction.action = Some(AppAction::OpenInBrowser);
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
                    self.interaction.action = Some(AppAction::OpenLinkedIssueInBrowser);
                    return;
                }
                self.interaction.action = Some(AppAction::OpenLinkedPullRequestInBrowser);
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
                    self.interaction.action = Some(AppAction::OpenLinkedIssueInTui);
                    return;
                }
                self.interaction.action = Some(AppAction::OpenLinkedPullRequestInTui);
            }
            KeyCode::Char('v')
                if matches!(
                    self.view,
                    View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
                ) =>
            {
                self.interaction.action = Some(AppAction::CheckoutPullRequest);
            }
            _ => {}
        }
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
        self.search.help_overlay_visible = false;
        if self.view != View::PullRequestFiles {
            self.pull_request.pull_request_diff_expanded = false;
        }
        match self.view {
            View::Issues => self.focus = Focus::IssuesList,
            View::IssueDetail => self.focus = Focus::IssueBody,
            View::PullRequestFiles => {
                self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Files;
            }
            _ => {
                self.search.issue_search_mode = false;
                if self.view != View::RepoPicker {
                    self.search.repo_search_mode = false;
                }
            }
        }
    }
}
