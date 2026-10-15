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
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(popup_inner);

    let filtered = app.filtered_label_indices();
    let query = if app.label_query().trim().is_empty() {
        "none"
    } else {
        app.label_query().trim()
    };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(theme.text_muted)),
            Span::raw(ellipsize(
                query,
                popup_inner.width.saturating_sub(8) as usize,
            )),
        ]),
        Line::from("Type to filter • Space toggle • Enter apply"),
        Line::from("↑/↓ move • Ctrl+u clear • Esc cancel"),
    ])
    .style(Style::default().fg(theme.text_muted).bg(theme.bg_popup));
    frame.render_widget(header, sections[0]);

    let items = if filtered.is_empty() {
        vec![ListItem::new(if app.label_options().is_empty() {
            "No labels discovered in this repo yet."
        } else {
            "No labels match this filter."
        })]
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
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(theme.bg_selected)
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
    let mut labels_state = list_state(selected_for_list(
        filtered
            .iter()
            .position(|index| *index == app.selected_label_option())
            .unwrap_or(0),
        filtered.len(),
    ));
    frame.render_stateful_widget(list, sections[1], &mut labels_state);
    let labels_inner = sections[1];
    let max_rows = labels_inner.height as usize;
    for (row, index) in (labels_state.offset()..filtered.len())
        .take(max_rows)
        .enumerate()
    {
        let y = labels_inner.y.saturating_add(row as u16);
        app.register_mouse_region(
            MouseTarget::LabelOption(index),
            labels_inner.x,
            y,
            labels_inner.width,
            1,
        );
    }

    draw_picker_footer(
        frame,
        app,
        sections[2],
        theme,
        app.selected_labels_csv(),
        (MouseTarget::LabelApply, MouseTarget::LabelCancel),
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
    let query = if app.assignee_query().trim().is_empty() {
        "none"
    } else {
        app.assignee_query().trim()
    };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(theme.text_muted)),
            Span::raw(ellipsize(
                query,
                popup_inner.width.saturating_sub(8) as usize,
            )),
        ]),
        Line::from("Type to filter • Space toggle • Enter apply"),
        Line::from("↑/↓ move • Ctrl+u clear • Esc cancel"),
    ])
    .style(Style::default().fg(theme.text_muted).bg(theme.bg_popup));
    frame.render_widget(header, sections[0]);

    let items = if filtered.is_empty() {
        vec![ListItem::new(if app.assignee_options().is_empty() {
            "No assignees discovered in this repo yet."
        } else {
            "No assignees match this filter."
        })]
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
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(theme.bg_selected)
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
    let mut assignees_state = list_state(selected_for_list(
        filtered
            .iter()
            .position(|index| *index == app.selected_assignee_option())
            .unwrap_or(0),
        filtered.len(),
    ));
    frame.render_stateful_widget(list, sections[1], &mut assignees_state);
    let assignees_inner = sections[1];
    let max_rows = assignees_inner.height as usize;
    for (row, index) in (assignees_state.offset()..filtered.len())
        .take(max_rows)
        .enumerate()
    {
        let y = assignees_inner.y.saturating_add(row as u16);
        app.register_mouse_region(
            MouseTarget::AssigneeOption(index),
            assignees_inner.x,
            y,
            assignees_inner.width,
            1,
        );
    }

    draw_picker_footer(
        frame,
        app,
        sections[2],
        theme,
        app.selected_assignees_csv(),
        (MouseTarget::AssigneeApply, MouseTarget::AssigneeCancel),
    );
}

fn draw_picker_footer(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    theme: &ThemePalette,
    selection: String,
    targets: (MouseTarget, MouseTarget),
) {
    let selection = if selection.is_empty() {
        "none"
    } else {
        &selection
    };
    let apply = Span::styled(
        "[Apply]",
        Style::default()
            .fg(theme.accent_success)
            .add_modifier(Modifier::BOLD),
    );
    let gap = Span::raw("  ");
    let cancel = Span::styled("[Cancel]", Style::default().fg(theme.text_muted));
    let apply_width = apply.width() as u16;
    let cancel_x = area.x + apply_width + gap.width() as u16;
    let cancel_width = cancel.width() as u16;
    let footer = Paragraph::new(vec![
        Line::default(),
        Line::from(format!(
            "selected: {}",
            ellipsize(selection, area.width.saturating_sub(10) as usize)
        )),
        Line::from(vec![apply, gap, cancel]),
    ])
    .style(Style::default().fg(theme.text_muted).bg(theme.bg_popup));
    frame.render_widget(footer, area);
    if area.height >= 3 {
        app.register_mouse_region(
            targets.0,
            area.x,
            area.y + 2,
            apply_width.min(area.width),
            1,
        );
        app.register_mouse_region(
            targets.1,
            cancel_x,
            area.y + 2,
            cancel_width.min(area.right().saturating_sub(cancel_x)),
            1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::AppAction, config::Config};
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn picker_controls_are_visible_and_buttons_match_their_text() {
        for (width, height) in [(80, 24), (120, 36)] {
            for assignees in [false, true] {
                let mut app = App::new(Config::default());
                let options = vec!["bug".to_string(), "docs".to_string()];
                if assignees {
                    app.open_assignee_picker(View::RepoPicker, options, "bug");
                } else {
                    app.open_label_picker(View::RepoPicker, options, &["bug".to_string()]);
                }
                let view = app.view();
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| crate::ui::draw(frame, &mut app))
                    .unwrap();
                let rows = terminal
                    .backend()
                    .buffer()
                    .content
                    .chunks(width as usize)
                    .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
                    .collect::<Vec<_>>();
                let rendered = rows.join("\n");
                for text in [
                    "filter: none",
                    "Type to filter",
                    "Space toggle",
                    "Enter apply",
                    "Ctrl+u clear",
                    "Esc cancel",
                    "[x] bug",
                    "[ ] docs",
                    "selected: bug",
                    "[Apply]  [Cancel]",
                ] {
                    assert!(
                        rendered.contains(text),
                        "missing {text} at {width}x{height}:\n{rendered}"
                    );
                }
                let (row, text) = rows
                    .iter()
                    .enumerate()
                    .find(|(_, text)| text.contains("[Apply]"))
                    .unwrap();
                let column = text[..text.find("[Apply]").unwrap()].chars().count() as u16;
                let submit = if assignees {
                    AppAction::SubmitAssignees
                } else {
                    AppAction::SubmitLabels
                };
                // Each displayed cell is clickable; the surrounding space is not.
                for offset in -1..=17 {
                    app.set_view(view);
                    app.on_mouse(MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column: column.saturating_add_signed(offset),
                        row: row as u16,
                        modifiers: KeyModifiers::NONE,
                    });
                    assert_eq!(
                        app.take_action(),
                        (0..7).contains(&offset).then_some(submit)
                    );
                    assert_eq!(
                        app.view(),
                        if (9..17).contains(&offset) {
                            View::RepoPicker
                        } else {
                            view
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn picker_distinguishes_no_matches_from_no_options() {
        for assignees in [false, true] {
            for empty in [false, true] {
                let mut app = App::new(Config::default());
                let options = if empty {
                    vec![]
                } else {
                    vec!["bug".to_string()]
                };
                if assignees {
                    app.open_assignee_picker(View::RepoPicker, options, "");
                } else {
                    app.open_label_picker(View::RepoPicker, options, &[]);
                }
                app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
                let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
                terminal
                    .draw(|frame| crate::ui::draw(frame, &mut app))
                    .unwrap();
                let rendered = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(rendered.contains(if empty {
                    "discovered in this repo"
                } else {
                    "match this filter"
                }));
            }
        }
    }
}
