use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};

use crate::app::{
    App, EditorMode, Focus, IssueFilter, MouseTarget, PullRequestReviewFocus, ReviewSide, View,
};
use crate::markdown;
use crate::pr_diff::{DiffKind, parse_patch};
use crate::theme::{ThemePalette, resolve_theme};

const RECENT_COMMENTS_HEIGHT: u16 = 10;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let theme = resolve_theme(app.theme_name());
    let area = frame.area();
    app.clear_mouse_regions();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg_app)),
        area,
    );
    match app.view() {
        View::RepoPicker => draw_repo_picker(frame, app, area, theme),
        View::RemoteChooser => draw_remote_chooser(frame, app, area, theme),
        View::Issues => draw_issues(frame, app, area, theme),
        View::IssueDetail => draw_issue_detail(frame, app, area, theme),
        View::IssueComments => draw_issue_comments(frame, app, area, theme),
        View::PullRequestFiles => draw_pull_request_files(frame, app, area, theme),
        View::LabelPicker => draw_label_picker(frame, app, area, theme),
        View::AssigneePicker => draw_assignee_picker(frame, app, area, theme),
        View::CommentPresetPicker => draw_preset_picker(frame, app, area, theme),
        View::CommentPresetName => draw_preset_name(frame, app, area, theme),
        View::CommentEditor => draw_comment_editor(frame, app, area, theme),
    }
}

fn draw_repo_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(main);

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
                "No repos found. Run `glyph sync` or press Ctrl+R to rescan.",
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
                        format!("{}", repo.remote_name),
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

    draw_status(frame, app, footer, theme);
}

