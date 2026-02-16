use super::*;

pub(super) fn draw_preset_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let close_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Close Pull Request"
    } else {
        "Close Issue"
    };
    let block = panel_block(close_title, theme);
    let mut items = Vec::new();
    items.push(ListItem::new("Close without comment"));
    items.push(ListItem::new("Custom message"));
    for preset in app.comment_defaults() {
        items.push(ListItem::new(preset.name.as_str()));
    }
    items.push(ListItem::new("Add preset"));

    let list = List::new(items)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(theme.bg_selected)
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
    let list_area = area.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    frame.render_stateful_widget(list, list_area, &mut list_state(app.selected_preset()));
    let list_inner = list_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = list_inner.height as usize;
    for index in 0..app.preset_items_len().min(max_rows) {
        let y = list_inner.y.saturating_add(index as u16);
        app.register_mouse_region(
            MouseTarget::PresetOption(index),
            list_inner.x,
            y,
            list_inner.width,
            1,
        );
    }
}

pub(super) fn draw_preset_name(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let input_area = area.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let block = panel_block("Preset Name", theme);
    let text = app.editor().name();
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, input_area);

    let text_area = input_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if text_area.width > 0 {
        let cursor_x = text_area
            .x
            .saturating_add(app.editor().name().chars().count() as u16)
            .min(
                text_area
                    .x
                    .saturating_add(text_area.width.saturating_sub(1)),
            );
        frame.set_cursor_position((cursor_x, text_area.y));
    }
}

pub(super) fn draw_comment_editor(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let close_editor_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Close Pull Request Comment"
    } else {
        "Close Issue Comment"
    };
    let title = match app.editor_mode() {
        EditorMode::CloseIssue => close_editor_title,
        EditorMode::AddComment => "Add Issue Comment",
        EditorMode::EditComment => "Edit Issue Comment",
        EditorMode::AddPullRequestReviewComment => "Add Pull Request Review Comment",
        EditorMode::EditPullRequestReviewComment => "Edit Pull Request Review Comment",
        EditorMode::AddPreset => "Preset Body",
    };
    let editor_area = area.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let block = panel_block(title, theme);
    let text = app.editor().text();
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, editor_area);

    let text_area = editor_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if text_area.width > 0 && text_area.height > 0 {
        let (row, col) = editor_cursor_position(app.editor().text());
        let cursor_y = text_area
            .y
            .saturating_add(row.min(text_area.height.saturating_sub(1)));
        let cursor_x = text_area
            .x
            .saturating_add(col.min(text_area.width.saturating_sub(1)));
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
