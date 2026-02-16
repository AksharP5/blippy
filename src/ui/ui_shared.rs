use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::*;

pub(super) fn panel_block<'a>(title: &'a str, theme: &ThemePalette) -> Block<'a> {
    panel_block_with_border(title, theme.border_panel, theme)
}

pub(super) fn popup_block<'a>(title: &'a str, theme: &ThemePalette) -> Block<'a> {
    Block::default()
        .title(Line::from(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme.border_popup)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .style(Style::default().bg(theme.bg_popup).fg(theme.text_primary))
        .border_style(Style::default().fg(theme.border_popup))
}

pub(super) fn panel_block_with_border<'a>(
    title: &'a str,
    border: Color,
    theme: &ThemePalette,
) -> Block<'a> {
    let title_color = if border == theme.border_focus {
        theme.border_focus
    } else {
        theme.accent_primary
    };
    let border_type = if border == theme.border_focus {
        BorderType::Thick
    } else {
        BorderType::Rounded
    };
    Block::default()
        .title(Line::from(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(border_type)
        .style(Style::default().bg(theme.bg_panel).fg(theme.text_primary))
        .border_style(Style::default().fg(border))
}

pub(super) fn register_mouse_region(app: &mut App, target: MouseTarget, area: Rect) {
    app.register_mouse_region(target, area.x, area.y, area.width, area.height);
}

pub(super) fn register_inline_button(
    app: &mut App,
    area: Rect,
    scroll: u16,
    line: usize,
    x_offset: u16,
    width: u16,
    target: MouseTarget,
) {
    if area.width == 0 || area.height == 0 || width == 0 {
        return;
    }
    let line = line as u16;
    if line < scroll {
        return;
    }
    let y = area.y.saturating_add(line.saturating_sub(scroll));
    if y >= area.y.saturating_add(area.height) {
        return;
    }
    let x = area.x.saturating_add(x_offset);
    if x >= area.x.saturating_add(area.width) {
        return;
    }
    let max_width = area.width.saturating_sub(x_offset);
    if max_width == 0 {
        return;
    }
    app.register_mouse_region(target, x, y, width.min(max_width), 1);
}

pub(super) fn fit_inline(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if value.chars().count() <= max {
        return value.to_string();
    }
    ellipsize(value, max)
}

pub(super) fn list_state(selected: usize) -> ListState {
    let mut state = ListState::default();
    state.select(Some(selected));
    state
}

pub(super) fn selected_for_list(selected: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    selected.min(len - 1)
}

pub(super) fn list_window_start(selected: usize, len: usize, viewport_items: usize) -> usize {
    if len == 0 || viewport_items == 0 {
        return 0;
    }
    let selected = selected_for_list(selected, len);
    selected.saturating_sub(viewport_items.saturating_sub(1))
}

pub(super) fn issue_tabs_line(
    filter: IssueFilter,
    open_count: usize,
    closed_count: usize,
    theme: &ThemePalette,
) -> Line<'static> {
    Line::from(vec![
        filter_tab(
            "1 Open",
            open_count,
            filter == IssueFilter::Open,
            theme.accent_success,
            theme,
        ),
        Span::raw("  "),
        filter_tab(
            "2 Closed",
            closed_count,
            filter == IssueFilter::Closed,
            theme.accent_danger,
            theme,
        ),
    ])
}

pub(super) fn filter_tab(
    label: &str,
    count: usize,
    active: bool,
    color: Color,
    theme: &ThemePalette,
) -> Span<'static> {
    let text = format!("{} ({})", label, count);
    if active {
        return Span::styled(
            format!("[{}]", text),
            Style::default()
                .fg(theme.bg_app)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        );
    }
    Span::styled(format!(" {} ", text), Style::default().fg(theme.text_muted))
}

pub(super) fn issue_state_color(state: &str, theme: &ThemePalette) -> Color {
    if state.eq_ignore_ascii_case("merged") {
        return theme.accent_merged;
    }
    if state.eq_ignore_ascii_case("closed") {
        return theme.accent_danger;
    }
    theme.accent_success
}

pub(super) fn styled_patch_line(line: &str, width: usize, theme: &ThemePalette) -> Line<'static> {
    let trimmed = ellipsize(line, width);
    if trimmed.starts_with("+++") || trimmed.starts_with("---") {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default()
                .fg(theme.border_focus)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if trimmed.starts_with("@@") {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default()
                .fg(theme.border_popup)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if trimmed.starts_with('+') {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default().fg(theme.accent_success),
        ));
    }
    if trimmed.starts_with('-') {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default().fg(theme.accent_danger),
        ));
    }
    Line::from(Span::styled(
        format!("  {}", trimmed),
        Style::default().fg(theme.text_muted),
    ))
}

