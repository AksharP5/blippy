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
    if app.editor_mode() == EditorMode::CreateIssue {
        draw_create_issue_editor(frame, app, area, theme);
        return;
    }

    let close_editor_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Close Pull Request Comment"
    } else {
        "Close Issue Comment"
    };
    let add_editor_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Add Pull Request Comment"
    } else {
        "Add Issue Comment"
    };
    let edit_editor_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Edit Pull Request Comment"
    } else {
        "Edit Issue Comment"
    };
    let title = match app.editor_mode() {
        EditorMode::CloseIssue => close_editor_title,
        EditorMode::CreateIssue => "Create Issue",
        EditorMode::AddComment => add_editor_title,
        EditorMode::EditComment => edit_editor_title,
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

fn draw_create_issue_editor(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let editor_area = area.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });

    let outer_block = panel_block("Create Issue", theme);
    frame.render_widget(outer_block, editor_area);

    let content_area = editor_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(content_area);

    let title_focused = app.editor().create_issue_title_focused();
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title("Title")
        .border_style(if title_focused {
            Style::default().fg(theme.border_focus)
        } else {
            Style::default().fg(theme.border_panel)
        });
    let title = Paragraph::new(app.editor().name())
        .block(title_block)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel));
    frame.render_widget(title, sections[0]);

    let body_block = Block::default()
        .borders(Borders::ALL)
        .title("Body")
        .border_style(if title_focused {
            Style::default().fg(theme.border_panel)
        } else {
            Style::default().fg(theme.border_focus)
        });
    let body = Paragraph::new(app.editor().text())
        .block(body_block)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .wrap(Wrap { trim: false });
    frame.render_widget(body, sections[1]);

    if title_focused {
        let title_inner = sections[0].inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        if title_inner.width > 0 {
            let cursor_x = title_inner
                .x
                .saturating_add(app.editor().name().chars().count() as u16)
                .min(
                    title_inner
                        .x
                        .saturating_add(title_inner.width.saturating_sub(1)),
                );
            frame.set_cursor_position((cursor_x, title_inner.y));
        }
        if app.editor().create_issue_confirm_visible() {
            draw_create_issue_confirm(frame, app, area, theme);
        }
        return;
    }

    let body_inner = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if body_inner.width > 0 && body_inner.height > 0 {
        let (row, col) = editor_cursor_position(app.editor().text());
        let cursor_y = body_inner
            .y
            .saturating_add(row.min(body_inner.height.saturating_sub(1)));
        let cursor_x = body_inner
            .x
            .saturating_add(col.min(body_inner.width.saturating_sub(1)));
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    if app.editor().create_issue_confirm_visible() {
        draw_create_issue_confirm(frame, app, area, theme);
    }
}

fn draw_create_issue_confirm(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let popup = ui_status_overlay::centered_rect(52, 28, area);
    frame.render_widget(Clear, popup);
    let block = popup_block("Create this issue?", theme);
    frame.render_widget(block, popup);

    let content = popup.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let title = app.editor().name().trim();
    let title = if title.is_empty() {
        "(untitled)".to_string()
    } else {
        fit_inline(title, content.width.saturating_sub(2) as usize)
    };
    let prompt = Line::from(vec![
        Span::styled("Title: ", Style::default().fg(theme.text_muted)),
        Span::styled(title, Style::default().fg(theme.text_primary)),
    ]);
    frame.render_widget(
        Paragraph::new(prompt).style(Style::default().bg(theme.bg_popup)),
        Rect {
            x: content.x,
            y: content.y,
            width: content.width,
            height: 1,
        },
    );

    let submit_selected = app.editor().create_issue_confirm_submit_selected();
    let cancel_style = if submit_selected {
        Style::default().fg(theme.text_muted)
    } else {
        Style::default()
            .fg(theme.bg_app)
            .bg(theme.accent_danger)
            .add_modifier(Modifier::BOLD)
    };
    let create_style = if submit_selected {
        Style::default()
            .fg(theme.bg_app)
            .bg(theme.accent_success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };

    let actions = Line::from(vec![
        Span::styled("[ Cancel ]", cancel_style),
        Span::raw("  "),
        Span::styled("[ Create ]", create_style),
    ]);
    let action_y = content.y.saturating_add(content.height.saturating_sub(1));
    frame.render_widget(
        Paragraph::new(actions).style(Style::default().bg(theme.bg_popup)),
        Rect {
            x: content.x,
            y: action_y,
            width: content.width,
            height: 1,
        },
    );
}
