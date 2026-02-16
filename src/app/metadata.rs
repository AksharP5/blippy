use super::*;

impl App {
    pub fn label_options(&self) -> &[String] {
        &self.metadata_picker.label_options
    }

    pub fn selected_label_option(&self) -> usize {
        self.metadata_picker.selected_label_option
    }

    pub fn label_option_selected(&self, label: &str) -> bool {
        self.metadata_picker
            .label_selected
            .contains(&label.to_ascii_lowercase())
    }

    pub fn label_query(&self) -> &str {
        self.metadata_picker.label_query.as_str()
    }

    pub fn filtered_label_indices(&self) -> Vec<usize> {
        let query = self.metadata_picker.label_query.trim().to_ascii_lowercase();
        self.metadata_picker
            .label_options
            .iter()
            .enumerate()
            .filter_map(|(index, label)| {
                if query.is_empty() {
                    return Some(index);
                }
                if label.to_ascii_lowercase().contains(query.as_str()) {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<usize>>()
    }

    pub fn assignee_options(&self) -> &[String] {
        &self.metadata_picker.assignee_options
    }

    pub fn selected_assignee_option(&self) -> usize {
        self.metadata_picker.selected_assignee_option
    }

    pub fn assignee_option_selected(&self, assignee: &str) -> bool {
        self.metadata_picker
            .assignee_selected
            .contains(&assignee.to_ascii_lowercase())
    }

    pub fn assignee_query(&self) -> &str {
        self.metadata_picker.assignee_query.as_str()
    }

    pub fn filtered_assignee_indices(&self) -> Vec<usize> {
        let query = self
            .metadata_picker
            .assignee_query
            .trim()
            .to_ascii_lowercase();
        self.metadata_picker
            .assignee_options
            .iter()
            .enumerate()
            .filter_map(|(index, assignee)| {
                if query.is_empty() {
                    return Some(index);
                }
                if assignee.to_ascii_lowercase().contains(query.as_str()) {
                    return Some(index);
                }
                None
            })
            .collect::<Vec<usize>>()
    }

    pub fn open_label_picker(
        &mut self,
        return_view: View,
        mut options: Vec<String>,
        current_labels: &str,
    ) {
        self.editor_flow.cancel_view = return_view;
        options.sort_by_key(|value| value.to_ascii_lowercase());
        options.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        self.metadata_picker.label_options = options;
        self.metadata_picker.selected_label_option = 0;
        self.metadata_picker.label_query.clear();
        self.metadata_picker.label_selected = Self::csv_set(current_labels);
        self.set_view(View::LabelPicker);
    }

    pub fn open_assignee_picker(
        &mut self,
        return_view: View,
        mut options: Vec<String>,
        current_assignees: &str,
    ) {
        self.editor_flow.cancel_view = return_view;
        options.sort_by_key(|value| value.to_ascii_lowercase());
        options.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        self.metadata_picker.assignee_options = options;
        self.metadata_picker.selected_assignee_option = 0;
        self.metadata_picker.assignee_query.clear();
        self.metadata_picker.assignee_selected = Self::csv_set(current_assignees);
        self.set_view(View::AssigneePicker);
    }

    pub fn merge_label_options(&mut self, labels: Vec<String>) {
        let mut merged = self.metadata_picker.label_options.clone();
        for label in labels {
            if label.trim().is_empty() {
                continue;
            }
            if merged
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(label.as_str()))
            {
                continue;
            }
            merged.push(label);
        }
        merged.sort_by_key(|value| value.to_ascii_lowercase());
        self.metadata_picker.label_options = merged;
        if let Some(index) = self.filtered_label_indices().first() {
            self.metadata_picker.selected_label_option = *index;
        }
    }

    pub fn merge_repo_label_colors(&mut self, labels: Vec<(String, String)>) {
        for (name, color) in labels {
            let key = name.trim().to_ascii_lowercase();
            let value = color.trim().trim_start_matches('#').to_string();
            if key.is_empty() || value.len() != 6 {
                continue;
            }
            self.repo_label_colors.insert(key, value);
        }
    }

    pub fn merge_assignee_options(&mut self, assignees: Vec<String>) {
        let mut merged = self.metadata_picker.assignee_options.clone();
        for assignee in assignees {
            if assignee.trim().is_empty() {
                continue;
            }
            if merged
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(assignee.as_str()))
            {
                continue;
            }
            merged.push(assignee);
        }
        merged.sort_by_key(|value| value.to_ascii_lowercase());
        self.metadata_picker.assignee_options = merged;
        if let Some(index) = self.filtered_assignee_indices().first() {
            self.metadata_picker.selected_assignee_option = *index;
        }
    }

    pub fn selected_labels_csv(&self) -> String {
        let mut values = self
            .metadata_picker
            .label_options
            .iter()
            .filter(|label| self.label_option_selected(label.as_str()))
            .cloned()
            .collect::<Vec<String>>();
        values.sort_by_key(|value| value.to_ascii_lowercase());
        values.join(",")
    }

    pub fn selected_labels(&self) -> Vec<String> {
        self.selected_labels_csv()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<String>>()
    }

    pub fn selected_assignees_csv(&self) -> String {
        let mut values = self
            .metadata_picker
            .assignee_options
            .iter()
            .filter(|assignee| self.assignee_option_selected(assignee.as_str()))
            .cloned()
            .collect::<Vec<String>>();
        values.sort_by_key(|value| value.to_ascii_lowercase());
        values.join(",")
    }

    pub fn selected_assignees(&self) -> Vec<String> {
        self.selected_assignees_csv()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<String>>()
    }

    fn csv_set(input: &str) -> HashSet<String> {
        input
            .split(',')
            .map(str::trim)
            .map(|value| value.to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<String>>()
    }

    pub(super) fn toggle_selected_label(&mut self) {
        if !self
            .filtered_label_indices()
            .contains(&self.metadata_picker.selected_label_option)
        {
            return;
        }
        let label = match self
            .metadata_picker
            .label_options
            .get(self.metadata_picker.selected_label_option)
        {
            Some(label) => label.to_ascii_lowercase(),
            None => return,
        };
        if self.metadata_picker.label_selected.contains(label.as_str()) {
            self.metadata_picker.label_selected.remove(label.as_str());
            return;
        }
        self.metadata_picker.label_selected.insert(label);
    }

    pub(super) fn toggle_selected_assignee(&mut self) {
        if !self
            .filtered_assignee_indices()
            .contains(&self.metadata_picker.selected_assignee_option)
        {
            return;
        }
        let assignee = match self
            .metadata_picker
            .assignee_options
            .get(self.metadata_picker.selected_assignee_option)
        {
            Some(assignee) => assignee.to_ascii_lowercase(),
            None => return,
        };
        if self
            .metadata_picker
            .assignee_selected
            .contains(assignee.as_str())
        {
            self.metadata_picker
                .assignee_selected
                .remove(assignee.as_str());
            return;
        }
        self.metadata_picker.assignee_selected.insert(assignee);
    }

    pub(super) fn handle_popup_filter_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            if self.view == View::LabelPicker {
                self.metadata_picker.label_query.clear();
                if let Some(index) = self.filtered_label_indices().first() {
                    self.metadata_picker.selected_label_option = *index;
                }
                return true;
            }
            if self.view == View::AssigneePicker {
                self.metadata_picker.assignee_query.clear();
                if let Some(index) = self.filtered_assignee_indices().first() {
                    self.metadata_picker.selected_assignee_option = *index;
                }
                return true;
            }
        }

        match key.code {
            KeyCode::Backspace => {
                if self.view == View::LabelPicker {
                    self.metadata_picker.label_query.pop();
                    if let Some(index) = self.filtered_label_indices().first() {
                        self.metadata_picker.selected_label_option = *index;
                    }
                    return true;
                }
                if self.view == View::AssigneePicker {
                    self.metadata_picker.assignee_query.pop();
                    if let Some(index) = self.filtered_assignee_indices().first() {
                        self.metadata_picker.selected_assignee_option = *index;
                    }
                    return true;
                }
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                if self.view == View::LabelPicker {
                    if self.metadata_picker.label_query.is_empty()
                        && matches!(ch, 'j' | 'k' | 'g' | 'G')
                    {
                        return false;
                    }
                    self.metadata_picker.label_query.push(ch);
                    if let Some(index) = self.filtered_label_indices().first() {
                        self.metadata_picker.selected_label_option = *index;
                    }
                    return true;
                }
                if self.view == View::AssigneePicker {
                    if self.metadata_picker.assignee_query.is_empty()
                        && matches!(ch, 'j' | 'k' | 'g' | 'G')
                    {
                        return false;
                    }
                    self.metadata_picker.assignee_query.push(ch);
                    if let Some(index) = self.filtered_assignee_indices().first() {
                        self.metadata_picker.selected_assignee_option = *index;
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }
}
