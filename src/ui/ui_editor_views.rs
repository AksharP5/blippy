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
    let mut presets_state = list_state(app.selected_preset());
    frame.render_stateful_widget(list, list_area, &mut presets_state);
    let list_inner = list_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = list_inner.height as usize;
    for (row, index) in (presets_state.offset()..app.preset_items_len())
        .take(max_rows)
        .enumerate()
    {
        let y = list_inner.y.saturating_add(row as u16);
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
    frame.render_widget(block, input_area);

    let text_area = input_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    draw_editor_input(frame, app.editor().name(), text_area, theme, false);
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
    frame.render_widget(block, editor_area);

    let text_area = editor_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    draw_editor_input(frame, app.editor().text(), text_area, theme, true);
}

fn draw_editor_input(
    frame: &mut Frame<'_>,
    text: &str,
    area: Rect,
    theme: &ThemePalette,
    wrap: bool,
) {
    if area.is_empty() {
        return;
    }
    let mut content = Text::from(text.split('\n').map(Line::from).collect::<Vec<_>>());
    // Render a temporary caret so wrapping and Unicode use the widget's own layout.
    let caret = Span::styled("█", Style::default().add_modifier(Modifier::REVERSED));
    if let Some(line) = content.lines.last_mut() {
        line.spans.push(caret);
    } else {
        content.lines.push(Line::from(caret));
    }
    let paragraph =
        Paragraph::new(content).style(Style::default().fg(theme.text_primary).bg(theme.bg_panel));
    let (paragraph, scroll) = if wrap {
        let paragraph = paragraph.wrap(Wrap { trim: false });
        let scroll = paragraph
            .line_count(area.width)
            .saturating_sub(area.height as usize);
        (paragraph, (scroll.min(u16::MAX as usize) as u16, 0))
    } else {
        let scroll = paragraph.line_width().saturating_sub(area.width as usize);
        (paragraph, (0, scroll.min(u16::MAX as usize) as u16))
    };
    frame.render_widget(paragraph.scroll(scroll), area);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            if cell.modifier.contains(Modifier::REVERSED) {
                cell.set_symbol(" ");
                cell.modifier.remove(Modifier::REVERSED);
                frame.set_cursor_position((x, y));
                return;
            }
        }
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
    frame.render_widget(title_block, sections[0]);

    let body_block = Block::default()
        .borders(Borders::ALL)
        .title("Body")
        .border_style(if title_focused {
            Style::default().fg(theme.border_panel)
        } else {
            Style::default().fg(theme.border_focus)
        });
    frame.render_widget(body_block, sections[1]);
    let title_inner = sections[0].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let body_inner = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let text_style = Style::default().fg(theme.text_primary).bg(theme.bg_panel);

    if title_focused {
        frame.render_widget(
            Paragraph::new(app.editor().text())
                .style(text_style)
                .wrap(Wrap { trim: false }),
            body_inner,
        );
        draw_editor_input(frame, app.editor().name(), title_inner, theme, false);
    } else {
        frame.render_widget(
            Paragraph::new(app.editor().name()).style(text_style),
            title_inner,
        );
        draw_editor_input(frame, app.editor().text(), body_inner, theme, true);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn comment_editor_cursor_uses_unicode_display_width() {
        for (text, width, row) in [("界abc", 5, 2), ("e\u{301}abc", 4, 2), ("first\n", 0, 3)] {
            let mut app = App::new(Config::default());
            app.open_comment_edit_editor(View::Issues, 1, text);
            let mut terminal = Terminal::new(TestBackend::new(40, 8)).expect("test terminal");
            terminal
                .draw(|frame| {
                    draw_comment_editor(frame, &mut app, frame.area(), resolve_theme(None))
                })
                .expect("draw editor");
            let cursor = terminal.get_cursor_position().expect("editor cursor");
            assert_eq!((cursor.x, cursor.y), (3 + width, row));
        }
    }

    #[test]
    fn comment_editor_keeps_wrapped_input_tail_and_cursor_visible() {
        let mut app = App::new(Config::default());
        app.open_comment_edit_editor(View::Issues, 1, &format!("{}TAIL", "abcdef ".repeat(30)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).expect("test terminal");
        terminal
            .draw(|frame| draw_comment_editor(frame, &mut app, frame.area(), resolve_theme(None)))
            .expect("draw editor");
        let (row, column) = terminal
            .backend()
            .buffer()
            .content
            .chunks(40)
            .enumerate()
            .find_map(|(row, cells)| {
                let text = cells.iter().map(|cell| cell.symbol()).collect::<String>();
                text.find("TAIL")
                    .map(|column| (row as u16, text[..column].chars().count() as u16))
            })
            .expect("tail visible in rendered editor");
        let cursor = terminal.get_cursor_position().expect("editor cursor");
        assert_eq!((cursor.x, cursor.y), (column + 4, row));
    }

    #[test]
    fn issue_title_editor_keeps_long_input_tail_visible() {
        let mut app = App::new(Config::default());
        app.open_create_issue_editor(View::Issues);
        for ch in format!("{}TAIL", "x".repeat(80)).chars() {
            app.editor_mut().append_name(ch);
        }
        let mut terminal = Terminal::new(TestBackend::new(40, 14)).expect("test terminal");
        terminal
            .draw(|frame| draw_comment_editor(frame, &mut app, frame.area(), resolve_theme(None)))
            .expect("draw editor");
        assert!(terminal.backend().buffer().content.chunks(40).any(|cells| {
            cells
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .contains("TAIL")
        }));
    }
}
