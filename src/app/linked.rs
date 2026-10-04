use super::*;

impl App {
    pub fn linked_pull_request_for_issue(&self, issue_number: i64) -> Option<i64> {
        self.linked
            .pull_requests
            .get(&issue_number)
            .and_then(|items| items.numbers.first().copied())
    }

    pub fn linked_issue_for_pull_request(&self, pull_number: i64) -> Option<i64> {
        self.linked
            .issues
            .get(&pull_number)
            .and_then(|items| items.numbers.first().copied())
    }

    pub fn linked_pull_requests_for_issue(&self, issue_number: i64) -> Vec<i64> {
        self.linked
            .pull_requests
            .get(&issue_number)
            .map(|items| items.numbers.clone())
            .unwrap_or_default()
    }

    pub fn linked_issues_for_pull_request(&self, pull_number: i64) -> Vec<i64> {
        self.linked
            .issues
            .get(&pull_number)
            .map(|items| items.numbers.clone())
            .unwrap_or_default()
    }

    pub fn linked_pull_request_known(&self, issue_number: i64) -> bool {
        self.linked
            .pull_requests
            .get(&issue_number)
            .is_some_and(|items| items.lookup == LinkedLookup::Complete)
    }

    pub fn linked_issue_known(&self, pull_number: i64) -> bool {
        self.linked
            .issues
            .get(&pull_number)
            .is_some_and(|items| items.lookup == LinkedLookup::Complete)
    }

    pub fn begin_linked_pull_request_lookup(&mut self, issue_number: i64) -> bool {
        self.linked
            .pull_requests
            .entry(issue_number)
            .or_default()
            .begin_lookup()
    }

    pub fn begin_linked_issue_lookup(&mut self, pull_number: i64) -> bool {
        self.linked
            .issues
            .entry(pull_number)
            .or_default()
            .begin_lookup()
    }

    pub fn end_linked_pull_request_lookup(&mut self, issue_number: i64) {
        if let Some(items) = self.linked.pull_requests.get_mut(&issue_number) {
            items.cancel_lookup();
        }
    }

    pub fn end_linked_issue_lookup(&mut self, pull_number: i64) {
        if let Some(items) = self.linked.issues.get_mut(&pull_number) {
            items.cancel_lookup();
        }
    }

    pub(super) fn retry_incomplete_linked_lookups(&mut self) {
        for items in self
            .linked
            .pull_requests
            .values_mut()
            .chain(self.linked.issues.values_mut())
        {
            if items.lookup == LinkedLookup::Incomplete {
                items.lookup = LinkedLookup::Ready;
            }
        }
    }

    #[cfg(test)]
    pub fn set_linked_pull_request(&mut self, issue_number: i64, pull_number: Option<i64>) {
        let pull_numbers = match pull_number {
            Some(pull_number) => vec![pull_number],
            None => Vec::new(),
        };
        self.set_linked_pull_requests(issue_number, pull_numbers, true);
    }

    pub fn set_linked_pull_requests(
        &mut self,
        issue_number: i64,
        pull_numbers: Vec<i64>,
        complete: bool,
    ) {
        let pull_numbers = dedupe_numbers(pull_numbers);
        self.linked
            .pull_requests
            .entry(issue_number)
            .or_default()
            .finish_lookup(pull_numbers.clone(), complete);
        for pull_number in pull_numbers {
            push_unique(
                &mut self.linked.issues.entry(pull_number).or_default().numbers,
                issue_number,
            );
        }
    }

    #[cfg(test)]
    pub fn set_linked_issue_for_pull_request(
        &mut self,
        pull_number: i64,
        issue_number: Option<i64>,
    ) {
        let issue_numbers = match issue_number {
            Some(issue_number) => vec![issue_number],
            None => Vec::new(),
        };
        self.set_linked_issues_for_pull_request(pull_number, issue_numbers, true);
    }

    pub fn set_linked_issues_for_pull_request(
        &mut self,
        pull_number: i64,
        issue_numbers: Vec<i64>,
        complete: bool,
    ) {
        let issue_numbers = dedupe_numbers(issue_numbers);
        self.linked
            .issues
            .entry(pull_number)
            .or_default()
            .finish_lookup(issue_numbers.clone(), complete);
        for issue_number in issue_numbers {
            push_unique(
                &mut self
                    .linked
                    .pull_requests
                    .entry(issue_number)
                    .or_default()
                    .numbers,
                pull_number,
            );
        }
    }