fn draw_remote_chooser(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
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
    let list_area = main.inner(Margin {
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

    draw_status(frame, app, footer, theme);
}

fn draw_issues(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(main);
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
    let list_block_title = focused_title(list_title, list_focused);
    let block = panel_block_with_border(
        list_block_title.as_str(),
        focus_border(list_focused, theme),
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
                let mut line1_spans = vec![
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
                if !issue.is_pr {
                    if let Some(linked_pr) = app.linked_pull_request_for_issue(issue.number) {
                        line1_spans.push(Span::raw(" "));
                        line1_spans.push(Span::styled(
                            format!("[PR#{}]", linked_pr),
                            Style::default()
                                .fg(theme.accent_success)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                }
                let line1 = Line::from(line1_spans);
                let line2 = Line::from(vec![
                    Span::styled(
                        "A:",
                        Style::default()
                            .fg(theme.accent_subtle)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        ellipsize(assignees, 20),
                        Style::default().fg(theme.text_muted),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "C:",
                        Style::default()
                            .fg(theme.accent_success)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        issue.comments_count.to_string(),
                        Style::default().fg(theme.text_muted),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "L:",
                        Style::default()
                            .fg(theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(ellipsize(labels, 24), Style::default().fg(theme.text_muted)),
                ]);
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
            let mut pr_tui_button_hit = None;
            let mut pr_web_button_hit = None;
            let mut issue_tui_button_hit = None;
            let mut issue_web_button_hit = None;
            let line_index = lines.len();
            if !issue.is_pr {
                let prefix = "linked PR ";
                if let Some(linked_pr) = app.linked_pull_request_for_issue(issue.number) {
                    let open_label = format!("[ Open PR #{} ]", linked_pr);
                    let web_label = "[ Web ]";
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
                } else if app.linked_pull_request_known(issue.number) {
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled("none found", Style::default().fg(theme.accent_danger)),
                    ]));
                } else {
                    let probe_label = "[ Find linked PR ]";
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            probe_label,
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_subtle)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    pr_tui_button_hit = Some((
                        line_index,
                        prefix.chars().count() as u16,
                        probe_label.chars().count() as u16,
                    ));
                }
            } else {
                let prefix = "linked issue ";
                if let Some(linked_issue) = app.linked_issue_for_pull_request(issue.number) {
                    let open_label = format!("[ Open Issue #{} ]", linked_issue);
                    let web_label = "[ Web ]";
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
                } else if app.linked_issue_known(issue.number) {
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled("none found", Style::default().fg(theme.accent_danger)),
                    ]));
                } else {
                    let probe_label = "[ Find linked issue ]";
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            probe_label,
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_subtle)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    issue_tui_button_hit = Some((
                        line_index,
                        prefix.chars().count() as u16,
                        probe_label.chars().count() as u16,
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
            lines.push(Line::from(vec![
                Span::styled(
                    "labels    ",
                    Style::default()
                        .fg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    ellipsize(labels.as_str(), 80),
                    Style::default().fg(theme.text_muted),
                ),
            ]));
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
    let preview_block_title = focused_title(preview_title.as_str(), preview_focused);
    let preview_block = panel_block_with_border(
        preview_block_title.as_str(),
        focus_border(preview_focused, theme),
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

    draw_status(frame, app, footer, theme);
}

fn draw_issue_detail(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(main);
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
                let prefix = "linked issue ";
                if let Some(linked_issue) = app.linked_issue_for_pull_request(number) {
                    let open_label = format!("[ Open Issue #{} ]", linked_issue);
                    let web_label = "[ Web ]";
                    body_lines.push(Line::from(vec![
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
                    linked_issue_tui_hit = Some((link_line, prefix_width, open_width));
                    linked_issue_web_hit = Some((
                        link_line,
                        prefix_width.saturating_add(open_width).saturating_add(1),
                        web_width,
                    ));
                } else if app.linked_issue_known(number) {
                    body_lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled("none found", Style::default().fg(theme.accent_danger)),
                    ]));
                } else {
                    let probe_label = "[ Find linked issue ]";
                    body_lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            probe_label,
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_subtle)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    linked_issue_tui_hit = Some((
                        link_line,
                        prefix.chars().count() as u16,
                        probe_label.chars().count() as u16,
                    ));
                }
            } else {
                let prefix = "linked PR ";
                if let Some(linked_pr) = app.linked_pull_request_for_issue(number) {
                    let open_label = format!("[ Open PR #{} ]", linked_pr);
                    let web_label = "[ Web ]";
                    body_lines.push(Line::from(vec![
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
                    linked_pr_tui_hit = Some((link_line, prefix_width, open_width));
                    linked_pr_web_hit = Some((
                        link_line,
                        prefix_width.saturating_add(open_width).saturating_add(1),
                        web_width,
                    ));
                } else if app.linked_pull_request_known(number) {
                    body_lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled("none found", Style::default().fg(theme.accent_danger)),
                    ]));
                } else {
                    let probe_label = "[ Find linked PR ]";
                    body_lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.text_muted)),
                        Span::styled(
                            probe_label,
                            Style::default()
                                .fg(theme.bg_app)
                                .bg(theme.accent_subtle)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    linked_pr_tui_hit = Some((
                        link_line,
                        prefix.chars().count() as u16,
                        probe_label.chars().count() as u16,
                    ));
                }
            }
        }
    }
    let metadata = Line::from(format!(
        "assignees: {} | comments: {} | labels: {}",
        assignees,
        comment_count,
        ellipsize(labels.as_str(), 44)
    ));
    body_lines.push(metadata.style(Style::default().fg(theme.text_muted)));
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
            side_lines.push(Line::from("Loading pull request changes..."));
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
                        side_lines
                            .push(Line::from("  ...").style(Style::default().fg(theme.text_muted)));
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
    let body_title = focused_title(base_body_title, body_focused);
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
        .border_style(Style::default().fg(focus_border(body_focused, theme)))
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
    let side_border = focus_border(comments_focused, theme);
    let side_title = if is_pr {
        format!("Changed files ({})", app.pull_request_files().len())
    } else {
        format!("Recent comments ({})", app.comments().len())
    };
    let side_title = focused_title(side_title.as_str(), comments_focused);
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

    draw_status(frame, app, footer, theme);
}

fn draw_issue_comments(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(main);
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

    draw_status(frame, app, footer, theme);
}

fn draw_pull_request_files(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(main);
    let content = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(content);

    let title = match app.current_issue_row() {
        Some(issue) => format!("PR review #{}", issue.number),
        None => "PR review".to_string(),
    };
    let focused = match app.pull_request_review_focus() {
        PullRequestReviewFocus::Files => "files",
        PullRequestReviewFocus::Diff => "diff",
    };
    let side = match app.pull_request_review_side() {
        ReviewSide::Left => "old",
        ReviewSide::Right => "new",
    };
    let visual = if app.pull_request_visual_mode() {
        "visual"
    } else {
        "normal"
    };
    let visual_range = app
        .pull_request_visual_range()
        .map(|(start, end)| format!("{}-{}", start + 1, end + 1))
        .unwrap_or_else(|| "-".to_string());
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
                format!("x:{}", horizontal_scroll),
                Style::default().fg(theme.text_muted),
            ),
        ]),
        Line::from(Span::styled(
            format!(
                "Ctrl+h/l pane • h/l side • [/ ] pan • 0 reset pan • w viewed • z collapse hunk • Shift+V visual • m comment • e edit • x delete • Shift+R resolve thread • focus:{} side:{} mode:{} range:{}",
                focused, side, visual, visual_range
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
    let files_block_title = focused_title("Changed files", files_focused);
    let files_list = List::new(file_items)
        .block(panel_block_with_border(
            files_block_title.as_str(),
            focus_border(files_focused, theme),
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

    let diff_focused = app.pull_request_review_focus() == PullRequestReviewFocus::Diff;
    let selected_file = app
        .selected_pull_request_file_row()
        .map(|file| (file.filename.clone(), file.patch.clone()));
    let mut lines = Vec::new();
    let mut row_offsets = Vec::new();

    if app.pull_request_files_syncing() {
        lines.push(Line::from("Loading pull request changes..."));
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
            let panel_width = panes[1].width.saturating_sub(2) as usize;
            let cells_width = panel_width.saturating_sub(2);
            let left_width = cells_width.saturating_sub(5) / 2;
            let right_width = cells_width.saturating_sub(left_width + 3);
            let horizontal_offset = app.pull_request_diff_horizontal_scroll() as usize;
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

                lines.push(render_split_diff_row(
                    row,
                    selected,
                    in_visual_range,
                    app.pull_request_review_side(),
                    left_width,
                    right_width,
                    horizontal_offset,
                    theme,
                ));

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
                    lines.push(render_inline_review_comment(
                        comment.id,
                        comment.author.as_str(),
                        comment.body.as_str(),
                        ReviewSide::Right,
                        comment.resolved,
                        panel_width,
                        left_width,
                        right_width,
                        app.selected_pull_request_review_comment_id() == Some(comment.id),
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
                    lines.push(render_inline_review_comment(
                        comment.id,
                        comment.author.as_str(),
                        comment.body.as_str(),
                        ReviewSide::Left,
                        comment.resolved,
                        panel_width,
                        left_width,
                        right_width,
                        app.selected_pull_request_review_comment_id() == Some(comment.id),
                        theme,
                    ));
                }
            }
        }
    }

    let content_width = panes[1].width.saturating_sub(2);
    let viewport_height = panes[1].height.saturating_sub(2) as usize;
    let total_lines = wrapped_line_count(&lines, content_width);
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;
    app.set_pull_request_diff_max_scroll(max_scroll);

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
    app.set_pull_request_diff_scroll(scroll);

    let diff_title = selected_file
        .as_ref()
        .map(|(file_name, _)| {
            format!(
                "Diff: {}  [pan {} | [/] move]",
                file_name,
                app.pull_request_diff_horizontal_scroll(),
            )
        })
        .unwrap_or_else(|| "Diff".to_string());
    let diff_block_title = focused_title(diff_title.as_str(), diff_focused);
    let paragraph = Paragraph::new(Text::from(lines))
        .block(panel_block_with_border(
            diff_block_title.as_str(),
            focus_border(diff_focused, theme),
            theme,
        ))
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, panes[1]);
    register_mouse_region(app, MouseTarget::PullRequestDiffPane, panes[1]);
    let diff_inner = panes[1].inner(Margin {
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

    draw_status(frame, app, footer, theme);
}

fn draw_label_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    draw_modal_background(frame, app, area, theme);
    let popup = centered_rect(74, 76, area);
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

fn draw_assignee_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    draw_modal_background(frame, app, area, theme);
    let popup = centered_rect(74, 76, area);
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
            Constraint::Length(3),
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

fn draw_preset_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let close_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Close Pull Request"
    } else {
        "Close Issue"
    };
    let block = panel_block(close_title, theme);
    let mut items = Vec::new();
    items.push(ListItem::new("Close without comment"));
    items.push(ListItem::new("Custom message..."));
    for preset in app.comment_defaults() {
        items.push(ListItem::new(preset.name.as_str()));
    }
    items.push(ListItem::new("Add preset..."));

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
    let list_area = main.inner(Margin {
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

    draw_status(frame, app, footer, theme);
}

fn draw_preset_name(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
    let input_area = main.inner(Margin {
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

    draw_status(frame, app, footer, theme);
}

fn draw_comment_editor(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let (main, footer) = split_area(area);
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
    let editor_area = main.inner(Margin {
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

    draw_status(frame, app, footer, theme);
}

fn draw_status(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: &ThemePalette) {
    let status = app.status();
    let context_raw = status_context(app);
    let help_raw = help_text(app);
    let sync = sync_state_label(app);
    let line_width = area.width.saturating_sub(4) as usize;
    let sync_label = format!("[{}]", sync);
    let status_prefix_width = "[Repos] ".chars().count() + 1 + sync_label.chars().count() + 2;
    let context_prefix_width = "context ".chars().count();
    let keys_prefix_width = "keys ".chars().count();
    let status_value = if status.is_empty() { "ready" } else { status };
    let status_text = fit_inline(status_value, line_width.saturating_sub(status_prefix_width));
    let context = fit_inline(
        context_raw.as_str(),
        line_width.saturating_sub(context_prefix_width),
    );
    let help = fit_help_tokens(
        help_raw.as_str(),
        line_width.saturating_sub(keys_prefix_width),
    );
    let mut lines = Vec::new();
    let mut status_line = vec![Span::styled(
        "[Repos] ",
        Style::default()
            .fg(theme.accent_primary)
            .add_modifier(Modifier::BOLD),
    )];
    status_line.push(Span::raw(" "));
    status_line.push(Span::styled(
        sync_label,
        Style::default()
            .fg(sync_state_color(sync, theme))
            .add_modifier(Modifier::BOLD),
    ));
    status_line.push(Span::raw("  "));
    status_line.push(Span::styled(
        status_text,
        Style::default().fg(theme.text_primary),
    ));
    lines.push(Line::from(status_line));
    lines.push(Line::from(vec![
        Span::styled(
            "context ",
            Style::default()
                .fg(theme.accent_success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(context, Style::default().fg(theme.text_muted)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "keys ",
            Style::default()
                .fg(theme.accent_subtle)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(help, Style::default().fg(theme.text_muted)),
    ]));
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme.text_muted).bg(theme.bg_panel))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default().bg(theme.bg_panel))
                .border_style(Style::default().fg(theme.border_panel)),
        );
    let status_area = area.inner(Margin {
        vertical: 0,
        horizontal: 2,
    });
    frame.render_widget(paragraph, status_area);
    app.register_mouse_region(
        MouseTarget::RepoPicker,
        status_area.x,
        status_area.y.saturating_add(1),
        10,
        1,
    );
}

fn panel_block<'a>(title: &'a str, theme: &ThemePalette) -> Block<'a> {
    panel_block_with_border(title, theme.border_panel, theme)
}

fn popup_block<'a>(title: &'a str, theme: &ThemePalette) -> Block<'a> {
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

fn focused_title(title: &str, focused: bool) -> String {
    if focused {
        return format!("> {}", title);
    }
    title.to_string()
}

fn panel_block_with_border<'a>(title: &'a str, border: Color, theme: &ThemePalette) -> Block<'a> {
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

fn focus_border(focused: bool, theme: &ThemePalette) -> Color {
    if focused {
        theme.border_focus
    } else {
        theme.border_panel
    }
}

fn draw_modal_background(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: &ThemePalette) {
    match app.editor_cancel_view() {
        View::Issues => draw_issues(frame, app, area, theme),
        View::IssueDetail => draw_issue_detail(frame, app, area, theme),
        View::IssueComments => draw_issue_comments(frame, app, area, theme),
        View::PullRequestFiles => draw_pull_request_files(frame, app, area, theme),
        _ => {
            let (main, footer) = split_area(area);
            frame.render_widget(panel_block("Glyph", theme), main);
            draw_status(frame, app, footer, theme);
        }
    }
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg_overlay)),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

fn split_area(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(4)])
        .split(area);
    (chunks[0], chunks[1])
}