pub(super) fn split_diff_horizontal_limit(
    rows: &[crate::pr_diff::DiffRow],
    left_width: usize,
    right_width: usize,
) -> usize {
    let left_content_width = left_width.saturating_sub(5);
    let right_content_width = right_width.saturating_sub(5);
    let hunk_width = left_width + right_width + 4;

    let mut max_offset = 0usize;
    for row in rows {
        if matches!(row.kind, DiffKind::Hunk | DiffKind::Meta) {
            let raw_width = row.raw.chars().count();
            max_offset = max_offset.max(raw_width.saturating_sub(hunk_width));
            continue;
        }
        let left = row.left.chars().count().saturating_sub(left_content_width);
        let right = row
            .right
            .chars()
            .count()
            .saturating_sub(right_content_width);
        max_offset = max_offset.max(left.max(right));
    }

    max_offset
}

pub(super) struct DiffRowContext {
    pub(super) selected: bool,
    pub(super) in_visual_range: bool,
    pub(super) selected_side: ReviewSide,
    pub(super) left_width: usize,
    pub(super) right_width: usize,
    pub(super) horizontal_offset: usize,
}

pub(super) struct CommentContext {
    pub(super) side: ReviewSide,
    pub(super) resolved: bool,
    pub(super) width: usize,
    pub(super) left_width: usize,
    pub(super) right_width: usize,
    pub(super) selected: bool,
}

