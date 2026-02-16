use super::*;

pub(super) fn draw_repo_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    let query = app.repo_query().trim();
    let query_display = if query.is_empty() {
        "none".to_string()
    } else {
        ellipsize(query, 64)
    };
    let visible_count = app.filtered_repo_rows().len();
    let total_count = app.repos().len();
    let header = Text::from(vec![
        Line::from(vec![
            Span::styled(
                "Repositories",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} shown", visible_count),
                Style::default().fg(theme.text_primary),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} total", total_count),
                Style::default().fg(theme.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("search: ", Style::default().fg(theme.text_muted)),
            Span::raw(query_display.clone()),
            Span::raw("  "),
            Span::styled("(/ to search)", Style::default().fg(theme.text_muted)),
        ]),
    ]);
    let header_area = sections[0].inner(Margin {
        vertical: 0,
        horizontal: 2,
    });
    frame.render_widget(
        Paragraph::new(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(theme.border_panel))
                    .style(Style::default().bg(theme.bg_panel)),
            )
            .style(Style::default().fg(theme.text_primary)),
        header_area,
    );
    if app.repo_search_mode() {
        let content = header_area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        if content.width > 0 && content.height > 1 {
            let cursor_x = content
                .x
                .saturating_add((8 + query_display.chars().count()) as u16)
                .min(content.x.saturating_add(content.width.saturating_sub(1)));
            let cursor_y = content.y.saturating_add(1);
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    let block = panel_block("Repositories", theme);
    let items = if app.filtered_repo_rows().is_empty() {
        if app.repos().is_empty() {
            vec![ListItem::new(
                "No repos found. Run `blippy sync` or press Ctrl+R to rescan.",
            )]
        } else {
            vec![ListItem::new(
                "No repos match current search. Press Esc to clear.",
            )]
        }
    } else {
        app.filtered_repo_rows()
            .iter()
            .map(|repo| {
                let line1 = Line::from(vec![
                    Span::styled(
                        format!("{} / {}", repo.owner, repo.repo),
                        Style::default()
                            .fg(theme.text_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        repo.remote_name.to_string(),
                        Style::default().fg(theme.text_muted),
                    ),
                ]);
                let line2 = Line::from(ellipsize(repo.path.as_str(), 96))
                    .style(Style::default().fg(theme.text_muted));
                ListItem::new(vec![line1, line2])
            })
            .collect()
    };
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
    let list_area = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    frame.render_stateful_widget(
        list,
        list_area,
        &mut list_state(selected_for_list(
            app.selected_repo(),
            app.filtered_repo_rows().len(),
        )),
    );

    register_mouse_region(app, MouseTarget::RepoListPane, list_area);
    let list_inner = list_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = (list_inner.height as usize) / 2;
    let filtered_len = app.filtered_repo_rows().len();
    let selected = selected_for_list(app.selected_repo(), filtered_len);
    let start = list_window_start(selected, filtered_len, max_rows);
    let visible = filtered_len.saturating_sub(start).min(max_rows);
    for row in 0..visible {
        let index = start + row;
        let y = list_inner.y.saturating_add((row * 2) as u16);
        app.register_mouse_region(
            MouseTarget::RepoRow(index),
            list_inner.x,
            y,
            list_inner.width,
            2,
        );
    }
}

pub(super) fn draw_remote_chooser(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let block = panel_block("Choose Remote", theme);
    let items = app
        .remotes()
        .iter()
        .map(|remote| {
            let label = format!(
                "{} -> {}/{}",
                remote.name, remote.slug.owner, remote.slug.repo
            );
            ListItem::new(label)
        })
        .collect::<Vec<ListItem>>();
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
    frame.render_stateful_widget(list, list_area, &mut list_state(app.selected_remote()));

    register_mouse_region(app, MouseTarget::RemoteListPane, list_area);
    let list_inner = list_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = list_inner.height as usize;
    let remotes_len = app.remotes().len();
    let selected = selected_for_list(app.selected_remote(), remotes_len);
    let start = list_window_start(selected, remotes_len, max_rows);
    let visible = remotes_len.saturating_sub(start).min(max_rows);
    for row in 0..visible {
        let index = start + row;
        let y = list_inner.y.saturating_add(row as u16);
        app.register_mouse_region(
            MouseTarget::RemoteRow(index),
            list_inner.x,
            y,
            list_inner.width,
            1,
        );
    }
}
