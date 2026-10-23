use super::*;

impl App {
    pub fn on_paste(&mut self, text: &str) {
        if self.search.help_overlay_visible {
            return;
        }
        let (input, multiline) = match self.view {
            View::CommentEditor if self.comment_editor.create_issue_confirm_visible() => return,
            View::CommentEditor if !self.comment_editor.create_issue_title_focused() => {
                (&mut self.comment_editor.text, true)
            }
            View::CommentEditor | View::CommentPresetName => (&mut self.comment_editor.name, false),
            View::Issues if self.search.issue_search_mode => (&mut self.search.issue_query, false),
            View::RepoPicker if self.search.repo_search_mode => {
                (&mut self.search.repo_query, false)
            }
            View::LabelPicker => (&mut self.metadata_picker.label_query, false),
            View::AssigneePicker => (&mut self.metadata_picker.assignee_query, false),
            _ => return,
        };
        // Terminals send pasted line endings as CR, LF, or CRLF.
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        input.extend(text.chars().filter_map(|ch| match ch {
            '\n' | '\t' if !multiline => Some(' '),
            '\n' | '\t' => Some(ch),
            ch if ch.is_control() => None,
            ch => Some(ch),
        }));

        match self.view {
            View::Issues => {
                self.rebuild_issue_filter();
                self.navigation.issues_preview_scroll = 0;
                self.update_search_status();
            }
            View::RepoPicker => {
                self.rebuild_repo_picker_filter();
                self.navigation.selected_repo = 0;
            }
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
            _ => {}
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
                KeyCode::Char(ch)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_comments_keep_line_breaks_and_wait_for_submit() {
        let mut config = Config::default();
        config.keybinds.insert("back_escape".into(), "q".into());
        let mut app = App::new(config);
        app.open_issue_comment_editor(View::IssueComments);
        app.on_paste("first line\r\n\tquoted λ\rlast line\n");

        assert_eq!(app.editor().text(), "first line\n\tquoted λ\nlast line\n");
        assert_eq!(app.view(), View::CommentEditor);
        assert!(app.take_action().is_none());
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.take_action(),
            Some(AppAction::SubmitIssueComment)
        ));
    }

    #[test]
    fn pasted_issue_title_and_body_preserve_confirmation() {
        let mut app = App::new(Config::default());
        app.open_create_issue_editor(View::Issues);
        app.on_paste("Issue\r\ntitle");
        assert_eq!(app.editor().name(), "Issue title");
        assert!(!app.editor().create_issue_confirm_visible());
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        app.on_paste("Body\nline two");
        assert_eq!(app.editor().text(), "Body\nline two");
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.editor().create_issue_confirm_visible());
        app.on_paste("\n\rmore text");
        assert_eq!(app.editor().text(), "Body\nline two");
        assert!(app.take_action().is_none());
    }

    #[test]
    fn pasted_text_updates_search_and_picker_queries_without_actions() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        app.on_paste("is:open\nparser");
        assert_eq!(app.issue_query(), "is:open parser");
        assert!(app.issue_search_mode());

        app.set_view(View::RepoPicker);
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        app.on_paste("acme/repo");
        assert_eq!(app.repo_query(), "acme/repo");
        assert!(app.repo_search_mode());

        app.open_label_picker(
            View::Issues,
            vec!["good first issue".into(), "bug".into()],
            &[],
        );
        app.on_paste("good first issue");
        assert_eq!(app.label_query(), "good first issue");
        assert_eq!(app.filtered_label_indices(), vec![1]);
        assert_eq!(app.selected_label_option(), 1);
        assert!(app.selected_labels().is_empty());

        app.open_assignee_picker(View::Issues, vec!["jane".into(), "alice".into()], "");
        app.on_paste("jane");
        assert_eq!(app.assignee_query(), "jane");
        assert_eq!(app.filtered_assignee_indices(), vec![1]);
        assert_eq!(app.selected_assignee_option(), 1);
        assert!(app.take_action().is_none());
    }

    #[test]
    fn paste_does_not_run_browsing_shortcuts_or_dismiss_help() {
        let mut app = App::new(Config::default());
        app.set_view(View::Issues);
        app.on_paste("p?dd\r\nM\u{1b}\u{3}");
        assert_eq!(app.view(), View::Issues);
        assert_eq!(app.work_item_mode(), WorkItemMode::Issues);
        assert!(!app.help_overlay_visible());
        assert!(!app.should_quit());
        assert!(app.take_action().is_none());
        app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        app.on_paste("?\u{1b}");
        assert!(app.help_overlay_visible());
        assert!(app.take_action().is_none());
    }
}