pub(super) fn render_split_diff_row(
    row: &crate::pr_diff::DiffRow,
    ctx: &DiffRowContext,
    theme: &ThemePalette,
) -> Line<'static> {
    if row.kind == DiffKind::Hunk {
        return Line::from(Span::styled(
            format!(
                " {}",
                clip_horizontal(
                    row.raw.as_str(),
                    ctx.horizontal_offset,
                    ctx.left_width + ctx.right_width + 4
                )
            ),
            Style::default()
                .fg(theme.border_popup)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if row.kind == DiffKind::Meta {
        return Line::from(Span::styled(
            format!(
                " {}",
                clip_horizontal(
                    row.raw.as_str(),
                    ctx.horizontal_offset,
                    ctx.left_width + ctx.right_width + 4
                )
            ),
            Style::default().fg(theme.text_muted),
        ));
    }

    let left_number = row
        .old_line
        .map(|line| line.to_string())
        .unwrap_or_default();
    let right_number = row
        .new_line
        .map(|line| line.to_string())
        .unwrap_or_default();

    let left_prefix = format!("{:>4} ", left_number);
    let right_prefix = format!("{:>4} ", right_number);
    let left_text = clip_horizontal(
        row.left.as_str(),
        ctx.horizontal_offset,
        ctx.left_width.saturating_sub(5),
    );
    let right_text = clip_horizontal(
        row.right.as_str(),
        ctx.horizontal_offset,
        ctx.right_width.saturating_sub(5),
    );

    let mut left_style = Style::default().fg(theme.text_muted);
    let mut right_style = Style::default().fg(theme.text_muted);
    match row.kind {
        DiffKind::Changed => {
            left_style = Style::default().fg(theme.accent_danger);
            right_style = Style::default().fg(theme.accent_success);
        }
        DiffKind::Added => {
            right_style = Style::default().fg(theme.accent_success);
        }
        DiffKind::Removed => {
            left_style = Style::default().fg(theme.accent_danger);
        }
        DiffKind::Context => {
            left_style = Style::default().fg(theme.text_primary);
            right_style = Style::default().fg(theme.text_primary);
        }
        _ => {}
    }

    let mut row_style = Style::default();
    let mut bg_color = None;
    if ctx.in_visual_range {
        bg_color = Some(theme.bg_visual_range);
        row_style = Style::default().bg(theme.bg_visual_range);
    }
    if ctx.selected {
        bg_color = Some(theme.bg_selected);
        row_style = Style::default()
            .bg(theme.bg_selected)
            .add_modifier(Modifier::BOLD);
        if ctx.selected_side == ReviewSide::Left {
            left_style = left_style.add_modifier(Modifier::BOLD);
        } else {
            right_style = right_style.add_modifier(Modifier::BOLD);
        }
    }
    if let Some(bg) = bg_color {
        left_style = left_style.bg(bg);
        right_style = right_style.bg(bg);
    }

    let left_cell = format!("{}{}", left_prefix, left_text);
    let right_cell = format!("{}{}", right_prefix, right_text);
    let left_cell = format!("{:width$}", left_cell, width = ctx.left_width);
    let right_cell = format!("{:width$}", right_cell, width = ctx.right_width);

    let indicator = if ctx.selected {
        match ctx.selected_side {
            ReviewSide::Left => "L",
            ReviewSide::Right => "R",
        }
    } else if ctx.in_visual_range {
        "V"
    } else {
        " "
    };
    let divider = if ctx.selected {
        match ctx.selected_side {
            ReviewSide::Left => "<| ",
            ReviewSide::Right => " |>",
        }
    } else {
        " | "
    };

    let mut line = Line::from(vec![
        Span::styled(
            format!("{} ", indicator),
            match bg_color {
                Some(bg) => Style::default()
                    .fg(theme.border_popup)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
                None => Style::default()
                    .fg(theme.border_popup)
                    .add_modifier(Modifier::BOLD),
            },
        ),
        Span::styled(left_cell, left_style),
        Span::styled(
            divider,
            match bg_color {
                Some(bg) => Style::default().fg(theme.border_panel).bg(bg),
                None => Style::default().fg(theme.border_panel),
            },
        ),
        Span::styled(right_cell, right_style),
    ]);

    if ctx.selected || ctx.in_visual_range {
        line = line.style(row_style);
    }
    line
}

pub(super) fn render_inline_review_comment(
    author: &str,
    body: &str,
    ctx: &CommentContext,
    theme: &ThemePalette,
) -> Line<'static> {
    let side_label = match ctx.side {
        ReviewSide::Left => "old",
        ReviewSide::Right => "new",
    };
    let prefix = if ctx.selected { ">" } else { " " };
    let resolved_label = if ctx.resolved { "resolved" } else { "open" };
    let body_preview = if ctx.resolved && !ctx.selected {
        format!(
            "(collapsed) {}",
            ellipsize(body, ctx.width.saturating_sub(38).max(16))
        )
    } else {
        ellipsize(body, ctx.width.saturating_sub(24))
    };
    let text = format!(
        "{} [{} {} @{}] {}",
        prefix, side_label, resolved_label, author, body_preview
    );

    let muted_left = " ".repeat(ctx.left_width);
    let muted_right = " ".repeat(ctx.right_width);
    let comment_width = ctx.width.saturating_sub(8);
    let text = ellipsize(text.as_str(), comment_width);
    let comment_style = Style::default()
        .fg(theme.border_popup)
        .bg(theme.bg_panel_alt);
    if ctx.side == ReviewSide::Left {
        let left_text = format!("{:width$}", text, width = ctx.left_width);
        Line::from(vec![
            Span::styled(left_text, comment_style),
            Span::styled(" | ", Style::default().fg(theme.border_panel)),
            Span::styled(muted_right, Style::default().fg(theme.text_muted)),
        ])
    } else {
        let right_text = format!("{:width$}", text, width = ctx.right_width);
        Line::from(vec![
            Span::styled(muted_left, Style::default().fg(theme.text_muted)),
            Span::styled(" | ", Style::default().fg(theme.border_panel)),
            Span::styled(right_text, comment_style),
        ])
    }
}

pub(super) fn file_status_symbol(status: &str) -> &'static str {
    if status.eq_ignore_ascii_case("added") {
        return "+";
    }
    if status.eq_ignore_ascii_case("removed") {
        return "-";
    }
    if status.eq_ignore_ascii_case("renamed") {
        return "R";
    }
    if status.eq_ignore_ascii_case("modified") {
        return "M";
    }
    "*"
}

pub(super) fn file_status_color(status: &str, theme: &ThemePalette) -> Color {
    if status.eq_ignore_ascii_case("added") {
        return theme.accent_success;
    }
    if status.eq_ignore_ascii_case("removed") {
        return theme.accent_danger;
    }
    if status.eq_ignore_ascii_case("renamed") {
        return theme.accent_primary;
    }
    if status.eq_ignore_ascii_case("modified") {
        return theme.accent_subtle;
    }
    theme.text_muted
}

pub(super) fn pending_issue_span(pending: Option<&str>, theme: &ThemePalette) -> Span<'static> {
    match pending {
        Some(label) => Span::styled(
            format!("  [{}]", label),
            Style::default()
                .fg(theme.accent_subtle)
                .add_modifier(Modifier::BOLD),
        ),
        None => Span::raw(String::new()),
    }
}