fn register_mouse_region(app: &mut App, target: MouseTarget, area: Rect) {
    app.register_mouse_region(target, area.x, area.y, area.width, area.height);
}

fn register_inline_button(
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

fn help_text(app: &App) -> String {
    match app.view() {
        View::RepoPicker => {
            if app.repo_search_mode() {
                return "Search repos: type query • Enter keep • Esc clear • Ctrl+u clear"
                    .to_string();
            }
            "Ctrl+R rescan • j/k move • Ctrl+u/d page • gg/G top/bottom • / search • Enter select • q quit"
                .to_string()
        }
        View::RemoteChooser => {
            "j/k move • gg/G top/bottom • Enter select • Ctrl+G repos • q quit"
                .to_string()
        }
        View::Issues => {
            if app.issue_search_mode() {
                return "Search: type terms/qualifiers (is:, label:, assignee:, #num) • Enter keep • Esc clear • Ctrl+u clear"
                    .to_string();
            }
            let selected_is_pr = app.selected_issue_row().is_some_and(|issue| issue.is_pr);
            let reviewing_pr =
                selected_is_pr || app.work_item_mode() == crate::app::WorkItemMode::PullRequests;
            let mut parts = vec![
                "j/k move",
                "Enter open",
                "/ search",
                "p issues/prs",
                "1/2 tabs",
                "f open/closed",
                "a assignee",
                "l labels",
                "Shift+A assignees",
                "m comment",
                "r refresh",
                "o browser",
                "q quit",
            ];
            if reviewing_pr {
                parts.insert(10, "u reopen");
                parts.insert(11, "dd close");
                parts.insert(12, "v checkout");
                parts.insert(13, "Shift+P linked issue (TUI)");
                parts.insert(14, "Shift+O linked issue (web)");
            } else {
                parts.insert(10, "u reopen");
                parts.insert(11, "dd close");
                if app.selected_issue_has_known_linked_pr() {
                    parts.insert(12, "Shift+P linked PR (TUI)");
                    parts.insert(13, "Shift+O linked PR (web)");
                }
            }
            parts.join(" • ")
        }
        View::IssueDetail => {
            let is_pr = app.current_issue_row().is_some_and(|issue| issue.is_pr);
            if is_pr {
                let linked_hint = if app.selected_pull_request_has_known_linked_issue() {
                    "Shift+P linked issue (TUI) • Shift+O linked issue (web)"
                } else {
                    "Shift+P find/open linked issue • Shift+O open linked issue (web)"
                };
                return "Ctrl+h/l pane • j/k scroll • Enter on description opens comments • Enter on changes opens review • c comments • h/l side in review • m comment • l labels • Shift+A assignees • u reopen • dd close • v checkout • Shift+P linked issue (TUI) • Shift+O linked issue (web) • r refresh • Esc back • q quit"
                    .replace(
                        "Shift+P linked issue (TUI) • Shift+O linked issue (web)",
                        linked_hint,
                    );
            }
            if app.selected_issue_has_known_linked_pr() {
                return "Ctrl+h/l pane • j/k scroll • Enter on right pane opens comments • c comments • m comment • l labels • Shift+A assignees • u reopen • dd close • Shift+P linked PR (TUI) • Shift+O linked PR (web) • r refresh • Esc back • q quit"
                    .to_string();
            }
            "Ctrl+h/l pane • j/k scroll • Enter on right pane opens comments • c comments • m comment • l labels • Shift+A assignees • u reopen • dd close • r refresh • Esc back • q quit"
                .to_string()
        }
        View::IssueComments => {
            let is_pr = app.current_issue_row().is_some_and(|issue| issue.is_pr);
            if is_pr {
                let linked_hint = if app.selected_pull_request_has_known_linked_issue() {
                    "Shift+P linked issue (TUI) • Shift+O linked issue (web)"
                } else {
                    "Shift+P find/open linked issue • Shift+O open linked issue (web)"
                };
                return "j/k comments • e edit • x delete • m comment • l labels • Shift+A assignees • u reopen • dd close • v checkout • Shift+P linked issue (TUI) • Shift+O linked issue (web) • r refresh • Esc back • q quit"
                    .replace(
                        "Shift+P linked issue (TUI) • Shift+O linked issue (web)",
                        linked_hint,
                    );
            }
            if app.selected_issue_has_known_linked_pr() {
                return "j/k comments • e edit • x delete • m comment • l labels • Shift+A assignees • u reopen • dd close • Shift+P linked PR (TUI) • Shift+O linked PR (web) • r refresh • Esc back • q quit"
                    .to_string();
            }
            "j/k comments • e edit • x delete • m comment • l labels • Shift+A assignees • u reopen • dd close • r refresh • Esc back • q quit"
                .to_string()
        }
        View::PullRequestFiles => {
            "Ctrl+h/l pane • j/k move file/line • [/ ] pan diff • 0 reset pan • w viewed • z collapse hunk • h/l old/new side • Shift+V visual range • m add • e edit • x delete • Shift+R resolve/reopen • n/p cycle line comments • r refresh • v checkout • Esc/back click • q quit"
                .to_string()
        }
        View::LabelPicker => {
            "Type to filter • j/k move • space toggle • Enter apply • Ctrl+u clear • Esc cancel"
                .to_string()
        }
        View::AssigneePicker => {
            "Type to filter • j/k move • space toggle • Enter apply • Ctrl+u clear • Esc cancel"
                .to_string()
        }
        View::CommentPresetPicker => {
            "j/k move • gg/G top/bottom • Enter select • Esc cancel • q quit".to_string()
        }
        View::CommentPresetName => {
            "Type name • Enter next • Esc cancel".to_string()
        }
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::AddPreset {
                return "Type preset body • Enter save • Shift+Enter newline (Ctrl+j fallback) • Esc cancel"
                    .to_string();
            }
            "Type message • Enter submit • Shift+Enter newline (Ctrl+j fallback) • Esc cancel"
                .to_string()
        }
    }
}