    pub fn open_linked_picker(
        &mut self,
        cancel_view: View,
        target: LinkedPickerTarget,
        options: Vec<i64>,
    ) {
        let numbers = dedupe_numbers(options);
        let options = numbers
            .into_iter()
            .map(|number| LinkedPickerOption {
                number,
                title: self
                    .issues
                    .iter()
                    .find(|issue| issue.number == number)
                    .map(|issue| issue.title.clone()),
            })
            .collect::<Vec<LinkedPickerOption>>();
        if options.is_empty() {
            return;
        }
        self.linked_picker.options = options;
        self.linked_picker.selected = 0;
        self.linked_picker.target = Some(target);
        self.linked_picker.cancel_view = cancel_view;
        self.linked_picker.origin = self.current_or_selected_issue().map(|issue| {
            let mode = if issue.is_pr {
                WorkItemMode::PullRequests
            } else {
                WorkItemMode::Issues
            };
            (issue.number, mode)
        });
        self.set_view(View::LinkedPicker);
    }

    pub fn linked_picker_numbers(&self) -> Vec<i64> {
        self.linked_picker
            .options
            .iter()
            .map(|option| option.number)
            .collect::<Vec<i64>>()
    }

    pub fn linked_picker_labels(&self) -> Vec<String> {
        self.linked_picker
            .options
            .iter()
            .map(|option| linked_picker_label(option.number, option.title.as_deref()))
            .collect::<Vec<String>>()
    }

    pub fn selected_linked_picker_index(&self) -> usize {
        self.linked_picker.selected
    }

    pub fn selected_linked_picker_number(&self) -> Option<i64> {
        self.linked_picker
            .options
            .get(self.linked_picker.selected)
            .map(|option| option.number)
    }

    pub fn linked_picker_target(&self) -> Option<LinkedPickerTarget> {
        self.linked_picker.target
    }

    pub fn linked_picker_cancel_view(&self) -> View {
        self.linked_picker.cancel_view
    }

    #[cfg(test)]
    pub fn linked_picker_origin(&self) -> Option<(i64, WorkItemMode)> {
        self.linked_picker.origin
    }

    pub fn apply_linked_picker_navigation_origin(&mut self) {
        if let Some(origin) = self.linked_picker.origin {
            self.linked.navigation_origin = Some(origin);
        }
    }

    pub fn set_selected_linked_picker_index(&mut self, index: usize) {
        if self.linked_picker.options.is_empty() {
            self.linked_picker.selected = 0;
            return;
        }
        self.linked_picker.selected = index.min(self.linked_picker.options.len() - 1);
    }

    pub fn clear_linked_picker_state(&mut self) {
        self.linked_picker.options.clear();
        self.linked_picker.selected = 0;
        self.linked_picker.target = None;
        self.linked_picker.origin = None;
    }

    pub fn cancel_linked_picker(&mut self) {
        let cancel_view = self.linked_picker.cancel_view;
        self.clear_linked_picker_state();
        self.set_view(cancel_view);
    }

    pub fn linked_picker_title(&self) -> &'static str {
        match self.linked_picker.target {
            Some(LinkedPickerTarget::PullRequestTui) => "Open Linked Pull Request",
            Some(LinkedPickerTarget::PullRequestBrowser) => "Open Linked Pull Request (Web)",
            Some(LinkedPickerTarget::IssueTui) => "Open Linked Issue",
            Some(LinkedPickerTarget::IssueBrowser) => "Open Linked Issue (Web)",
            None => "Choose Linked Item",
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

        if self.reveal_issue_by_number(issue_number, mode) {
            self.set_view(View::Issues);
            self.status = format!("Returned to #{}", issue_number);
            return true;
        }

        self.status = format!("Could not return to #{}", issue_number);
        false
    }
}

impl LinkedItems {
    fn begin_lookup(&mut self) -> bool {
        if self.lookup != LinkedLookup::Ready {
            return false;
        }
        self.lookup = LinkedLookup::Loading;
        true
    }

    fn cancel_lookup(&mut self) {
        if self.lookup == LinkedLookup::Loading {
            self.lookup = LinkedLookup::Ready;
        }
    }

    fn finish_lookup(&mut self, numbers: Vec<i64>, complete: bool) {
        if complete && !numbers.is_empty() {
            self.numbers = numbers;
        } else {
            for number in numbers {
                push_unique(&mut self.numbers, number);
            }
        }
        self.lookup = if complete {
            LinkedLookup::Complete
        } else {
            LinkedLookup::Incomplete
        };
    }
}

fn dedupe_numbers(values: Vec<i64>) -> Vec<i64> {
    let mut deduped = Vec::new();
    for value in values {
        if deduped.contains(&value) {
            continue;
        }
        deduped.push(value);
    }
    deduped
}

fn push_unique(values: &mut Vec<i64>, value: i64) {
    if values.contains(&value) {
        return;
    }
    values.push(value);
}

fn linked_picker_label(number: i64, title: Option<&str>) -> String {
    let title = title.map(str::trim).filter(|title| !title.is_empty());
    if let Some(title) = title {
        return format!("#{}  {}", number, title);
    }
    format!("#{}", number)
}
