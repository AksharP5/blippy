use super::*;

pub(super) fn draw_label_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    ui_status_overlay::draw_modal_background(frame, app, area, theme);
    let popup = ui_status_overlay::centered_rect(74, 76, area);
    frame.render_widget(Clear, popup);
    let shell = popup_block("Label Picker", theme);
    let popup_inner = shell.inner(popup).inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(shell, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(popup_inner);

    let filtered = app.filtered_label_indices();
    let selected_count = app.selected_labels().len();
    let total_count = app.label_options().len();
    let query_display = if app.label_query().trim().is_empty() {
        "none".to_string()
    } else {
        ellipsize(app.label_query().trim(), 56)
    };
    let header = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Edit Labels",
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(theme.text_muted)),
            Span::raw(query_display),
            Span::raw("  "),
            Span::styled("selected: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                format!("{}/{}", selected_count, total_count),
                Style::default()
                    .fg(theme.accent_success)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "Type to filter • Space toggle • Enter apply • Ctrl+u clear • Esc cancel",
            Style::default().fg(theme.text_muted),
        )),
    ]))
    .block(panel_block_with_border("Labels", theme.border_popup, theme))
    .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup));
    frame.render_widget(header, sections[0]);

    let items = if filtered.is_empty() {
        vec![ListItem::new("No labels discovered in this repo yet.")]
    } else {
        filtered
            .iter()
            .filter_map(|index| app.label_options().get(*index))
            .map(|label| {
                let checked = if app.label_option_selected(label.as_str()) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let selected = app.label_option_selected(label.as_str());
                ListItem::new(Line::from(vec![
                    Span::styled(
                        checked,
                        Style::default().fg(if selected {
                            theme.accent_success
                        } else {
                            theme.accent_primary
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        label.clone(),
                        Style::default().fg(if selected {
                            theme.text_primary
                        } else {
                            theme.text_muted
                        }),
                    ),
                ]))
            })
            .collect::<Vec<ListItem>>()
    };
    let list = List::new(items)
        .block(panel_block_with_border(
            "Available labels",
            theme.border_popup,
            theme,
        ))
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(theme.bg_selected)
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        sections[1],
        &mut list_state(selected_for_list(
            filtered
                .iter()
                .position(|index| *index == app.selected_label_option())
                .unwrap_or(0),
            filtered.len(),
        )),
    );
    let labels_inner = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = labels_inner.height as usize;
    for index in 0..filtered.len().min(max_rows) {
        let y = labels_inner.y.saturating_add(index as u16);
        app.register_mouse_region(
            MouseTarget::LabelOption(index),
            labels_inner.x,
            y,
            labels_inner.width,
            1,
        );
    }

    let selection = if app.selected_labels_csv().is_empty() {
        "selected: none".to_string()
    } else {
        format!(
            "selected: {}",
            ellipsize(app.selected_labels_csv().as_str(), 80)
        )
    };
    let footer = Paragraph::new(Text::from(vec![
        Line::from(selection),
        Line::from(vec![
            Span::styled(
                "[Apply]",
                Style::default()
                    .fg(theme.accent_success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("[Cancel]", Style::default().fg(theme.text_muted)),
        ]),
    ]))
    .style(Style::default().fg(theme.text_muted))
    .block(panel_block_with_border(
        "Selection",
        theme.border_popup,
        theme,
    ));
    frame.render_widget(footer, sections[2]);
    let footer_content = sections[2].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    app.register_mouse_region(
        MouseTarget::LabelApply,
        footer_content.x,
        footer_content.y.saturating_add(1),
        8,
        1,
    );
    app.register_mouse_region(
        MouseTarget::LabelCancel,
        footer_content.x.saturating_add(10),
        footer_content.y.saturating_add(1),
        10,
        1,
    );
}

pub(super) fn draw_assignee_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    ui_status_overlay::draw_modal_background(frame, app, area, theme);
    let popup = ui_status_overlay::centered_rect(74, 76, area);
    frame.render_widget(Clear, popup);
    let shell = popup_block("Assignee Picker", theme);
    let popup_inner = shell.inner(popup).inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(shell, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(popup_inner);

    let filtered = app.filtered_assignee_indices();
    let selected_count = app.selected_assignees().len();
    let total_count = app.assignee_options().len();
    let query_display = if app.assignee_query().trim().is_empty() {
        "none".to_string()
    } else {
        ellipsize(app.assignee_query().trim(), 56)
    };
    let header = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Edit Assignees",
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(theme.text_muted)),
            Span::raw(query_display),
            Span::raw("  "),
            Span::styled("selected: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                format!("{}/{}", selected_count, total_count),
                Style::default()
                    .fg(theme.accent_success)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "Type to filter • Space toggle • Enter apply • Ctrl+u clear • Esc cancel",
            Style::default().fg(theme.text_muted),
        )),
        Line::from(Span::styled(
            "Source: synced issues + GitHub assignable users",
            Style::default().fg(theme.text_muted),
        )),
    ]))
    .block(panel_block_with_border(
        "Assignees",
        theme.border_popup,
        theme,
    ))
    .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup));
    frame.render_widget(header, sections[0]);

    let items = if filtered.is_empty() {
        vec![ListItem::new("No assignees discovered in this repo yet.")]
    } else {
        filtered
            .iter()
            .filter_map(|index| app.assignee_options().get(*index))
            .map(|assignee| {
                let checked = if app.assignee_option_selected(assignee.as_str()) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let selected = app.assignee_option_selected(assignee.as_str());
                ListItem::new(Line::from(vec![
                    Span::styled(
                        checked,
                        Style::default().fg(if selected {
                            theme.accent_success
                        } else {
                            theme.accent_primary
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        assignee.clone(),
                        Style::default().fg(if selected {
                            theme.text_primary
                        } else {
                            theme.text_muted
                        }),
                    ),
                ]))
            })
            .collect::<Vec<ListItem>>()
    };
    let list = List::new(items)
        .block(panel_block_with_border(
            "Available assignees",
            theme.border_popup,
            theme,
        ))
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(theme.bg_selected)
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        sections[1],
        &mut list_state(selected_for_list(
            filtered
                .iter()
                .position(|index| *index == app.selected_assignee_option())
                .unwrap_or(0),
            filtered.len(),
        )),
    );
    let assignees_inner = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = assignees_inner.height as usize;
    for index in 0..filtered.len().min(max_rows) {
        let y = assignees_inner.y.saturating_add(index as u16);
        app.register_mouse_region(
            MouseTarget::AssigneeOption(index),
            assignees_inner.x,
            y,
            assignees_inner.width,
            1,
        );
    }

    let selection = if app.selected_assignees_csv().is_empty() {
        "selected: none".to_string()
    } else {
        format!(
            "selected: {}",
            ellipsize(app.selected_assignees_csv().as_str(), 80)
        )
    };
    let footer = Paragraph::new(Text::from(vec![
        Line::from(selection),
        Line::from(vec![
            Span::styled(
                "[Apply]",
                Style::default()
                    .fg(theme.accent_success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("[Cancel]", Style::default().fg(theme.text_muted)),
        ]),
    ]))
    .style(Style::default().fg(theme.text_muted))
    .block(panel_block_with_border(
        "Selection",
        theme.border_popup,
        theme,
    ));
    frame.render_widget(footer, sections[2]);
    let footer_content = sections[2].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    app.register_mouse_region(
        MouseTarget::AssigneeApply,
        footer_content.x,
        footer_content.y.saturating_add(1),
        8,
        1,
    );
    app.register_mouse_region(
        MouseTarget::AssigneeCancel,
        footer_content.x.saturating_add(10),
        footer_content.y.saturating_add(1),
        10,
        1,
    );
}