fn status_context(app: &App) -> String {
    let repo = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => format!("{}/{}", owner, repo),
        _ => "no repo selected".to_string(),
    };
    let sync = sync_state_label(app);
    if app.view() == View::Issues {
        let query = app.issue_query().trim();
        let query = if query.is_empty() {
            "none".to_string()
        } else {
            ellipsize(query, 24)
        };
        let assignee = ellipsize(app.assignee_filter_label().as_str(), 18);
        let mode = if app.issue_search_mode() {
            "search"
        } else {
            "browse"
        };
        let item_mode = app.work_item_mode().label();
        return format!(
            "repo: {}  |  mode: {} ({})  |  assignee: {}  |  query: {}  |  status: {}",
            repo, mode, item_mode, assignee, query, sync
        );
    }
    format!("repo: {}  |  status: {}", repo, sync)
}

fn sync_state_label(app: &App) -> &'static str {
    if app.syncing() {
        return "syncing";
    }
    if app.pull_request_files_syncing() {
        return "loading pr files";
    }
    if app.pull_request_review_comments_syncing() {
        return "loading review comments";
    }
    if app.comment_syncing() {
        return "syncing comments";
    }
    if app.scanning() {
        return "scanning";
    }
    "idle"
}

fn sync_state_color(sync: &str, theme: &ThemePalette) -> Color {
    if sync == "idle" {
        return theme.text_muted;
    }
    if sync == "scanning" {
        return theme.accent_subtle;
    }
    theme.accent_success
}

