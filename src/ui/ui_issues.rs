use super::*;

pub(super) fn draw_issues(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(sections[1]);

    let visible_issues = app
        .issues_for_view()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let (open_count, closed_count) = app.issue_counts();
    let item_mode = app.work_item_mode();
    let item_label = item_mode.label();
    let list_title = if item_mode == crate::app::WorkItemMode::PullRequests {
        "Pull request list"
    } else {
        "Issue list"
    };
    let preview_title_text = if item_mode == crate::app::WorkItemMode::PullRequests {
        "Pull request preview"
    } else {
        "Issue preview"
    };
    let query = app.issue_query().trim();
    let query_label = if app.issue_search_mode() {
        query.to_string()
    } else if query.is_empty() {
        "none".to_string()
    } else {
        query.to_string()
    };
    let query_display = ellipsize(query_label.as_str(), 64);
    let assignee = app.assignee_filter_label();
    let visible_count = visible_issues.len();
    let total_count = open_count + closed_count;
    let header_text = Text::from(vec![
        issue_tabs_line(app.issue_filter(), open_count, closed_count, theme),
        Line::from(vec![
            Span::styled("mode: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                item_label,
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            Span::raw("  "),
            Span::styled("(p toggle)", Style::default().fg(theme.text_muted)),
            Span::raw("  "),
            Span::styled("assignee: ", Style::default().fg(theme.text_muted)),
            if app.has_assignee_filter() {
                Span::styled(
                    assignee.clone(),
                    Style::default()
                        .fg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )
            } else {
                Span::styled(assignee.clone(), Style::default().fg(theme.text_muted))
            },
            Span::raw("  "),
            Span::styled("(a cycle)", Style::default().fg(theme.text_muted)),
            Span::raw("  "),
            Span::styled(
                format!("showing {} of {}", visible_count, total_count),
                Style::default().fg(theme.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("search: ", Style::default().fg(theme.text_muted)),
            Span::raw(query_display.clone()),
        ]),
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
        Paragraph::new(header_text)
            .block(header_block)
            .style(Style::default().fg(theme.text_primary)),
        header_area,
    );
    let header_content = header_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let open_label = format!("1 Open ({})", open_count);
    let closed_label = format!("2 Closed ({})", closed_count);
    app.register_mouse_region(
        MouseTarget::IssueTabOpen,
        header_content.x,
        header_content.y,
        (open_label.len() as u16).saturating_add(3),
        1,
    );
    let closed_x = header_content
        .x
        .saturating_add((open_label.len() as u16).saturating_add(5));
    app.register_mouse_region(
        MouseTarget::IssueTabClosed,
        closed_x,
        header_content.y,
        (closed_label.len() as u16).saturating_add(3),
        1,
    );
    if app.issue_search_mode() {
        let content = header_area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        if content.width > 0 && content.height > 2 {
            let cursor_x = content
                .x
                .saturating_add((8 + query_display.chars().count()) as u16)
                .min(content.x.saturating_add(content.width.saturating_sub(1)));
            let cursor_y = content.y.saturating_add(2);
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    let list_focused = app.focus() == Focus::IssuesList;
    let preview_focused = app.focus() == Focus::IssuesPreview;
    let list_block_title = ui_status_overlay::focused_title(list_title, list_focused);
    let block = panel_block_with_border(
        list_block_title.as_str(),
        ui_status_overlay::focus_border(list_focused, theme),
        theme,
    );
    let items = if visible_issues.is_empty() {
        if app.issues().is_empty() {
            let message = if item_mode == crate::app::WorkItemMode::PullRequests {
                "No cached pull requests yet. Press r to sync."
            } else {
                "No cached issues yet. Press r to sync."
            };
            vec![ListItem::new(message)]
        } else {
            let message = if item_mode == crate::app::WorkItemMode::PullRequests {
                "No pull requests match current filter."
            } else {
                "No issues match current filter."
            };
            vec![ListItem::new(message)]
        }
    } else {
        visible_issues
            .iter()
            .map(|issue| {
                let assignees = if issue.assignees.is_empty() {
                    "unassigned"
                } else {
                    issue.assignees.as_str()
                };
                let labels = if issue.labels.is_empty() {
                    "none"
                } else {
                    issue.labels.as_str()
                };
                let line1_spans = vec![
                    Span::styled(
                        if issue.is_pr {
                            format!("PR #{} ", issue.number)
                        } else {
                            format!("#{} ", issue.number)
                        },
                        Style::default()
                            .fg(theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("[{}] ", issue.state),
                        Style::default().fg(issue_state_color(issue.state.as_str(), theme)),
                    ),
                    Span::styled(
                        ellipsize(issue.title.as_str(), 60),
                        Style::default().fg(theme.text_primary),
                    ),
                    pending_issue_span(app.pending_issue_badge(issue.number), theme),
                ];
                let line1 = Line::from(line1_spans);
                let mut line2_spans = Vec::new();
                if issue.is_pr {
                    if let Some(linked_issue) = app.linked_issue_for_pull_request(issue.number) {
                        line2_spans.push(Span::styled(
                            "I:",
                            Style::default()
                                .fg(theme.accent_subtle)
                                .add_modifier(Modifier::BOLD),
                        ));
                        line2_spans.push(Span::styled(
                            format!("#{}", linked_issue),
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_subtle)
                                .add_modifier(Modifier::BOLD),
                        ));
                        line2_spans.push(Span::raw("  "));
                    }
                } else if let Some(linked_pr) = app.linked_pull_request_for_issue(issue.number) {
                    line2_spans.push(Span::styled(
                        "PR:",
                        Style::default()
                            .fg(theme.accent_success)
                            .add_modifier(Modifier::BOLD),
                    ));
                    line2_spans.push(Span::styled(
                        format!("#{}", linked_pr),
                        Style::default()
                            .fg(theme.bg_app)
                            .bg(theme.accent_success)
                            .add_modifier(Modifier::BOLD),
                    ));
                    line2_spans.push(Span::raw("  "));
                }
                line2_spans.push(Span::styled(
                    "A:",
                    Style::default()
                        .fg(theme.accent_subtle)
                        .add_modifier(Modifier::BOLD),
                ));
                line2_spans.push(Span::styled(
                    ellipsize(assignees, 20),
                    Style::default().fg(theme.text_muted),
                ));
                line2_spans.push(Span::raw("  "));
                line2_spans.push(Span::styled(
                    "C:",
                    Style::default()
                        .fg(theme.accent_success)
                        .add_modifier(Modifier::BOLD),
                ));
                line2_spans.push(Span::styled(
                    issue.comments_count.to_string(),
                    Style::default().fg(theme.text_muted),
                ));
                line2_spans.push(Span::raw("  "));
                line2_spans.push(Span::styled(
                    "L:",
                    Style::default()
                        .fg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                ));
                line2_spans.extend(label_chip_spans(app, labels, 2, theme));
                let line2 = Line::from(line2_spans);
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
    let issues_list_area = panes[0].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    frame.render_stateful_widget(
        list,
        issues_list_area,
        &mut list_state(selected_for_list(
            app.selected_issue(),
            visible_issues.len(),
        )),
    );
    register_mouse_region(app, MouseTarget::IssuesListPane, issues_list_area);
    let issues_list_inner = issues_list_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let max_rows = (issues_list_inner.height as usize) / 2;
    for index in 0..visible_issues.len().min(max_rows) {
        let y = issues_list_inner.y.saturating_add((index * 2) as u16);
        app.register_mouse_region(
            MouseTarget::IssueRow(index),
            issues_list_inner.x,
            y,
            issues_list_inner.width,
            2,
        );
    }

    let (
        preview_title,
        preview_lines,
        linked_pr_tui_button,
        linked_pr_web_button,
        linked_issue_tui_button,
        linked_issue_web_button,
    ) = match app.selected_issue_row() {
        Some(issue) => {
            let assignees = if issue.assignees.is_empty() {
                "unassigned".to_string()
            } else {
                issue.assignees.clone()
            };
            let labels = if issue.labels.is_empty() {
                "none".to_string()
            } else {
                issue.labels.clone()
            };
            let mut lines = Vec::new();
            lines.push(Line::from(vec![
                Span::styled(
                    if issue.is_pr {
                        format!("PR #{}", issue.number)
                    } else {
                        format!("#{}", issue.number)
                    },
                    Style::default()
                        .fg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", issue.state),
                    Style::default().fg(issue_state_color(issue.state.as_str(), theme)),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    "title     ",
                    Style::default()
                        .fg(theme.accent_subtle)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(issue.title.clone(), Style::default().fg(theme.text_primary)),
            ]));
            let mut pr_tui_button_hit = None;
            let mut pr_web_button_hit = None;
            let mut issue_tui_button_hit = None;
            let mut issue_web_button_hit = None;
            let line_index = lines.len();
            if !issue.is_pr {
                let prefix = "linked PR ";
                if let Some(linked_pr) = app.linked_pull_request_for_issue(issue.number) {
                    let open_label = format!("[ PR #{} ]", linked_pr);
                    let web_label = "[ web ]";
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            open_label.clone(),
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_success)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            web_label,
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_primary)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    let prefix_width = prefix.chars().count() as u16;
                    let open_width = open_label.chars().count() as u16;
                    let web_width = web_label.chars().count() as u16;
                    pr_tui_button_hit = Some((line_index, prefix_width, open_width));
                    pr_web_button_hit = Some((
                        line_index,
                        prefix_width.saturating_add(open_width).saturating_add(1),
                        web_width,
                    ));
                }
            } else {
                let prefix = "linked issue ";
                if let Some(linked_issue) = app.linked_issue_for_pull_request(issue.number) {
                    let open_label = format!("[ Issue #{} ]", linked_issue);
                    let web_label = "[ web ]";
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            open_label.clone(),
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_success)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            web_label,
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_primary)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    let prefix_width = prefix.chars().count() as u16;
                    let open_width = open_label.chars().count() as u16;
                    let web_width = web_label.chars().count() as u16;
                    issue_tui_button_hit = Some((line_index, prefix_width, open_width));
                    issue_web_button_hit = Some((
                        line_index,
                        prefix_width.saturating_add(open_width).saturating_add(1),
                        web_width,
                    ));
                }
            }
            lines.push(Line::from(vec![
                Span::styled(
                    "assignees ",
                    Style::default()
                        .fg(theme.accent_subtle)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    ellipsize(assignees.as_str(), 80),
                    Style::default().fg(theme.text_muted),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    "comments  ",
                    Style::default()
                        .fg(theme.accent_success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    issue.comments_count.to_string(),
                    Style::default().fg(theme.text_muted),
                ),
            ]));
            let mut label_row = vec![Span::styled(
                "labels    ",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            )];
            label_row.extend(label_chip_spans(app, labels.as_str(), 4, theme));
            lines.push(Line::from(label_row));
            if let Some(updated) = format_datetime(issue.updated_at.as_deref()) {
                lines.push(Line::from(vec![
                    Span::styled(
                        "updated   ",
                        Style::default()
                            .fg(theme.accent_subtle)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(updated, Style::default().fg(theme.text_muted)),
                ]));
            }
            lines.push(Line::from(""));

            let rendered = markdown::render(issue.body.as_str());
            if rendered.lines.is_empty() {
                lines.push(Line::from("No description."));
            } else {
                lines.extend(rendered.lines);
            }
            (
                preview_title_text.to_string(),
                lines,
                pr_tui_button_hit,
                pr_web_button_hit,
                issue_tui_button_hit,
                issue_web_button_hit,
            )
        }
        None => (
            preview_title_text.to_string(),
            vec![Line::from(
                if item_mode == crate::app::WorkItemMode::PullRequests {
                    "Select a pull request to preview."
                } else {
                    "Select an issue to preview."
                },
            )],
            None,
            None,
            None,
            None,
        ),
    };

    let preview_area = panes[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let preview_content_width = preview_area.width.saturating_sub(2);
    let viewport_height = preview_area.height.saturating_sub(2) as usize;
    let total_lines = wrapped_line_count(&preview_lines, preview_content_width);
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;
    app.set_issues_preview_max_scroll(max_scroll);
    let scroll = app.issues_preview_scroll();
    let preview_block_title =
        ui_status_overlay::focused_title(preview_title.as_str(), preview_focused);
    let preview_block = panel_block_with_border(
        preview_block_title.as_str(),
        ui_status_overlay::focus_border(preview_focused, theme),
        theme,
    );
    let preview_widget = Paragraph::new(Text::from(preview_lines))
        .block(preview_block)
        .style(
            Style::default()
                .fg(theme.text_primary)
                .bg(theme.bg_panel_alt),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(preview_widget, preview_area);
    register_mouse_region(app, MouseTarget::IssuesPreviewPane, preview_area);
    let preview_inner = preview_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if let Some((line, x_offset, width)) = linked_pr_tui_button {
        register_inline_button(
            app,
            preview_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedPullRequestTuiButton,
        );
    }
    if let Some((line, x_offset, width)) = linked_pr_web_button {
        register_inline_button(
            app,
            preview_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedPullRequestWebButton,
        );
    }
    if let Some((line, x_offset, width)) = linked_issue_tui_button {
        register_inline_button(
            app,
            preview_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedIssueTuiButton,
        );
    }
    if let Some((line, x_offset, width)) = linked_issue_web_button {
        register_inline_button(
            app,
            preview_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedIssueWebButton,
        );
    }
}
