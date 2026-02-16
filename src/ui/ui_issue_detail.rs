use super::*;

pub(super) fn draw_issue_detail(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);
    let content_area = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let body_focused = app.focus() == Focus::IssueBody;
    let comments_focused = app.focus() == Focus::IssueRecentComments;
    let is_pr = app.current_issue_row().is_some_and(|issue| issue.is_pr);
    let (
        issue_number,
        issue_title,
        issue_state,
        body,
        assignees,
        labels,
        comment_count,
        updated_at,
    ) = match app.current_issue_row() {
        Some(issue) => (
            Some(issue.number),
            if issue.is_pr {
                format!("PR #{} {}", issue.number, issue.title)
            } else {
                format!("#{} {}", issue.number, issue.title)
            },
            issue.state.clone(),
            issue.body.clone(),
            if issue.assignees.is_empty() {
                "unassigned".to_string()
            } else {
                issue.assignees.clone()
            },
            if issue.labels.is_empty() {
                "none".to_string()
            } else {
                issue.labels.clone()
            },
            issue.comments_count,
            issue.updated_at.clone(),
        ),
        None => (
            None,
            String::new(),
            String::new(),
            String::new(),
            "unassigned".to_string(),
            "none".to_string(),
            0,
            None,
        ),
    };

    let header_text = if issue_title.is_empty() {
        Text::from(vec![
            Line::from(Span::styled(
                "[Back]",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("Issue detail"),
        ])
    } else {
        let pending = issue_number.and_then(|number| app.pending_issue_badge(number));
        Text::from(vec![
            Line::from(Span::styled(
                "[Back]",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled(
                    issue_title.clone(),
                    Style::default()
                        .fg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("[{}]", issue_state),
                    Style::default()
                        .fg(issue_state_color(issue_state.as_str(), theme))
                        .add_modifier(Modifier::BOLD),
                ),
                pending_issue_span(pending, theme),
            ]),
        ])
    };
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
    app.register_mouse_region(MouseTarget::Back, header_content.x, header_content.y, 8, 1);

    let mut body_lines = Vec::new();
    let mut linked_pr_tui_hit = None;
    let mut linked_pr_web_hit = None;
    let mut linked_issue_tui_hit = None;
    let mut linked_issue_web_hit = None;
    if issue_title.is_empty() {
        body_lines.push(Line::from("No issue selected."));
    } else {
        body_lines.push(Line::from(Span::styled(
            issue_title,
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )));
        if let Some(number) = issue_number {
            let link_line = body_lines.len();
            if is_pr {
                let linked_issues = app.linked_issues_for_pull_request(number);
                if !linked_issues.is_empty() {
                    let prefix = if linked_issues.len() == 1 {
                        "linked issue "
                    } else {
                        "linked issues "
                    };
                    let (open_label, more_hint) =
                        linked_item_label("Issue", linked_issues[0], linked_issues.len());
                    let web_label = "[ web ]";
                    let mut linked_row = vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            open_label.clone(),
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_success)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    let mut more_hint_width = 0u16;
                    if let Some(more_hint) = more_hint {
                        more_hint_width = more_hint.chars().count() as u16;
                        linked_row.push(Span::raw(" "));
                        linked_row.push(Span::styled(
                            more_hint,
                            Style::default().fg(theme.text_muted),
                        ));
                    }
                    linked_row.push(Span::raw(" "));
                    linked_row.push(Span::styled(
                        web_label,
                        Style::default()
                            .fg(theme.bg_app)
                            .bg(theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ));
                    body_lines.push(Line::from(linked_row));
                    let prefix_width = prefix.chars().count() as u16;
                    let open_width = open_label.chars().count() as u16;
                    let web_width = web_label.chars().count() as u16;
                    let web_offset = if more_hint_width == 0 {
                        prefix_width.saturating_add(open_width).saturating_add(1)
                    } else {
                        prefix_width
                            .saturating_add(open_width)
                            .saturating_add(1)
                            .saturating_add(more_hint_width)
                            .saturating_add(1)
                    };
                    linked_issue_tui_hit = Some((link_line, prefix_width, open_width));
                    linked_issue_web_hit = Some((link_line, web_offset, web_width));
                }
            } else {
                let linked_prs = app.linked_pull_requests_for_issue(number);
                if !linked_prs.is_empty() {
                    let prefix = if linked_prs.len() == 1 {
                        "linked PR "
                    } else {
                        "linked PRs "
                    };
                    let (open_label, more_hint) =
                        linked_item_label("PR", linked_prs[0], linked_prs.len());
                    let web_label = "[ web ]";
                    let mut linked_row = vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            open_label.clone(),
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_success)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    let mut more_hint_width = 0u16;
                    if let Some(more_hint) = more_hint {
                        more_hint_width = more_hint.chars().count() as u16;
                        linked_row.push(Span::raw(" "));
                        linked_row.push(Span::styled(
                            more_hint,
                            Style::default().fg(theme.text_muted),
                        ));
                    }
                    linked_row.push(Span::raw(" "));
                    linked_row.push(Span::styled(
                        web_label,
                        Style::default()
                            .fg(theme.bg_app)
                            .bg(theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ));
                    body_lines.push(Line::from(linked_row));
                    let prefix_width = prefix.chars().count() as u16;
                    let open_width = open_label.chars().count() as u16;
                    let web_width = web_label.chars().count() as u16;
                    let web_offset = if more_hint_width == 0 {
                        prefix_width.saturating_add(open_width).saturating_add(1)
                    } else {
                        prefix_width
                            .saturating_add(open_width)
                            .saturating_add(1)
                            .saturating_add(more_hint_width)
                            .saturating_add(1)
                    };
                    linked_pr_tui_hit = Some((link_line, prefix_width, open_width));
                    linked_pr_web_hit = Some((link_line, web_offset, web_width));
                }
            }
        }
    }
    let metadata = Line::from(format!(
        "assignees: {} | comments: {}",
        assignees, comment_count
    ));
    body_lines.push(metadata.style(Style::default().fg(theme.text_muted)));
    let mut labels_row = vec![Span::styled(
        "labels: ",
        Style::default().fg(theme.text_muted),
    )];
    labels_row.extend(label_chip_spans(app, labels.as_str(), 5, theme));
    body_lines.push(Line::from(labels_row));
    if let Some(updated) = format_datetime(updated_at.as_deref()) {
        body_lines.push(Line::from(format!("updated: {}", updated)));
    }
    body_lines.push(Line::from(""));
    let rendered_body = markdown::render(body.as_str());
    if rendered_body.lines.is_empty() {
        body_lines.push(Line::from("No description."));
    } else {
        for line in rendered_body.lines {
            body_lines.push(line);
        }
    }

    let mut side_lines = Vec::new();
    if is_pr {
        side_lines.push(Line::from(Span::styled(
            "Press Enter for full-screen changes",
            Style::default()
                .fg(theme.border_popup)
                .add_modifier(Modifier::BOLD),
        )));
        side_lines.push(Line::from(""));
    } else {
        side_lines.push(Line::from(Span::styled(
            "Press Enter for full comments",
            Style::default()
                .fg(theme.border_popup)
                .add_modifier(Modifier::BOLD),
        )));
        side_lines.push(Line::from(""));
    }
    if is_pr {
        if app.pull_request_files_syncing() {
            side_lines.push(Line::from("Loading pull request changes"));
        } else if app.pull_request_files().is_empty() {
            side_lines.push(Line::from(
                "No changed files cached yet. Press r to refresh.",
            ));
        } else {
            for file in app.pull_request_files() {
                side_lines.push(Line::from(vec![
                    Span::styled(
                        file_status_symbol(file.status.as_str()),
                        Style::default().fg(file_status_color(file.status.as_str(), theme)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        file.filename.clone(),
                        Style::default()
                            .fg(theme.text_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                side_lines.push(
                    Line::from(format!("  +{} -{}", file.additions, file.deletions))
                        .style(Style::default().fg(theme.text_muted)),
                );
                if let Some(patch) = file.patch.as_deref() {
                    for patch_line in patch.lines().take(8) {
                        side_lines.push(styled_patch_line(patch_line, 100, theme));
                    }
                    if patch.lines().count() > 8 {
                        side_lines.push(
                            Line::from("  more").style(Style::default().fg(theme.text_muted)),
                        );
                    }
                }
                side_lines.push(Line::from(""));
            }
        }
    } else if app.comments().is_empty() {
        side_lines.push(Line::from("No comments cached yet."));
    } else {
        let start = app.comments().len().saturating_sub(3);
        for (index, comment) in app.comments()[start..].iter().enumerate() {
            side_lines.push(comment_header(
                start + index + 1,
                comment.author.as_str(),
                comment.created_at.as_deref(),
                false,
                theme,
            ));
            let rendered_comment = markdown::render(comment.body.as_str());
            if rendered_comment.lines.is_empty() {
                side_lines.push(Line::from(""));
            } else {
                for line in rendered_comment.lines {
                    side_lines.push(line);
                }
            }
            side_lines.push(Line::from(""));
        }
    }

    let min_body_height = 6u16;
    let mut comments_height =
        RECENT_COMMENTS_HEIGHT.min(content_area.height.saturating_sub(min_body_height));
    if comments_height < 3 {
        comments_height = content_area.height.min(3);
    }
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(comments_height)])
        .split(content_area);

    let body_content_width = panes[0].width.saturating_sub(2);
    let body_viewport_height = panes[0].height.saturating_sub(2) as usize;
    let body_total_lines = wrapped_line_count(&body_lines, body_content_width);
    let max_scroll = body_total_lines.saturating_sub(body_viewport_height) as u16;
    app.set_issue_detail_max_scroll(max_scroll);
    let scroll = app.issue_detail_scroll();

    let base_body_title = if is_pr {
        "Pull request description"
    } else {
        "Issue description"
    };
    let body_title = ui_status_overlay::focused_title(base_body_title, body_focused);
    let body_block = Block::default()
        .title(Line::from(Span::styled(
            body_title,
            Style::default()
                .fg(if body_focused {
                    theme.accent_primary
                } else {
                    theme.text_muted
                })
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ui_status_overlay::focus_border(body_focused, theme)))
        .style(Style::default().bg(if body_focused {
            theme.bg_panel_alt
        } else {
            theme.bg_panel
        }));
    let body_paragraph = Paragraph::new(Text::from(body_lines))
        .block(body_block)
        .style(Style::default().fg(theme.text_primary).bg(if body_focused {
            theme.bg_panel_alt
        } else {
            theme.bg_panel
        }))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(body_paragraph, panes[0]);
    register_mouse_region(app, MouseTarget::IssueBodyPane, panes[0]);
    let body_inner = panes[0].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if let Some((line, x_offset, width)) = linked_pr_tui_hit {
        register_inline_button(
            app,
            body_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedPullRequestTuiButton,
        );
    }
    if let Some((line, x_offset, width)) = linked_pr_web_hit {
        register_inline_button(
            app,
            body_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedPullRequestWebButton,
        );
    }
    if let Some((line, x_offset, width)) = linked_issue_tui_hit {
        register_inline_button(
            app,
            body_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedIssueTuiButton,
        );
    }
    if let Some((line, x_offset, width)) = linked_issue_web_hit {
        register_inline_button(
            app,
            body_inner,
            scroll,
            line,
            x_offset,
            width,
            MouseTarget::LinkedIssueWebButton,
        );
    }

    let side_content_width = panes[1].width.saturating_sub(2);
    let side_viewport = panes[1].height.saturating_sub(2) as usize;
    let side_total_lines = wrapped_line_count(&side_lines, side_content_width);
    let side_max_scroll = side_total_lines.saturating_sub(side_viewport) as u16;
    app.set_issue_recent_comments_max_scroll(side_max_scroll);
    let side_scroll = app.issue_recent_comments_scroll();
    let side_border = ui_status_overlay::focus_border(comments_focused, theme);
    let side_title = if is_pr {
        format!("Changed files ({})", app.pull_request_files().len())
    } else {
        format!("Recent comments ({})", app.comments().len())
    };
    let side_title = ui_status_overlay::focused_title(side_title.as_str(), comments_focused);
    let side_block = Block::default()
        .title(Line::from(Span::styled(
            side_title,
            Style::default()
                .fg(if comments_focused {
                    theme.accent_primary
                } else {
                    theme.text_muted
                })
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(if comments_focused {
            theme.bg_panel_alt
        } else {
            theme.bg_panel
        }))
        .border_style(Style::default().fg(side_border));
    let side_paragraph = Paragraph::new(Text::from(side_lines))
        .block(side_block)
        .style(
            Style::default()
                .fg(theme.text_primary)
                .bg(if comments_focused {
                    theme.bg_panel_alt
                } else {
                    theme.bg_panel
                }),
        )
        .wrap(Wrap { trim: false })
        .scroll((side_scroll, 0));
    frame.render_widget(side_paragraph, panes[1]);
    register_mouse_region(app, MouseTarget::IssueSidePane, panes[1]);
}

pub(super) fn draw_issue_comments(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);
    let content_area = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let title = match app.current_issue_row() {
        Some(issue) => {
            if issue.is_pr {
                format!("Comments PR #{}", issue.number)
            } else {
                format!("Comments #{}", issue.number)
            }
        }
        None => "Comments (j/k jump)".to_string(),
    };
    let selected = if app.comments().is_empty() {
        "none".to_string()
    } else {
        format!("{}/{}", app.selected_comment() + 1, app.comments().len())
    };
    let header = Text::from(vec![
        Line::from(Span::styled(
            "[Back]",
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            title.clone(),
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "j/k jump comments • selected {} • e edit • x delete",
                selected
            ),
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
    app.register_mouse_region(MouseTarget::Back, header_content.x, header_content.y, 8, 1);

    let block = panel_block(&title, theme);
    let mut lines = Vec::new();
    let mut comment_header_offsets = Vec::new();
    if app.comments().is_empty() {
        lines.push(Line::from("No comments cached yet."));
    } else {
        for (index, comment) in app.comments().iter().enumerate() {
            comment_header_offsets.push((index, lines.len() as u16));
            lines.push(comment_header(
                index + 1,
                comment.author.as_str(),
                comment.created_at.as_deref(),
                index == app.selected_comment(),
                theme,
            ));
            let rendered = markdown::render(comment.body.as_str());
            if rendered.lines.is_empty() {
                lines.push(Line::from(""));
            } else {
                for line in rendered.lines {
                    lines.push(line);
                }
            }
            lines.push(Line::from(""));
        }
    }

    let comments_content_width = content_area.width.saturating_sub(2);
    let viewport_height = content_area.height.saturating_sub(2) as usize;
    let total_lines = wrapped_line_count(&lines, comments_content_width);
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;
    app.set_issue_comments_max_scroll(max_scroll);
    let scroll = app.issue_comments_scroll();

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, content_area);
    register_mouse_region(app, MouseTarget::CommentsPane, content_area);
    let content_inner = content_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    for (index, offset) in comment_header_offsets {
        if offset < scroll {
            continue;
        }
        let y = content_inner
            .y
            .saturating_add(offset.saturating_sub(scroll));
        if y >= content_inner.y.saturating_add(content_inner.height) {
            continue;
        }
        app.register_mouse_region(
            MouseTarget::CommentRow(index),
            content_inner.x,
            y,
            content_inner.width,
            1,
        );
    }
}

fn linked_item_label(kind: &str, number: i64, total: usize) -> (String, Option<String>) {
    let open = format!("[ {} #{} ]", kind, number);
    let more = total.saturating_sub(1);
    if more == 0 {
        return (open, None);
    }
    (open, Some(format!("+{} more", more)))
}

#[cfg(test)]
mod tests {
    use super::linked_item_label;

    #[test]
    fn linked_item_label_omits_hint_for_single() {
        let (label, hint) = linked_item_label("Issue", 42, 1);
        assert_eq!(label, "[ Issue #42 ]");
        assert_eq!(hint, None);
    }

    #[test]
    fn linked_item_label_adds_more_hint_for_multiple() {
        let (label, hint) = linked_item_label("PR", 7, 3);
        assert_eq!(label, "[ PR #7 ]");
        assert_eq!(hint.as_deref(), Some("+2 more"));
    }
}
