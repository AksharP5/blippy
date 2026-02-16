use super::*;

impl App {
    pub fn linked_pull_request_for_issue(&self, issue_number: i64) -> Option<i64> {
        self.linked
            .pull_requests
            .get(&issue_number)
            .and_then(|pull_number| *pull_number)
    }

    pub fn linked_issue_for_pull_request(&self, pull_number: i64) -> Option<i64> {
        self.linked
            .issues
            .get(&pull_number)
            .and_then(|issue_number| *issue_number)
    }

    pub fn linked_pull_request_known(&self, issue_number: i64) -> bool {
        self.linked.pull_requests.contains_key(&issue_number)
    }

    pub fn linked_issue_known(&self, pull_number: i64) -> bool {
        self.linked.issues.contains_key(&pull_number)
    }

    pub fn begin_linked_pull_request_lookup(&mut self, issue_number: i64) -> bool {
        if self.linked_pull_request_known(issue_number) {
            return false;
        }
        self.linked.pull_request_lookups.insert(issue_number)
    }

    pub fn begin_linked_issue_lookup(&mut self, pull_number: i64) -> bool {
        if self.linked_issue_known(pull_number) {
            return false;
        }
        self.linked.issue_lookups.insert(pull_number)
    }

    pub fn end_linked_pull_request_lookup(&mut self, issue_number: i64) {
        self.linked.pull_request_lookups.remove(&issue_number);
    }

    pub fn end_linked_issue_lookup(&mut self, pull_number: i64) {
        self.linked.issue_lookups.remove(&pull_number);
    }

    pub fn set_linked_pull_request(&mut self, issue_number: i64, pull_number: Option<i64>) {
        self.end_linked_pull_request_lookup(issue_number);
        if pull_number.is_none()
            && self
                .linked
                .pull_requests
                .get(&issue_number)
                .is_some_and(|existing| existing.is_some())
        {
            return;
        }
        self.linked.pull_requests.insert(issue_number, pull_number);
        if let Some(pull_number) = pull_number {
            self.linked.issues.insert(pull_number, Some(issue_number));
            self.end_linked_issue_lookup(pull_number);
        }
    }

    pub fn set_linked_issue_for_pull_request(
        &mut self,
        pull_number: i64,
        issue_number: Option<i64>,
    ) {
        self.end_linked_issue_lookup(pull_number);
        if issue_number.is_none()
            && self
                .linked
                .issues
                .get(&pull_number)
                .is_some_and(|existing| existing.is_some())
        {
            return;
        }
        self.linked.issues.insert(pull_number, issue_number);
        if let Some(issue_number) = issue_number {
            self.linked
                .pull_requests
                .insert(issue_number, Some(pull_number));
            self.end_linked_pull_request_lookup(issue_number);
        }
    }

    pub fn capture_linked_navigation_origin(&mut self) {
        let issue = match self.current_or_selected_issue() {
            Some(issue) => issue,
            None => return,
        };
        let mode = if issue.is_pr {
            WorkItemMode::PullRequests
        } else {
            WorkItemMode::Issues
        };
        self.linked.navigation_origin = Some((issue.number, mode));
    }

    pub fn clear_linked_navigation_origin(&mut self) {
        self.linked.navigation_origin = None;
    }

    pub fn selected_issue_has_known_linked_pr(&self) -> bool {
        let issue = match self.current_or_selected_issue() {
            Some(issue) => issue,
            None => return false,
        };
        if issue.is_pr {
            return false;
        }
        self.linked_pull_request_for_issue(issue.number).is_some()
    }

    pub fn selected_pull_request_has_known_linked_issue(&self) -> bool {
        let issue = match self.current_or_selected_issue() {
            Some(issue) => issue,
            None => return false,
        };
        if !issue.is_pr {
            return false;
        }
        self.linked_issue_for_pull_request(issue.number).is_some()
    }

    pub(super) fn restore_linked_navigation_origin(&mut self) -> bool {
        let (issue_number, mode) = match self.linked.navigation_origin {
            Some(origin) => origin,
            None => return false,
        };
        self.linked.navigation_origin = None;

        self.set_view(View::Issues);
        self.set_work_item_mode(mode);
        let try_filters = [IssueFilter::Open, IssueFilter::Closed];
        for filter in try_filters {
            self.set_issue_filter(filter);
            if !self.select_issue_by_number(issue_number) {
                continue;
            }
            self.status = format!("Returned to #{}", issue_number);
            return true;
        }

        self.status = format!("Could not return to #{}", issue_number);
        false
    }
}
