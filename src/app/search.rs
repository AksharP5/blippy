use super::*;

impl App {
    pub(super) fn open_repo_picker(&mut self) {
        self.search.repo_query.clear();
        self.search.repo_search_mode = false;
        self.rebuild_repo_picker_filter();
        self.set_view(View::RepoPicker);
    }

    pub fn issue_query(&self) -> &str {
        self.search.issue_query.as_str()
    }

    pub fn issue_search_mode(&self) -> bool {
        self.search.issue_search_mode
    }

    pub fn help_overlay_visible(&self) -> bool {
        self.search.help_overlay_visible
    }

    pub(super) fn rebuild_issue_filter(&mut self) {
        let query = self.search.issue_query.trim().to_ascii_lowercase();
        self.search.filtered_issue_indices = self
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

        self.search
            .filtered_issue_indices
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

        if self.navigation.selected_issue >= self.search.filtered_issue_indices.len() {
            self.navigation.selected_issue =
                self.search.filtered_issue_indices.len().saturating_sub(1);
        }
    }

    pub(super) fn issue_matches_query(issue: &IssueRow, query: &str) -> bool {
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
                if value == "closed" {
                    return issue_state_is_closed(issue.state.as_str());
                }
                if value == "merged" {
                    return issue_state_is_merged(issue.state.as_str());
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

    pub(super) fn cycle_assignee_filter(&mut self, forward: bool) {
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
        self.navigation.issues_preview_scroll = 0;
        self.status = format!(
            "Assignee: {} ({} items)",
            self.assignee_filter.label(),
            self.search.filtered_issue_indices.len()
        );
    }

    pub(super) fn reset_assignee_filter(&mut self) {
        self.assignee_filter = AssigneeFilter::All;
        self.rebuild_issue_filter();
        self.navigation.issues_preview_scroll = 0;
        self.status = format!(
            "Assignee: {} ({} items)",
            self.assignee_filter.label(),
            self.search.filtered_issue_indices.len()
        );
    }

    pub(super) fn assignee_filter_options(&self) -> Vec<AssigneeFilter> {
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

    pub(super) fn assignee_filter_matches(&self, issue: &IssueRow) -> bool {
        match &self.assignee_filter {
            AssigneeFilter::All => true,
            AssigneeFilter::Unassigned => issue.assignees.trim().is_empty(),
            AssigneeFilter::User(user) => Self::issue_has_assignee(issue.assignees.as_str(), user),
        }
    }

    pub(super) fn issue_has_assignee(issue_assignees: &str, user: &str) -> bool {
        issue_assignees
            .split(',')
            .map(str::trim)
            .any(|assignee| assignee.eq_ignore_ascii_case(user))
    }

    pub(super) fn handle_issue_search_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            self.search.issue_query.clear();
            self.rebuild_issue_filter();
            self.navigation.issues_preview_scroll = 0;
            self.update_search_status();
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                self.search.issue_search_mode = false;
                self.search.issue_query.clear();
                self.rebuild_issue_filter();
                self.navigation.issues_preview_scroll = 0;
                self.status = "Search cleared".to_string();
            }
            KeyCode::Enter => {
                self.search.issue_search_mode = false;
                self.update_search_status();
            }
            KeyCode::Backspace => {
                self.search.issue_query.pop();
                self.rebuild_issue_filter();
                self.navigation.issues_preview_scroll = 0;
                self.update_search_status();
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.search.issue_query.push(ch);
                self.rebuild_issue_filter();
                self.navigation.issues_preview_scroll = 0;
                self.update_search_status();
            }
            _ => {}
        }
        true
    }

    pub(super) fn handle_repo_search_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            self.search.repo_query.clear();
            self.rebuild_repo_picker_filter();
            self.navigation.selected_repo = 0;
            self.status = "Repo search cleared".to_string();
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                self.search.repo_search_mode = false;
                self.search.repo_query.clear();
                self.rebuild_repo_picker_filter();
                self.navigation.selected_repo = 0;
                self.status = String::new();
            }
            KeyCode::Enter => {
                self.search.repo_search_mode = false;
                self.status = format!("{} repos", self.search.filtered_repo_indices.len());
            }
            KeyCode::Backspace => {
                self.search.repo_query.pop();
                self.rebuild_repo_picker_filter();
                self.navigation.selected_repo = 0;
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.search.repo_query.push(ch);
                self.rebuild_repo_picker_filter();
                self.navigation.selected_repo = 0;
            }
            _ => {}
        }
        true
    }

    pub(super) fn update_search_status(&mut self) {
        if self.search.issue_query.trim().is_empty() {
            self.status = format!(
                "Filter: {} | assignee: {}",
                self.issue_filter.label(),
                self.assignee_filter.label()
            );
            return;
        }
        self.status = format!(
            "Search: {} | assignee: {} ({} results)",
            self.search.issue_query,
            self.assignee_filter.label(),
            self.search.filtered_issue_indices.len()
        );
    }

    pub(super) fn rebuild_repo_picker_filter(&mut self) {
        let query = self.search.repo_query.trim().to_ascii_lowercase();
        self.search.filtered_repo_indices = self
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
