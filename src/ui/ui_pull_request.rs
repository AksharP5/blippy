use super::*;

pub(super) fn draw_pull_request_files(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);
    let content = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let diff_expanded = app.pull_request_diff_expanded();
    let panes = if diff_expanded {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(content)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(content)
    };

    let title = match app.current_issue_row() {
        Some(issue) => format!("PR review #{}", issue.number),
        None => "PR review".to_string(),
    };
    let focused = match app.pull_request_review_focus() {
        PullRequestReviewFocus::Files => "files",
        PullRequestReviewFocus::Diff => "diff",
    };
    let horizontal_scroll = app.pull_request_diff_horizontal_scroll();
    let header = Text::from(vec![
        Line::from(Span::styled(
            title.clone(),
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                "[Back]",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "[Files]",
                if focused == "files" {
                    Style::default()
                        .fg(theme.accent_success)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_muted)
                },
            ),
            Span::raw("  "),
            Span::styled(
                "[Diff]",
                if focused == "diff" {
                    Style::default()
                        .fg(theme.accent_success)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_muted)
                },
            ),
            Span::raw("  "),
            Span::styled(
                if diff_expanded {
                    "[Expanded]"
                } else {
                    "[Split]"
                },
                if diff_expanded {
                    Style::default()
                        .fg(theme.accent_success)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_muted)
                },
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "pan:{}/{}",
                    horizontal_scroll,
                    app.pull_request_diff_horizontal_max()
                ),
                Style::default().fg(theme.text_muted),
            ),
        ]),
        Line::from(Span::styled(
            pull_request_header_hint(app),
            Style::default().fg(theme.text_muted),
        )),
    ]);
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_panel))
        .style(Style::default().bg(theme.bg_panel));
    let header_area = sections[0].inner(Margin {
        vertical: 0,
        horizontal: 2,
    });
    frame.render_widget(
        Paragraph::new(header)
            .block(header_block)
            .style(Style::default().fg(theme.text_primary)),
        header_area,
    );
    let header_content = header_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    app.register_mouse_region(
        MouseTarget::Back,
        header_content.x,
        header_content.y.saturating_add(1),
        8,
        1,
    );
    app.register_mouse_region(
        MouseTarget::PullRequestFocusFiles,
        header_content.x.saturating_add(9),
        header_content.y.saturating_add(1),
        9,
        1,
    );
    app.register_mouse_region(
        MouseTarget::PullRequestFocusDiff,
        header_content.x.saturating_add(20),
        header_content.y.saturating_add(1),
        8,
        1,
    );

    let file_items = if app.pull_request_files().is_empty() {
        vec![ListItem::new(
            "No changed files cached yet. Press r to refresh.",
        )]
    } else {
        app.pull_request_files()
            .iter()
            .map(|file| {
                let comment_count =
                    app.pull_request_comments_count_for_path(file.filename.as_str());
                let viewed = app.pull_request_file_is_viewed(file.filename.as_str());
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if viewed { "✓" } else { "·" },
                        if viewed {
                            Style::default()
                                .fg(theme.accent_success)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.text_muted)
                        },
                    ),
                    Span::raw(" "),
                    Span::styled(
                        file_status_symbol(file.status.as_str()),
                        Style::default().fg(file_status_color(file.status.as_str(), theme)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        ellipsize(file.filename.as_str(), 34),
                        Style::default()
                            .fg(theme.text_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("+{} -{}", file.additions, file.deletions),
                        Style::default().fg(theme.text_muted),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("c:{}", comment_count),
                        Style::default().fg(theme.border_popup),
                    ),
                ]))
            })
            .collect::<Vec<ListItem>>()
    };
    let files_focused = app.pull_request_review_focus() == PullRequestReviewFocus::Files;
    if !diff_expanded {
        let files_block_title = ui_status_overlay::focused_title("Changed files", files_focused);
        let files_list = List::new(file_items)
            .block(panel_block_with_border(
                files_block_title.as_str(),
                ui_status_overlay::focus_border(files_focused, theme),
                theme,
            ))
            .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
            .highlight_symbol("▸ ")
            .highlight_style(
                Style::default()
                    .bg(theme.bg_selected)
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_stateful_widget(
            files_list,
            panes[0],
            &mut list_state(selected_for_list(
                app.selected_pull_request_file(),
                app.pull_request_files().len(),
            )),
        );
        register_mouse_region(app, MouseTarget::PullRequestFilesPane, panes[0]);
        let files_inner = panes[0].inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let max_file_rows = files_inner.height as usize;
        for index in 0..app.pull_request_files().len().min(max_file_rows) {
            let y = files_inner.y.saturating_add(index as u16);
            app.register_mouse_region(
                MouseTarget::PullRequestFileRow(index),
                files_inner.x,
                y,
                files_inner.width,
                1,
            );
        }
    }

    let diff_focused = app.pull_request_review_focus() == PullRequestReviewFocus::Diff;
    let diff_area = if diff_expanded { panes[0] } else { panes[1] };
    let selected_file = app
        .selected_pull_request_file_row()
        .map(|file| (file.filename.clone(), file.patch.clone()));
    let mut lines = Vec::new();
    let mut row_offsets = Vec::new();
    let mut horizontal_max = 0usize;

    if app.pull_request_files_syncing() {
        lines.push(Line::from("Loading pull request changes"));
    } else if selected_file.is_none() {
        lines.push(Line::from("Select a file to start reviewing."));
    } else {
        let (file_name, patch) = selected_file.clone().expect("selected file exists");
        let rows = parse_patch(patch.as_deref());
        if rows.is_empty() {
            lines.push(Line::from(Span::styled(
                "No textual patch available for this file.",
                Style::default().fg(theme.text_muted),
            )));
        } else {
            row_offsets = vec![None; rows.len()];
            let panel_width = diff_area.width.saturating_sub(2) as usize;
            let cells_width = panel_width.saturating_sub(2);
            let left_width = cells_width.saturating_sub(5) / 2;
            let right_width = cells_width.saturating_sub(left_width + 3);
            let horizontal_offset = app.pull_request_diff_horizontal_scroll() as usize;
            horizontal_max = split_diff_horizontal_limit(rows.as_slice(), left_width, right_width);
            let visual_range = app.pull_request_visual_range();
            for (index, row) in rows.iter().enumerate() {
                if app.pull_request_diff_row_hidden(file_name.as_str(), rows.as_slice(), index) {
                    continue;
                }
                row_offsets[index] = Some(lines.len() as u16);
                let selected = index == app.selected_pull_request_diff_line();
                let in_visual_range =
                    visual_range.is_some_and(|(start, end)| index >= start && index <= end);

                if row.kind == DiffKind::Hunk
                    && app.pull_request_hunk_is_collapsed(file_name.as_str(), index)
                {
                    let hidden_lines = app.pull_request_hunk_hidden_line_count(
                        file_name.as_str(),
                        rows.as_slice(),
                        index,
                    );
                    let indicator = if selected {
                        match app.pull_request_review_side() {
                            ReviewSide::Left => "L",
                            ReviewSide::Right => "R",
                        }
                    } else if in_visual_range {
                        "V"
                    } else {
                        "▶"
                    };
                    let mut style = Style::default()
                        .fg(theme.border_popup)
                        .add_modifier(Modifier::BOLD);
                    if in_visual_range {
                        style = style.bg(theme.bg_visual_range);
                    }
                    if selected {
                        style = style.bg(theme.bg_selected);
                    }
                    let text = format!(
                        " {} {}  [{} lines hidden]",
                        indicator,
                        clip_horizontal(
                            row.raw.as_str(),
                            horizontal_offset,
                            panel_width.saturating_sub(24)
                        ),
                        hidden_lines,
                    );
                    lines.push(Line::from(Span::styled(text, style)));
                    continue;
                }

                let ctx = DiffRowContext {
                    selected,
                    in_visual_range,
                    selected_side: app.pull_request_review_side(),
                    left_width,
                    right_width,
                    horizontal_offset,
                };
                lines.push(render_split_diff_row(row, &ctx, theme));

                let target_right = row
                    .new_line
                    .map(|line| {
                        app.pull_request_comments_for_path_and_line(
                            file_name.as_str(),
                            ReviewSide::Right,
                            line,
                        )
                    })
                    .unwrap_or_default();
                for comment in target_right {
                    let ctx = CommentContext {
                        side: ReviewSide::Right,
                        resolved: comment.resolved,
                        width: panel_width,
                        left_width,
                        right_width,
                        selected: app.selected_pull_request_review_comment_id() == Some(comment.id),
                    };
                    lines.push(render_inline_review_comment(
                        comment.author.as_str(),
                        comment.body.as_str(),
                        &ctx,
                        theme,
                    ));
                }

                let target_left = row
                    .old_line
                    .map(|line| {
                        app.pull_request_comments_for_path_and_line(
                            file_name.as_str(),
                            ReviewSide::Left,
                            line,
                        )
                    })
                    .unwrap_or_default();
                for comment in target_left {
                    let ctx = CommentContext {
                        side: ReviewSide::Left,
                        resolved: comment.resolved,
                        width: panel_width,
                        left_width,
                        right_width,
                        selected: app.selected_pull_request_review_comment_id() == Some(comment.id),
                    };
                    lines.push(render_inline_review_comment(
                        comment.author.as_str(),
                        comment.body.as_str(),
                        &ctx,
                        theme,
                    ));
                }
            }
        }
    }

    let content_width = diff_area.width.saturating_sub(2);
    let viewport_height = diff_area.height.saturating_sub(2) as usize;
    let total_lines = wrapped_line_count(&lines, content_width);
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;
    app.set_pull_request_diff_max_scroll(max_scroll);
    app.set_pull_request_diff_horizontal_max(horizontal_max.min(u16::MAX as usize) as u16);

    let selected_row_offset = row_offsets
        .get(app.selected_pull_request_diff_line())
        .and_then(|offset| *offset)
        .unwrap_or(0);
    let mut scroll = app.pull_request_diff_scroll();
    if selected_row_offset < scroll {
        scroll = selected_row_offset;
    }
    let viewport = viewport_height as u16;
    if viewport > 0 && selected_row_offset >= scroll.saturating_add(viewport) {
        scroll = selected_row_offset.saturating_sub(viewport.saturating_sub(1));
    }
    let last_visible_index = row_offsets
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, offset)| offset.map(|_| index));
    if last_visible_index.is_some_and(|index| app.selected_pull_request_diff_line() >= index) {
        scroll = max_scroll;
    }
    app.set_pull_request_diff_scroll(scroll);

    let diff_title = selected_file
        .as_ref()
        .map(|(file_name, _)| {
            format!(
                "Diff: {}  [{}] [pan {}/{} | [/] move]",
                file_name,
                if diff_expanded { "expanded" } else { "split" },
                app.pull_request_diff_horizontal_scroll(),
                app.pull_request_diff_horizontal_max(),
            )
        })
        .unwrap_or_else(|| "Diff".to_string());
    let diff_block_title = ui_status_overlay::focused_title(diff_title.as_str(), diff_focused);
    let paragraph = Paragraph::new(Text::from(lines))
        .block(panel_block_with_border(
            diff_block_title.as_str(),
            ui_status_overlay::focus_border(diff_focused, theme),
            theme,
        ))
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, diff_area);
    register_mouse_region(app, MouseTarget::PullRequestDiffPane, diff_area);
    let diff_inner = diff_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let half = diff_inner.width / 2;
    for (index, offset) in row_offsets.iter().enumerate() {
        let offset = match offset {
            Some(offset) => *offset,
            None => continue,
        };
        if offset < scroll {
            continue;
        }
        let y = diff_inner.y.saturating_add(offset.saturating_sub(scroll));
        if y >= diff_inner.y.saturating_add(diff_inner.height) {
            continue;
        }
        app.register_mouse_region(
            MouseTarget::PullRequestDiffRow(index, ReviewSide::Left),
            diff_inner.x,
            y,
            half.max(1),
            1,
        );
        app.register_mouse_region(
            MouseTarget::PullRequestDiffRow(index, ReviewSide::Right),
            diff_inner.x.saturating_add(half),
            y,
            diff_inner.width.saturating_sub(half).max(1),
            1,
        );
    }
}

fn pull_request_header_hint(app: &App) -> String {
    if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
        return "Ctrl+h/l pane • j/k files • Enter full diff • w viewed • b/Esc back".to_string();
    }

    let toggle_hint = if app.pull_request_diff_expanded() {
        "Enter or b/Esc split diff"
    } else {
        "Enter full diff"
    };
    format!(
        "Ctrl+h/l pane • j/k diff • {} • c collapse hunk • h/l side • [/ ] pan • 0 reset • m add • n/p thread • e edit • x delete • Shift+R resolve • Shift+V visual",
        toggle_hint
    )
}