fn fit_inline(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if value.chars().count() <= max {
        return value.to_string();
    }
    ellipsize(value, max)
}

fn fit_help_tokens(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if value.chars().count() <= max {
        return value.to_string();
    }
    let parts = value.split(" • ").collect::<Vec<&str>>();
    if parts.is_empty() {
        return fit_inline(value, max);
    }
    let mut compact = String::new();
    for part in parts {
        let separator = if compact.is_empty() { "" } else { " • " };
        let next = format!("{}{}", separator, part);
        if compact.chars().count() + next.chars().count() > max.saturating_sub(4) {
            break;
        }
        compact.push_str(next.as_str());
    }
    if compact.is_empty() {
        return fit_inline(value, max);
    }
    if compact.chars().count() + 4 <= max {
        compact.push_str(" ...");
    }
    fit_inline(compact.as_str(), max)
}

fn list_state(selected: usize) -> ListState {
    let mut state = ListState::default();
    state.select(Some(selected));
    state
}

fn selected_for_list(selected: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    selected.min(len - 1)
}

fn list_window_start(selected: usize, len: usize, viewport_items: usize) -> usize {
    if len == 0 || viewport_items == 0 {
        return 0;
    }
    let selected = selected_for_list(selected, len);
    selected.saturating_sub(viewport_items.saturating_sub(1))
}