pub(super) fn label_chip_spans(
    app: &App,
    labels_csv: &str,
    max_labels: usize,
    theme: &ThemePalette,
) -> Vec<Span<'static>> {
    let labels = labels_csv
        .split(',')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .collect::<Vec<&str>>();
    if labels.is_empty() {
        return vec![Span::styled("none", Style::default().fg(theme.text_muted))];
    }

    let mut spans = Vec::new();
    for (index, label) in labels.iter().take(max_labels).enumerate() {
        let (background, foreground) = label_chip_colors(app, label, index, theme);
        spans.push(Span::styled(
            format!(" {} ", label),
            Style::default().fg(foreground).bg(background),
        ));
        spans.push(Span::raw(" "));
    }

    let remaining = labels.len().saturating_sub(max_labels);
    if remaining > 0 {
        spans.push(Span::styled(
            format!("+{}", remaining),
            Style::default()
                .fg(theme.text_muted)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans
}

pub(super) fn label_chip_colors(
    app: &App,
    label: &str,
    index: usize,
    theme: &ThemePalette,
) -> (Color, Color) {
    if let Some((red, green, blue)) = parse_hex_color(app.repo_label_color(label)) {
        let background = Color::Rgb(red, green, blue);
        let luminance = (red as u32 * 299 + green as u32 * 587 + blue as u32 * 114) / 1000;
        let foreground = if luminance > 150 {
            Color::Black
        } else {
            Color::White
        };
        return (background, foreground);
    }

    let mut hasher = DefaultHasher::new();
    label.to_ascii_lowercase().hash(&mut hasher);
    let hash = hasher.finish() as usize;
    let background = match (hash + index) % 4 {
        0 => theme.accent_primary,
        1 => theme.accent_subtle,
        2 => theme.accent_success,
        _ => theme.border_focus,
    };
    (background, theme.bg_app)
}

pub(super) fn parse_hex_color(value: Option<&str>) -> Option<(u8, u8, u8)> {
    let value = value?.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    let parsed = u32::from_str_radix(value, 16).ok()?;
    Some(((parsed >> 16) as u8, (parsed >> 8) as u8, parsed as u8))
}

pub(super) fn wrapped_line_count(lines: &[Line<'_>], width: u16) -> usize {
    if lines.is_empty() {
        return 0;
    }
    let content_width = width.max(1) as usize;
    lines
        .iter()
        .map(|line| {
            let line_width = line
                .spans
                .iter()
                .map(|span| span.content.chars().count())
                .sum::<usize>()
                .max(1);
            line_width.div_ceil(content_width)
        })
        .sum()
}

pub(super) fn ellipsize(input: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if input.chars().count() <= max {
        return input.to_string();
    }
    input.chars().take(max).collect::<String>()
}

pub(super) fn clip_horizontal(input: &str, offset: usize, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars = input.chars().collect::<Vec<char>>();
    if chars.len() <= max && offset == 0 {
        return input.to_string();
    }
    if offset >= chars.len() {
        return String::new();
    }
    let visible = chars.iter().skip(offset).take(max).collect::<String>();
    if visible.chars().count() <= max {
        return visible;
    }
    ellipsize(visible.as_str(), max)
}

pub(super) fn comment_header(
    index: usize,
    author: &str,
    created_at: Option<&str>,
    selected: bool,
    theme: &ThemePalette,
) -> Line<'static> {
    let mut spans = Vec::new();
    if selected {
        spans.push(Span::styled(
            "▸ ",
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(
        format!("{}  {}", index, author),
        Style::default()
            .fg(if selected {
                theme.text_primary
            } else {
                theme.accent_primary
            })
            .add_modifier(Modifier::BOLD),
    ));
    if let Some(date) = format_comment_date(created_at) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(date, Style::default().fg(Color::Gray)));
    }
    Line::from(spans)
}

pub(super) fn format_comment_date(created_at: Option<&str>) -> Option<String> {
    format_datetime(created_at)
}

pub(super) fn format_datetime(value: Option<&str>) -> Option<String> {
    let raw = value?;
    if raw.len() >= 16 {
        return Some(raw[0..16].replace('T', " "));
    }
    if raw.is_empty() {
        return None;
    }
    Some(raw.to_string())
}

pub(super) fn editor_cursor_position(text: &str) -> (u16, u16) {
    let mut row = 0u16;
    let mut col = 0u16;
    for ch in text.chars() {
        if ch == '\n' {
            row = row.saturating_add(1);
            col = 0;
            continue;
        }
        col = col.saturating_add(1);
    }
    (row, col)
}
