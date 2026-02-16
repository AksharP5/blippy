use super::*;

impl App {
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
        self.editor_flow.editing_comment_id = None;
        self.comment_editor.reset_for_close();
        self.editor_flow.cancel_view = View::CommentPresetPicker;
        self.set_view(View::CommentEditor);
    }

    pub fn open_issue_comment_editor(&mut self, return_view: View) {
        self.editor_flow.editing_comment_id = None;
        self.pull_request.editing_pull_request_review_comment_id = None;
        self.pull_request.pending_review_target = None;
        self.comment_editor.reset_for_comment();
        self.editor_flow.cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_create_issue_editor(&mut self, return_view: View) {
        self.editor_flow.editing_comment_id = None;
        self.pull_request.editing_pull_request_review_comment_id = None;
        self.pull_request.pending_review_target = None;
        self.comment_editor.reset_for_issue_create();
        self.editor_flow.cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_comment_edit_editor(&mut self, return_view: View, comment_id: i64, body: &str) {
        self.editor_flow.editing_comment_id = Some(comment_id);
        self.pull_request.editing_pull_request_review_comment_id = None;
        self.pull_request.pending_review_target = None;
        self.comment_editor.reset_for_comment_edit(body);
        self.editor_flow.cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_pull_request_review_comment_editor(
        &mut self,
        return_view: View,
        target: PullRequestReviewTarget,
    ) {
        self.pull_request.editing_pull_request_review_comment_id = None;
        self.pull_request.pending_review_target = Some(target);
        self.comment_editor.reset_for_pull_request_review_comment();
        self.editor_flow.cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn open_pull_request_review_comment_edit_editor(
        &mut self,
        return_view: View,
        comment_id: i64,
        body: &str,
    ) {
        self.pull_request.editing_pull_request_review_comment_id = Some(comment_id);
        self.pull_request.pending_review_target = None;
        self.comment_editor
            .reset_for_pull_request_review_comment_edit(body);
        self.editor_flow.cancel_view = return_view;
        self.set_view(View::CommentEditor);
    }

    pub fn editor_cancel_view(&self) -> View {
        self.editor_flow.cancel_view
    }

    pub fn take_editing_comment_id(&mut self) -> Option<i64> {
        self.editor_flow.editing_comment_id.take()
    }

    pub fn take_pending_review_target(&mut self) -> Option<PullRequestReviewTarget> {
        self.pull_request.pending_review_target.take()
    }

    pub fn take_editing_pull_request_review_comment_id(&mut self) -> Option<i64> {
        self.pull_request
            .editing_pull_request_review_comment_id
            .take()
    }

    pub(super) fn handle_editor_key(&mut self, key: KeyEvent) {
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
                    self.editor_flow.cancel_view = View::CommentPresetPicker;
                    self.set_view(View::CommentEditor);
                }
                KeyCode::Backspace => self.comment_editor.backspace_name(),
                KeyCode::Char(ch) => self.comment_editor.append_name(ch),
                _ => {}
            },
            View::CommentEditor => match key.code {
                KeyCode::Esc => {
                    if self.comment_editor.mode() == EditorMode::CreateIssue
                        && self.comment_editor.create_issue_confirm_visible()
                    {
                        self.comment_editor.hide_create_issue_confirm();
                        return;
                    }
                    self.editor_flow.editing_comment_id = None;
                    self.pull_request.editing_pull_request_review_comment_id = None;
                    self.pull_request.pending_review_target = None;
                    self.set_view(self.editor_flow.cancel_view);
                }
                KeyCode::Tab => {
                    if self.comment_editor.create_issue_confirm_visible() {
                        self.comment_editor
                            .toggle_create_issue_confirm_submit_selected();
                    }
                }
                KeyCode::BackTab => {
                    if self.comment_editor.create_issue_confirm_visible() {
                        self.comment_editor
                            .toggle_create_issue_confirm_submit_selected();
                    }
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    if self.comment_editor.mode().allows_multiline()
                        && !self.comment_editor.create_issue_title_focused()
                        && !self.comment_editor.create_issue_confirm_visible()
                    {
                        self.comment_editor.newline()
                    }
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                    if self.comment_editor.mode().allows_multiline()
                        && !self.comment_editor.create_issue_title_focused()
                        && !self.comment_editor.create_issue_confirm_visible()
                    {
                        self.comment_editor.newline()
                    }
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.comment_editor.mode() == EditorMode::CreateIssue {
                        self.comment_editor.focus_create_issue_body();
                        return;
                    }
                    if self.comment_editor.mode().allows_multiline()
                        && !self.comment_editor.create_issue_confirm_visible()
                    {
                        self.comment_editor.newline()
                    }
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.comment_editor.mode() == EditorMode::CreateIssue {
                        self.comment_editor.focus_create_issue_title();
                    }
                }
                KeyCode::Char('j') if self.comment_editor.create_issue_confirm_visible() => {
                    self.comment_editor
                        .set_create_issue_confirm_submit_selected(true);
                }
                KeyCode::Char('k') if self.comment_editor.create_issue_confirm_visible() => {
                    self.comment_editor
                        .set_create_issue_confirm_submit_selected(false);
                }
                KeyCode::Left if self.comment_editor.create_issue_confirm_visible() => {
                    self.comment_editor
                        .set_create_issue_confirm_submit_selected(false);
                }
                KeyCode::Right if self.comment_editor.create_issue_confirm_visible() => {
                    self.comment_editor
                        .set_create_issue_confirm_submit_selected(true);
                }
                KeyCode::Enter => match self.comment_editor.mode() {
                    EditorMode::CloseIssue => {
                        self.interaction.action = Some(AppAction::SubmitComment);
                    }
                    EditorMode::CreateIssue => {
                        if self.comment_editor.create_issue_confirm_visible() {
                            if self.comment_editor.create_issue_confirm_submit_selected() {
                                self.interaction.action = Some(AppAction::SubmitCreatedIssue);
                            } else {
                                self.comment_editor.hide_create_issue_confirm();
                            }
                            return;
                        }
                        if self.comment_editor.name().trim().is_empty() {
                            self.status = "Issue title required".to_string();
                            return;
                        }
                        self.comment_editor.show_create_issue_confirm();
                    }
                    EditorMode::AddComment => {
                        self.interaction.action = Some(AppAction::SubmitIssueComment);
                    }
                    EditorMode::EditComment => {
                        self.interaction.action = Some(AppAction::SubmitEditedComment);
                    }
                    EditorMode::AddPullRequestReviewComment => {
                        self.interaction.action = Some(AppAction::SubmitPullRequestReviewComment);
                    }
                    EditorMode::EditPullRequestReviewComment => {
                        self.interaction.action =
                            Some(AppAction::SubmitEditedPullRequestReviewComment);
                    }
                    EditorMode::AddPreset => {
                        self.interaction.action = Some(AppAction::SavePreset);
                    }
                },
                KeyCode::Backspace => {
                    if self.comment_editor.create_issue_confirm_visible() {
                        return;
                    }
                    if self.comment_editor.mode() == EditorMode::CreateIssue
                        && self.comment_editor.create_issue_title_focused()
                    {
                        self.comment_editor.backspace_name();
                    } else {
                        self.comment_editor.backspace_text();
                    }
                }
                KeyCode::Char(ch) => {
                    if self.comment_editor.create_issue_confirm_visible() {
                        return;
                    }
                    if self.comment_editor.mode() == EditorMode::CreateIssue
                        && self.comment_editor.create_issue_title_focused()
                    {
                        self.comment_editor.append_name(ch);
                    } else {
                        self.comment_editor.append_text(ch);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}