fn issue_tabs_line(
    filter: IssueFilter,
    open_count: usize,
    closed_count: usize,
    theme: &ThemePalette,
) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(filter_tab(
        "1 Open",
        open_count,
        filter == IssueFilter::Open,
        theme.accent_success,
        theme,
    ));
    spans.push(Span::raw("  "));
    spans.push(filter_tab(
        "2 Closed",
        closed_count,
        filter == IssueFilter::Closed,
        theme.accent_danger,
        theme,
    ));
    Line::from(spans)
}

fn filter_tab(
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

fn issue_state_color(state: &str, theme: &ThemePalette) -> Color {
    if state.eq_ignore_ascii_case("closed") {
        return theme.accent_danger;
    }
    theme.accent_success
}

fn styled_patch_line(line: &str, width: usize, theme: &ThemePalette) -> Line<'static> {
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

fn render_split_diff_row(
    row: &crate::pr_diff::DiffRow,
    selected: bool,
    in_visual_range: bool,
    selected_side: ReviewSide,
    left_width: usize,
    right_width: usize,
    horizontal_offset: usize,
    theme: &ThemePalette,
) -> Line<'static> {
    if row.kind == DiffKind::Hunk {
        return Line::from(Span::styled(
            format!(
                " {}",
                clip_horizontal(
                    row.raw.as_str(),
                    horizontal_offset,
                    left_width + right_width + 4
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
                    horizontal_offset,
                    left_width + right_width + 4
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
        horizontal_offset,
        left_width.saturating_sub(5),
    );
    let right_text = clip_horizontal(
        row.right.as_str(),
        horizontal_offset,
        right_width.saturating_sub(5),
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
    if in_visual_range {
        bg_color = Some(theme.bg_visual_range);
        row_style = Style::default().bg(theme.bg_visual_range);
    }
    if selected {
        bg_color = Some(theme.bg_selected);
        row_style = Style::default()
            .bg(theme.bg_selected)
            .add_modifier(Modifier::BOLD);
        if selected_side == ReviewSide::Left {
            left_style = left_style.add_modifier(Modifier::UNDERLINED);
        } else {
            right_style = right_style.add_modifier(Modifier::UNDERLINED);
        }
    }
    if let Some(bg) = bg_color {
        left_style = left_style.bg(bg);
        right_style = right_style.bg(bg);
    }

    let left_cell = format!("{}{}", left_prefix, left_text);
    let right_cell = format!("{}{}", right_prefix, right_text);
    let left_cell = format!("{:width$}", left_cell, width = left_width);
    let right_cell = format!("{:width$}", right_cell, width = right_width);

    let indicator = if selected {
        match selected_side {
            ReviewSide::Left => "L",
            ReviewSide::Right => "R",
        }
    } else if in_visual_range {
        "V"
    } else {
        " "
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
            " | ",
            match bg_color {
                Some(bg) => Style::default().fg(theme.border_panel).bg(bg),
                None => Style::default().fg(theme.border_panel),
            },
        ),
        Span::styled(right_cell, right_style),
    ]);
    if selected || in_visual_range {
        line = line.style(row_style);
    }
    line
}

fn render_inline_review_comment(
    _comment_id: i64,
    author: &str,
    body: &str,
    side: ReviewSide,
    resolved: bool,
    width: usize,
    left_width: usize,
    right_width: usize,
    selected: bool,
    theme: &ThemePalette,
) -> Line<'static> {
    let side_label = match side {
        ReviewSide::Left => "old",
        ReviewSide::Right => "new",
    };
    let prefix = if selected { ">" } else { " " };
    let resolved_label = if resolved { "done" } else { "open" };
    let text = format!(
        "{} [{} {} @{}] {}",
        prefix,
        side_label,
        resolved_label,
        author,
        ellipsize(body, width.saturating_sub(24))
    );

    let muted_left = " ".repeat(left_width);
    let muted_right = " ".repeat(right_width);
    let comment_width = width.saturating_sub(8);
    let text = ellipsize(text.as_str(), comment_width);
    let comment_style = Style::default()
        .fg(theme.border_popup)
        .bg(theme.bg_panel_alt);
    let mut line = if side == ReviewSide::Left {
        let left_text = format!("{:width$}", text, width = left_width);
        Line::from(vec![
            Span::styled(left_text, comment_style),
            Span::styled(" | ", Style::default().fg(theme.border_panel)),
            Span::styled(muted_right, Style::default().fg(theme.text_muted)),
        ])
    } else {
        let right_text = format!("{:width$}", text, width = right_width);
        Line::from(vec![
            Span::styled(muted_left, Style::default().fg(theme.text_muted)),
            Span::styled(" | ", Style::default().fg(theme.border_panel)),
            Span::styled(right_text, comment_style),
        ])
    };
    if selected {
        line = line.style(
            Style::default()
                .bg(theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );
    }
    line
}

fn file_status_symbol(status: &str) -> &'static str {
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

fn file_status_color(status: &str, theme: &ThemePalette) -> Color {
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

fn pending_issue_span(pending: Option<&str>, theme: &ThemePalette) -> Span<'static> {
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

fn wrapped_line_count(lines: &[Line<'_>], width: u16) -> usize {
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

fn ellipsize(input: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if input.chars().count() <= max {
        return input.to_string();
    }
    if max <= 3 {
        return ".".repeat(max);
    }
    let head = input
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>();
    format!("{}...", head)
}

fn clip_horizontal(input: &str, offset: usize, max: usize) -> String {
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

fn comment_header(
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

fn format_comment_date(created_at: Option<&str>) -> Option<String> {
    format_datetime(created_at)
}

fn format_datetime(value: Option<&str>) -> Option<String> {
    let raw = value?;
    if raw.len() >= 16 {
        return Some(raw[0..16].replace('T', " "));
    }
    if raw.is_empty() {
        return None;
    }
    Some(raw.to_string())
}

fn editor_cursor_position(text: &str) -> (u16, u16) {
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
