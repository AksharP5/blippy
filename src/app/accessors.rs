use super::*;

impl App {
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
        self.search
            .filtered_repo_indices
            .iter()
            .filter_map(|index| self.repos.get(*index))
            .collect::<Vec<&LocalRepoRow>>()
    }

    pub fn repo_query(&self) -> &str {
        self.search.repo_query.as_str()
    }

    pub fn repo_search_mode(&self) -> bool {
        self.search.repo_search_mode
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
        self.search
            .filtered_issue_indices
            .iter()
            .filter_map(|index| self.issues.get(*index))
            .collect::<Vec<&IssueRow>>()
    }

    pub fn selected_issue_row(&self) -> Option<&IssueRow> {
        let issue_index = *self
            .search
            .filtered_issue_indices
            .get(self.navigation.selected_issue)?;
        self.issues.get(issue_index)
    }

    pub fn current_issue_row(&self) -> Option<&IssueRow> {
        let issue_id = self.context.issue_id?;
        self.issues.iter().find(|issue| issue.id == issue_id)
    }

    pub fn issue_filter(&self) -> IssueFilter {
        self.issue_filter
    }

    pub fn work_item_mode(&self) -> WorkItemMode {
        self.work_item_mode
    }

    pub fn current_repo_path(&self) -> Option<&str> {
        self.context.path.as_deref()
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

    pub fn set_issue_filter(&mut self, filter: IssueFilter) {
        self.issue_filter = filter;
        self.rebuild_issue_filter();
        self.navigation.issues_preview_scroll = 0;
        self.status = format!(
            "Filter: {} | assignee: {}",
            self.issue_filter.label(),
            self.assignee_filter.label()
        );
    }

    pub fn set_work_item_mode(&mut self, mode: WorkItemMode) {
        self.work_item_mode = mode;
        self.rebuild_issue_filter();
        self.navigation.selected_issue = 0;
        self.navigation.issues_preview_scroll = 0;
    }

    pub fn select_issue_by_number(&mut self, issue_number: i64) -> bool {
        let selected = self.search.filtered_issue_indices.iter().position(|index| {
            self.issues
                .get(*index)
                .is_some_and(|issue| issue.number == issue_number)
        });
        let selected = match selected {
            Some(selected) => selected,
            None => return false,
        };
        self.navigation.selected_issue = selected;
        self.navigation.issues_preview_scroll = 0;
        true
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
        self.navigation.selected_repo
    }

    pub fn selected_repo_target(&self) -> Option<(String, String, String)> {
        let repo_index = *self
            .search
            .filtered_repo_indices
            .get(self.navigation.selected_repo)?;
        let repo = self.repos.get(repo_index)?;
        Some((repo.owner.clone(), repo.repo.clone(), repo.path.clone()))
    }

    pub fn selected_remote(&self) -> usize {
        self.navigation.selected_remote
    }

    pub fn selected_issue(&self) -> usize {
        self.navigation.selected_issue
    }

    pub fn selected_comment(&self) -> usize {
        self.navigation.selected_comment
    }

    pub fn selected_comment_row(&self) -> Option<&CommentRow> {
        self.comments.get(self.navigation.selected_comment)
    }

    pub fn issue_detail_scroll(&self) -> u16 {
        self.navigation.issue_detail_scroll
    }

    pub fn issues_preview_scroll(&self) -> u16 {
        self.navigation.issues_preview_scroll
    }

    pub fn issue_comments_scroll(&self) -> u16 {
        self.navigation.issue_comments_scroll
    }

    pub fn issue_recent_comments_scroll(&self) -> u16 {
        self.navigation.issue_recent_comments_scroll
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn current_owner(&self) -> Option<&str> {
        self.context.owner.as_deref()
    }

    pub fn current_repo(&self) -> Option<&str> {
        self.context.repo.as_deref()
    }

    pub fn scanning(&self) -> bool {
        self.sync.scanning
    }

    pub fn syncing(&self) -> bool {
        self.sync.syncing
    }

    pub fn repo_permissions_syncing(&self) -> bool {
        self.sync.repo_permissions_syncing
    }

    pub fn repo_labels_syncing(&self) -> bool {
        self.sync.repo_labels_syncing
    }

    pub fn repo_issue_metadata_editable(&self) -> Option<bool> {
        self.sync.repo_issue_metadata_editable
    }

    pub fn repo_label_color(&self, label: &str) -> Option<&str> {
        let key = label.trim().to_ascii_lowercase();
        self.repo_label_colors.get(&key).map(String::as_str)
    }

    pub fn comment_syncing(&self) -> bool {
        self.sync.comment_syncing
    }

    pub fn pull_request_files_syncing(&self) -> bool {
        self.sync.pull_request_files_syncing
    }

    pub fn pull_request_review_comments_syncing(&self) -> bool {
        self.sync.pull_request_review_comments_syncing
    }
}
