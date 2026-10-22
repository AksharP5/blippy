use super::*;

impl App {
    pub fn pull_request_files(&self) -> &[PullRequestFile] {
        &self.pull_request.pull_request_files
    }

    pub fn pull_request_id(&self) -> Option<&str> {
        self.pull_request.pull_request_id.as_deref()
    }

    pub fn pull_request_file_is_viewed(&self, file_path: &str) -> bool {
        self.pull_request
            .pull_request_viewed_files
            .contains(file_path)
    }

    pub fn pull_request_view_state_loaded(&self) -> bool {
        self.pull_request.pull_request_view_state_loaded
    }

    pub fn pull_request_hunk_is_collapsed(&self, file_path: &str, hunk_start: usize) -> bool {
        self.pull_request
            .pull_request_collapsed_hunks
            .get(file_path)
            .is_some_and(|collapsed| collapsed.contains(&hunk_start))
    }

    pub fn visible_pull_request_diff_rows<'a>(
        &'a self,
        file_path: &str,
        rows: &'a [crate::pr_diff::DiffRow],
    ) -> impl Iterator<Item = (usize, &'a crate::pr_diff::DiffRow)> + 'a {
        let collapsed_hunks = self
            .pull_request
            .pull_request_collapsed_hunks
            .get(file_path);
        let mut hidden = false;
        rows.iter().enumerate().filter(move |(index, row)| {
            if row.kind == DiffKind::Hunk {
                hidden = collapsed_hunks.is_some_and(|hunks| hunks.contains(index));
                return true;
            }
            !hidden
        })
    }

    pub fn pull_request_hunk_hidden_line_count(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        hunk_start: usize,
    ) -> usize {
        if !self.pull_request_hunk_is_collapsed(file_path, hunk_start) {
            return 0;
        }
        let hunk_end = match pull_request_hunk_end(rows, hunk_start) {
            Some(hunk_end) => hunk_end,
            None => return 0,
        };
        hunk_end.saturating_sub(hunk_start)
    }

    pub fn set_pull_request_file_viewed(&mut self, file_path: &str, viewed: bool) {
        if viewed {
            self.pull_request
                .pull_request_viewed_files
                .insert(file_path.to_string());
            return;
        }
        self.pull_request
            .pull_request_viewed_files
            .remove(file_path);
    }

    pub fn begin_pull_request_file_view_update(&mut self, issue_id: i64, path: &str) -> bool {
        self.interaction
            .pending_file_view_updates
            .insert((issue_id, path.to_string()))
    }

    pub fn finish_pull_request_file_view_update(&mut self, issue_id: i64, path: &str) {
        self.interaction
            .pending_file_view_updates
            .remove(&(issue_id, path.to_string()));
    }

    pub fn selected_pull_request_file_view_toggle(&self) -> Option<(String, bool)> {
        if !self.pull_request_view_state_loaded() {
            return None;
        }
        let file = self.selected_pull_request_file_row()?;
        let viewed = self.pull_request_file_is_viewed(file.filename.as_str());
        Some((file.filename.clone(), !viewed))
    }

    pub fn set_pull_request_view_state(
        &mut self,
        pull_request_id: Option<String>,
        viewed_files: HashSet<String>,
    ) {
        self.pull_request.pull_request_id = pull_request_id;
        self.pull_request.pull_request_view_state_loaded = true;
        self.pull_request.pull_request_viewed_files = viewed_files;
        self.pull_request
            .pull_request_viewed_files
            .retain(|file_path| {
                self.pull_request
                    .pull_request_files
                    .iter()
                    .any(|file| file.filename == *file_path)
            });
    }

    pub fn pull_request_review_focus(&self) -> PullRequestReviewFocus {
        self.pull_request.pull_request_review_focus
    }

    pub fn selected_pull_request_file(&self) -> usize {
        self.pull_request.selected_pull_request_file
    }

    pub fn selected_pull_request_diff_line(&self) -> usize {
        self.pull_request.selected_pull_request_diff_line
    }

    pub fn pull_request_review_side(&self) -> ReviewSide {
        self.pull_request.pull_request_review_side
    }

    pub fn pull_request_visual_range(&self) -> Option<(usize, usize)> {
        if !self.pull_request.pull_request_visual_mode {
            return None;
        }
        Some(self.selected_pull_request_diff_range())
    }

    pub fn selected_pull_request_review_comment_id(&self) -> Option<i64> {
        self.pull_request.selected_pull_request_review_comment_id
    }

    pub fn pull_request_diff_scroll(&self) -> u16 {
        self.pull_request.pull_request_diff_scroll
    }

    pub fn pull_request_diff_horizontal_scroll(&self) -> u16 {
        self.pull_request.pull_request_diff_horizontal_scroll
    }

    pub fn pull_request_diff_horizontal_max(&self) -> u16 {
        self.pull_request.pull_request_diff_horizontal_max
    }

    pub fn pull_request_diff_expanded(&self) -> bool {
        self.pull_request.pull_request_diff_expanded
    }

    pub fn selected_pull_request_file_row(&self) -> Option<&PullRequestFile> {
        self.pull_request
            .pull_request_files
            .get(self.pull_request.selected_pull_request_file)
    }

    pub fn selected_pull_request_review_target(&self) -> Option<PullRequestReviewTarget> {
        let file = self.selected_pull_request_file_row()?;
        let rows = parse_patch(file.patch.as_deref());
        self.review_target_for_rows(file.filename.as_str(), rows.as_slice())
    }

    pub fn pull_request_comments_for_path_and_line(
        &self,
        path: &str,
        side: ReviewSide,
        line: i64,
    ) -> Vec<&PullRequestReviewComment> {
        let comments = &self.pull_request.pull_request_review_comments;
        // set_pull_request_review_comments keeps comments sorted by path, line, and ID.
        let start = comments
            .partition_point(|comment| (comment.path.as_str(), comment.line) < (path, line));
        comments[start..]
            .iter()
            .take_while(|comment| comment.path == path && comment.line == line)
            .filter(|comment| comment.anchored && comment.side == side)
            .collect()
    }

    pub fn pull_request_comments_count_for_path(&self, path: &str) -> usize {
        let comments = &self.pull_request.pull_request_review_comments;
        let start = comments.partition_point(|comment| comment.path.as_str() < path);
        let end = comments.partition_point(|comment| comment.path.as_str() <= path);
        end - start
    }

    pub fn selected_pull_request_review_comment(&self) -> Option<&PullRequestReviewComment> {
        let target = self.selected_pull_request_review_target()?;
        let mut comments = self
            .pull_request
            .pull_request_review_comments
            .iter()
            .filter(|comment| {
                comment.anchored
                    && comment.path == target.path
                    && comment.side == target.side
                    && comment.line == target.line
            })
            .collect::<Vec<&PullRequestReviewComment>>();
        comments.sort_by_key(|comment| comment.id);

        if let Some(comment_id) = self.pull_request.selected_pull_request_review_comment_id
            && let Some(comment) = comments.iter().find(|comment| comment.id == comment_id)
        {
            return Some(*comment);
        }
        comments.first().copied()
    }

    pub fn update_pull_request_review_comment_body_by_id(&mut self, comment_id: i64, body: &str) {
        for comment in &mut self.pull_request.pull_request_review_comments {
            if comment.id != comment_id {
                continue;
            }
            comment.body = body.to_string();
            return;
        }
    }

    pub fn remove_pull_request_review_comment_by_id(&mut self, comment_id: i64) {
        self.pull_request
            .pull_request_review_comments
            .retain(|comment| comment.id != comment_id);
        if self.pull_request.selected_pull_request_review_comment_id == Some(comment_id) {
            self.pull_request.selected_pull_request_review_comment_id = None;
            self.sync_selected_pull_request_review_comment();
        }
    }

    pub fn set_pull_request_files(
        &mut self,
        issue_id: i64,
        head_sha: String,
        files: Vec<PullRequestFile>,
    ) {
        self.pull_request.pull_request_files_issue_id = Some(issue_id);
        self.pull_request.pull_request_files_head_sha = Some(head_sha);
        self.pull_request.pull_request_id = None;
        self.pull_request.pull_request_view_state_loaded = false;
        self.pull_request.pull_request_files = files;
        let mut active_file_paths = HashSet::new();
        for file in &self.pull_request.pull_request_files {
            active_file_paths.insert(file.filename.clone());
        }
        self.pull_request.pull_request_viewed_files.clear();
        self.pull_request
            .pull_request_collapsed_hunks
            .retain(|file_path, _| active_file_paths.contains(file_path));
        self.pull_request.selected_pull_request_file = 0;
        self.pull_request.selected_pull_request_diff_line = 0;
        self.pull_request.pull_request_diff_scroll = 0;
        self.pull_request.pull_request_diff_horizontal_scroll = 0;
        self.pull_request.pull_request_diff_max_scroll = 0;
        self.pull_request.pull_request_diff_horizontal_max = 0;
        self.pull_request.pull_request_diff_expanded = false;
        self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Files;
        self.pull_request.pull_request_visual_mode = false;
        self.pull_request.pull_request_visual_anchor = None;
        self.pull_request.selected_pull_request_review_comment_id = None;
    }

    pub fn set_pull_request_review_comments(
        &mut self,
        mut comments: Vec<PullRequestReviewComment>,
    ) {
        comments.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line.cmp(&right.line))
                .then(left.id.cmp(&right.id))
        });
        self.pull_request.pull_request_review_comments = comments;
        self.pull_request.selected_pull_request_review_comment_id = self
            .selected_pull_request_review_comment()
            .map(|comment| comment.id);
    }

    pub fn set_pull_request_review_focus(&mut self, focus: PullRequestReviewFocus) {
        self.pull_request.pull_request_review_focus = focus;
        if focus == PullRequestReviewFocus::Files {
            self.pull_request.pull_request_diff_expanded = false;
            self.pull_request.pull_request_visual_mode = false;
            self.pull_request.pull_request_visual_anchor = None;
        }
        if focus == PullRequestReviewFocus::Diff {
            let selected_file = self
                .selected_pull_request_file_row()
                .map(|file| (file.filename.clone(), file.patch.clone()));
            if let Some((file_path, patch)) = selected_file {
                let rows = parse_patch(patch.as_deref());
                self.pull_request.selected_pull_request_diff_line = self
                    .nearest_visible_pull_request_diff_line(
                        file_path.as_str(),
                        rows.as_slice(),
                        self.pull_request.selected_pull_request_diff_line,
                    );
            }
        }
        self.sync_selected_pull_request_review_comment();
    }

    pub fn set_pull_request_diff_max_scroll(&mut self, max_scroll: u16) {
        self.pull_request.pull_request_diff_max_scroll = max_scroll;
        if self.pull_request.pull_request_diff_scroll > max_scroll {
            self.pull_request.pull_request_diff_scroll = max_scroll;
        }
    }

    pub fn set_pull_request_diff_scroll(&mut self, scroll: u16) {
        self.pull_request.pull_request_diff_scroll =
            scroll.min(self.pull_request.pull_request_diff_max_scroll);
    }

    pub fn reset_pull_request_diff_horizontal_scroll(&mut self) {
        self.pull_request.pull_request_diff_horizontal_scroll = 0;
    }

    pub fn set_pull_request_diff_horizontal_max(&mut self, max_scroll: u16) {
        self.pull_request.pull_request_diff_horizontal_max = max_scroll;
        if self.pull_request.pull_request_diff_horizontal_scroll > max_scroll {
            self.pull_request.pull_request_diff_horizontal_scroll = max_scroll;
        }
    }

    pub(super) fn toggle_pull_request_diff_expanded(&mut self) {
        if self.view != View::PullRequestFiles
            || self.pull_request.pull_request_review_focus != PullRequestReviewFocus::Diff
        {
            return;
        }
        self.pull_request.pull_request_diff_expanded =
            !self.pull_request.pull_request_diff_expanded;
        if self.pull_request.pull_request_diff_expanded {
            self.status = "Expanded diff view".to_string();
            return;
        }
        self.status = "Split files and diff view".to_string();
    }

    pub(super) fn back_from_pull_request_files(&mut self) {
        if self.pull_request.pull_request_diff_expanded {
            self.pull_request.pull_request_diff_expanded = false;
            self.status = "Split files and diff view".to_string();
            return;
        }
        self.set_view(View::IssueDetail);
    }

    pub(super) fn scroll_pull_request_diff_horizontal(&mut self, delta: i16) {
        if self.view != View::PullRequestFiles
            || self.pull_request.pull_request_review_focus != PullRequestReviewFocus::Diff
        {
            return;
        }
        let amount = delta.unsigned_abs();
        if delta.is_negative() {
            self.pull_request.pull_request_diff_horizontal_scroll = self
                .pull_request
                .pull_request_diff_horizontal_scroll
                .saturating_sub(amount);
            return;
        }
        self.pull_request.pull_request_diff_horizontal_scroll = self
            .pull_request
            .pull_request_diff_horizontal_scroll
            .saturating_add(amount)
            .min(self.pull_request.pull_request_diff_horizontal_max);
    }

    pub(super) fn reset_pull_request_state(&mut self) {
        self.pull_request.pull_request_files_issue_id = None;
        self.pull_request.pull_request_files_head_sha = None;
        self.pull_request.pull_request_id = None;
        self.pull_request.pull_request_view_state_loaded = false;
        self.pull_request.pull_request_files.clear();
        self.pull_request.pull_request_viewed_files.clear();
        self.pull_request.pull_request_collapsed_hunks.clear();
        self.pull_request.pull_request_review_comments.clear();
        self.pull_request.selected_pull_request_file = 0;
        self.pull_request.selected_pull_request_diff_line = 0;
        self.pull_request.pull_request_diff_scroll = 0;
        self.pull_request.pull_request_diff_horizontal_scroll = 0;
        self.pull_request.pull_request_diff_max_scroll = 0;
        self.pull_request.pull_request_diff_horizontal_max = 0;
        self.pull_request.pull_request_diff_expanded = false;
        self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Files;
        self.pull_request.pull_request_review_side = ReviewSide::Right;
        self.pull_request.pull_request_visual_mode = false;
        self.pull_request.pull_request_visual_anchor = None;
        self.pull_request.selected_pull_request_review_comment_id = None;
        self.pull_request.editing_pull_request_review_comment_id = None;
        self.pull_request.pending_review_target = None;
    }

    pub(super) fn reset_pull_request_diff_position(&mut self) {
        self.pull_request.selected_pull_request_diff_line = 0;
        self.pull_request.pull_request_diff_scroll = 0;
        self.pull_request.pull_request_diff_horizontal_scroll = 0;
        self.pull_request.pull_request_diff_horizontal_max = 0;
        self.pull_request.pull_request_visual_mode = false;
        self.pull_request.pull_request_visual_anchor = None;
    }

    pub(super) fn reset_pull_request_diff_view_for_file_selection(&mut self) {
        self.reset_pull_request_diff_position();
        self.pull_request.pull_request_diff_expanded = false;
    }

    fn collapsed_pull_request_hunk_start(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> Option<usize> {
        let collapsed_hunks = self
            .pull_request
            .pull_request_collapsed_hunks
            .get(file_path)
            .filter(|hunks| !hunks.is_empty())?;
        let hunk_start = rows
            .get(..=row_index)?
            .iter()
            .rposition(|row| row.kind == DiffKind::Hunk)?;
        collapsed_hunks.contains(&hunk_start).then_some(hunk_start)
    }

    pub(super) fn nearest_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> usize {
        if rows.is_empty() {
            return 0;
        }
        let row_index = row_index.min(rows.len() - 1);
        self.collapsed_pull_request_hunk_start(file_path, rows, row_index)
            .unwrap_or(row_index)
    }

    pub(super) fn next_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> Option<usize> {
        if rows.is_empty() {
            return None;
        }
        let mut index = row_index.min(rows.len() - 1) + 1;
        if let Some(hunk_start) = self.collapsed_pull_request_hunk_start(file_path, rows, index)
            && index > hunk_start
        {
            index = pull_request_hunk_end(rows, hunk_start)? + 1;
        }
        (index < rows.len()).then_some(index)
    }

    pub(super) fn previous_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
        row_index: usize,
    ) -> Option<usize> {
        if rows.is_empty() {
            return None;
        }
        let index = row_index.min(rows.len() - 1).checked_sub(1)?;
        Some(self.nearest_visible_pull_request_diff_line(file_path, rows, index))
    }

    pub(super) fn last_visible_pull_request_diff_line(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
    ) -> Option<usize> {
        let index = rows.len().checked_sub(1)?;
        Some(self.nearest_visible_pull_request_diff_line(file_path, rows, index))
    }

    pub(super) fn toggle_selected_pull_request_hunk_collapsed(&mut self) {
        if self.pull_request.pull_request_review_focus != PullRequestReviewFocus::Diff {
            self.status = "Focus the diff pane to collapse sections".to_string();
            return;
        }

        let selected_file = match self.selected_pull_request_file_row() {
            Some(file) => (file.filename.clone(), file.patch.clone()),
            None => {
                self.status = "No file selected".to_string();
                return;
            }
        };
        let file_path = selected_file.0;
        let rows = parse_patch(selected_file.1.as_deref());
        if rows.is_empty() {
            self.status = "No diff section to collapse".to_string();
            return;
        }

        let selected_line = self
            .pull_request
            .selected_pull_request_diff_line
            .min(rows.len().saturating_sub(1));
        let hunk_range = match pull_request_hunk_range_for_row(rows.as_slice(), selected_line) {
            Some(hunk_range) => hunk_range,
            None => {
                self.status = "No hunk at this line".to_string();
                return;
            }
        };

        let mut collapsed = true;
        let mut remove_entry = false;
        {
            let collapsed_hunks = self
                .pull_request
                .pull_request_collapsed_hunks
                .entry(file_path.clone())
                .or_default();
            if !collapsed_hunks.insert(hunk_range.start) {
                collapsed_hunks.remove(&hunk_range.start);
                collapsed = false;
            }
            if collapsed_hunks.is_empty() {
                remove_entry = true;
            }
        }
        if remove_entry {
            self.pull_request
                .pull_request_collapsed_hunks
                .remove(file_path.as_str());
        }

        self.pull_request.selected_pull_request_diff_line = hunk_range.start;
        self.pull_request.pull_request_visual_mode = false;
        self.pull_request.pull_request_visual_anchor = None;
        self.sync_selected_pull_request_review_comment();

        if collapsed {
            let hidden_lines = hunk_range.end.saturating_sub(hunk_range.start);
            self.status = format!("Collapsed {} lines in {}", hidden_lines, file_path);
            return;
        }
        self.status = format!("Expanded section in {}", file_path);
    }

    pub(super) fn toggle_pull_request_visual_mode(&mut self) {
        if self.pull_request.pull_request_review_focus != PullRequestReviewFocus::Diff {
            self.pull_request.pull_request_review_focus = PullRequestReviewFocus::Diff;
        }
        if self.pull_request.pull_request_visual_mode {
            self.pull_request.pull_request_visual_mode = false;
            self.pull_request.pull_request_visual_anchor = None;
            self.sync_selected_pull_request_review_comment();
            return;
        }
        self.pull_request.pull_request_visual_mode = true;
        self.pull_request.pull_request_visual_anchor =
            Some(self.pull_request.selected_pull_request_diff_line);
        self.sync_selected_pull_request_review_comment();
    }

    pub(super) fn selected_pull_request_diff_range(&self) -> (usize, usize) {
        if !self.pull_request.pull_request_visual_mode {
            return (
                self.pull_request.selected_pull_request_diff_line,
                self.pull_request.selected_pull_request_diff_line,
            );
        }
        let anchor = self
            .pull_request
            .pull_request_visual_anchor
            .unwrap_or(self.pull_request.selected_pull_request_diff_line);
        let start = anchor.min(self.pull_request.selected_pull_request_diff_line);
        let end = anchor.max(self.pull_request.selected_pull_request_diff_line);
        (start, end)
    }

    pub(super) fn review_target_for_rows(
        &self,
        file_path: &str,
        rows: &[crate::pr_diff::DiffRow],
    ) -> Option<PullRequestReviewTarget> {
        let commit_id = self.pull_request.pull_request_files_head_sha.as_ref()?;
        if rows.is_empty() {
            return None;
        }
        let (start_index, end_index) = self.selected_pull_request_diff_range();
        let start_index = start_index.min(rows.len() - 1);
        let end_index = end_index.min(rows.len() - 1);
        let hunk_range = pull_request_hunk_range_for_row(rows, start_index)?;
        // GitHub requires multiline review threads to stay within one diff hunk.
        if end_index > hunk_range.end {
            return None;
        }

        let side = self.pull_request.pull_request_review_side;
        let mut selected_lines = Vec::new();
        for row in &rows[start_index..=end_index] {
            let line = match side {
                ReviewSide::Left => row.old_line,
                ReviewSide::Right => row.new_line,
            };
            if line.is_none() {
                continue;
            }
            selected_lines.push(line.unwrap_or_default());
        }

        if selected_lines.is_empty() {
            let row = rows.get(self.pull_request.selected_pull_request_diff_line)?;
            match row.kind {
                DiffKind::Added | DiffKind::Context => {
                    return Some(PullRequestReviewTarget {
                        commit_id: commit_id.clone(),
                        path: file_path.to_string(),
                        line: row.new_line?,
                        side: ReviewSide::Right,
                        start_line: None,
                        start_side: None,
                    });
                }
                DiffKind::Removed => {
                    return Some(PullRequestReviewTarget {
                        commit_id: commit_id.clone(),
                        path: file_path.to_string(),
                        line: row.old_line?,
                        side: ReviewSide::Left,
                        start_line: None,
                        start_side: None,
                    });
                }
                _ => return None,
            }
        }

        let line = *selected_lines.last().unwrap_or(&0);
        let start_line = if selected_lines.len() > 1 {
            selected_lines.first().copied()
        } else {
            None
        };

        Some(PullRequestReviewTarget {
            commit_id: commit_id.clone(),
            path: file_path.to_string(),
            line,
            side,
            start_line,
            start_side: start_line.map(|_| side),
        })
    }

    pub(super) fn cycle_pull_request_review_comment(&mut self, forward: bool) {
        let target = match self.selected_pull_request_review_target() {
            Some(target) => target,
            None => return,
        };
        let mut ids = self
            .pull_request
            .pull_request_review_comments
            .iter()
            .filter(|comment| {
                comment.anchored
                    && comment.path == target.path
                    && comment.side == target.side
                    && comment.line == target.line
            })
            .map(|comment| comment.id)
            .collect::<Vec<i64>>();
        ids.sort_unstable();
        if ids.is_empty() {
            self.pull_request.selected_pull_request_review_comment_id = None;
            return;
        }
        let current_index = self
            .pull_request
            .selected_pull_request_review_comment_id
            .and_then(|id| ids.iter().position(|value| *value == id))
            .unwrap_or(0);
        let next_index = if forward {
            (current_index + 1) % ids.len()
        } else if current_index == 0 {
            ids.len() - 1
        } else {
            current_index - 1
        };
        self.pull_request.selected_pull_request_review_comment_id = Some(ids[next_index]);
    }

    pub(super) fn sync_selected_pull_request_review_comment(&mut self) {
        let comment_id = self
            .selected_pull_request_review_comment()
            .map(|comment| comment.id);
        self.pull_request.selected_pull_request_review_comment_id = comment_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend};

    const PATCH: &str = "diff --git a/example b/example\n@@ -1,2 +1,2 @@\n hidden first\n hidden second\n@@ -5,0 +5,0 @@\n@@ -10,2 +10,2 @@\n visible one\n visible two\n@@ -20 +20 @@\n hidden last\n";

    fn collapsed_app() -> App {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            "head".into(),
            vec![PullRequestFile {
                filename: "example".into(),
                status: "modified".into(),
                additions: 0,
                deletions: 0,
                patch: Some(PATCH.into()),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        for index in [1, 4, 8] {
            app.pull_request.selected_pull_request_diff_line = index;
            app.toggle_selected_pull_request_hunk_collapsed();
        }
        app.pull_request.selected_pull_request_diff_line = 1;
        app
    }

    #[test]
    fn review_comment_lookups_preserve_paths_sides_and_id_order() {
        let comment = |id, path: &str, line, side, anchored| PullRequestReviewComment {
            id,
            thread_id: None,
            resolved: false,
            anchored,
            path: path.to_string(),
            line,
            side,
            body: String::new(),
            author: "reviewer".to_string(),
            created_at: None,
        };
        let mut app = App::new(Config::default());
        app.set_pull_request_review_comments(vec![
            comment(3, "b.rs", 10, ReviewSide::Right, true),
            comment(9, "a.rs", 10, ReviewSide::Right, true),
            comment(5, "a.rs", 10, ReviewSide::Left, true),
            comment(1, "a.rs", 9, ReviewSide::Right, true),
            comment(2, "a.rs", 10, ReviewSide::Right, true),
            comment(4, "a.rs", 10, ReviewSide::Right, false),
            comment(6, "a.rs", 11, ReviewSide::Right, true),
            comment(8, "c.rs", 10, ReviewSide::Right, true),
        ]);
        for (path, line, side, expected) in [
            ("a.rs", 10, ReviewSide::Right, vec![2, 9]),
            ("a.rs", 10, ReviewSide::Left, vec![5]),
            ("a.rs", 9, ReviewSide::Right, vec![1]),
            ("a.rs", 11, ReviewSide::Right, vec![6]),
            ("b.rs", 10, ReviewSide::Right, vec![3]),
            ("c.rs", 10, ReviewSide::Right, vec![8]),
            ("", 10, ReviewSide::Right, vec![]),
            ("a.rs", 12, ReviewSide::Right, vec![]),
            ("z.rs", 10, ReviewSide::Right, vec![]),
        ] {
            let ids = app
                .pull_request_comments_for_path_and_line(path, side, line)
                .iter()
                .map(|comment| comment.id)
                .collect::<Vec<_>>();
            assert_eq!(ids, expected);
        }
        assert_eq!(app.pull_request_comments_count_for_path("a.rs"), 6);
        assert_eq!(app.pull_request_comments_count_for_path("b.rs"), 1);
        assert_eq!(app.pull_request_comments_count_for_path("absent.rs"), 0);
        app.remove_pull_request_review_comment_by_id(2);
        app.update_pull_request_review_comment_body_by_id(9, "updated");
        let remaining = app.pull_request_comments_for_path_and_line("a.rs", ReviewSide::Right, 10);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, 9);
        assert_eq!(remaining[0].body, "updated");
        assert_eq!(app.pull_request_comments_count_for_path("a.rs"), 5);
        app.set_pull_request_review_comments(vec![]);
        assert!(
            app.pull_request_comments_for_path_and_line("a.rs", ReviewSide::Right, 10)
                .is_empty()
        );
        assert_eq!(app.pull_request_comments_count_for_path("a.rs"), 0);
    }

    #[test]
    fn collapsed_hunk_navigation_preserves_headers_and_file_boundaries() {
        let app = collapsed_app();
        let rows = parse_patch(Some(PATCH));
        let visible = app
            .visible_pull_request_diff_rows("example", &rows)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(visible, [0, 1, 4, 5, 6, 7, 8]);
        assert_eq!(
            app.visible_pull_request_diff_rows("other", &rows).count(),
            rows.len()
        );
        assert_eq!(
            app.last_visible_pull_request_diff_line("example", &rows),
            Some(8)
        );

        for (index, nearest, previous, next) in [
            (0, 0, None, Some(1)),
            (1, 1, Some(0), Some(4)),
            (2, 1, Some(1), Some(4)),
            (3, 1, Some(1), Some(4)),
            (4, 4, Some(1), Some(5)),
            (5, 5, Some(4), Some(6)),
            (6, 6, Some(5), Some(7)),
            (7, 7, Some(6), Some(8)),
            (8, 8, Some(7), None),
            (9, 8, Some(8), None),
            (usize::MAX, 8, Some(8), None),
        ] {
            assert_eq!(
                app.nearest_visible_pull_request_diff_line("example", &rows, index),
                nearest,
                "nearest to {index}"
            );
            assert_eq!(
                app.previous_visible_pull_request_diff_line("example", &rows, index),
                previous,
                "previous to {index}"
            );
            assert_eq!(
                app.next_visible_pull_request_diff_line("example", &rows, index),
                next,
                "next to {index}"
            );
        }
        assert_eq!(
            app.nearest_visible_pull_request_diff_line("example", &[], 0),
            0
        );
        assert_eq!(
            app.next_visible_pull_request_diff_line("example", &[], 0),
            None
        );
        assert_eq!(
            app.previous_visible_pull_request_diff_line("example", &[], 0),
            None
        );
        assert_eq!(
            app.last_visible_pull_request_diff_line("example", &[]),
            None
        );
    }

    #[test]
    fn collapsed_diff_renders_hidden_counts_and_keeps_row_clicks_aligned() {
        let mut app = collapsed_app();
        let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw(frame, &mut app))
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .chunks(160)
            .map(|cells| cells.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        for count in [0, 1, 2] {
            assert!(screen.contains(&format!("[{count} lines hidden]")));
        }
        assert!(screen.contains("visible one"));
        assert!(screen.contains("visible two"));
        assert!(!screen.contains("hidden first"));
        assert!(!screen.contains("hidden last"));
        let visible_indices = app
            .interaction
            .mouse_regions
            .iter()
            .filter_map(|region| {
                if let MouseTarget::PullRequestDiffRow(index, ReviewSide::Right) = region.target {
                    Some(index)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(visible_indices, [0, 1, 4, 5, 6, 7, 8]);
        let region = *app
            .interaction
            .mouse_regions
            .iter()
            .find(|region| region.target == MouseTarget::PullRequestDiffRow(6, ReviewSide::Right))
            .unwrap();
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: region.x,
            row: region.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.selected_pull_request_diff_line(), 6);
        assert_eq!(app.pull_request_review_side(), ReviewSide::Right);
    }
}
