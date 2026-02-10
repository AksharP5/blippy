use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, EditorMode, Focus, IssueFilter, View};
use crate::markdown;

const GITHUB_BLUE: Color = Color::Rgb(88, 166, 255);
const GITHUB_GREEN: Color = Color::Rgb(63, 185, 80);
const GITHUB_RED: Color = Color::Rgb(248, 81, 73);
const GITHUB_BG: Color = Color::Rgb(13, 17, 23);
const GITHUB_PANEL: Color = Color::Rgb(22, 27, 34);
const GITHUB_PANEL_ALT: Color = Color::Rgb(28, 34, 43);
const GITHUB_MUTED: Color = Color::Rgb(139, 148, 158);
const PANEL_BORDER: Color = Color::Rgb(48, 54, 61);
const SELECT_BG: Color = Color::Rgb(33, 58, 89);
const RECENT_COMMENTS_HEIGHT: u16 = 10;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(GITHUB_BG)), area);
    match app.view() {
        View::RepoPicker => draw_repo_picker(frame, app, area),
        View::RemoteChooser => draw_remote_chooser(frame, app, area),
        View::Issues => draw_issues(frame, app, area),
        View::IssueDetail => draw_issue_detail(frame, app, area),
        View::IssueComments => draw_issue_comments(frame, app, area),
        View::CommentPresetPicker => draw_preset_picker(frame, app, area),
        View::CommentPresetName => draw_preset_name(frame, app, area),
        View::CommentEditor => draw_comment_editor(frame, app, area),
    }
}

fn draw_repo_picker(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
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
    let grouped_count = app.repo_picker_entries().len();
    let total_remotes = app.repos().len();
    let header = Text::from(vec![
        Line::from(vec![
            Span::styled("Repositories", Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                format!("{} groups", grouped_count),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} remotes", total_remotes),
                Style::default().fg(GITHUB_MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("search: ", Style::default().fg(GITHUB_MUTED)),
            Span::raw(query_display.clone()),
            Span::raw("  "),
            Span::styled("(/ to search)", Style::default().fg(GITHUB_MUTED)),
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
                    .border_style(Style::default().fg(PANEL_BORDER))
                    .style(Style::default().bg(GITHUB_PANEL)),
            )
            .style(Style::default().fg(Color::White)),
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

    let block = panel_block("Repository Groups");
    let items = if app.repo_picker_entries().is_empty() {
        if app.repos().is_empty() {
            vec![ListItem::new("No repos found. Run `glyph sync` or press Ctrl+R to rescan.")]
        } else {
            vec![ListItem::new("No repos match current search. Press Esc to clear.")]
        }
    } else {
        app.repo_picker_entries()
            .iter()
            .map(|repo| {
                let line1 = Line::from(vec![
                    Span::styled(
                        format!("{} / {}", repo.owner, repo.repo),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("{} remotes", repo.remotes),
                        Style::default().fg(GITHUB_MUTED),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("{} paths", repo.paths),
                        Style::default().fg(GITHUB_MUTED),
                    ),
                ]);
                let last_seen = repo
                    .last_seen
                    .as_deref()
                    .map(|value| format!("last seen {}", value.replace('T', " ")))
                    .unwrap_or_else(|| "last seen unknown".to_string());
                let line2 = Line::from(last_seen).style(Style::default().fg(GITHUB_MUTED));
                ListItem::new(vec![line1, line2])
            })
            .collect()
    };
    let list = List::new(items)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        sections[1].inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(selected_for_list(
            app.selected_repo(),
            app.repo_picker_entries().len(),
        )),
    );

    draw_status(frame, app, footer);
}

fn draw_remote_chooser(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let block = panel_block("Choose Remote");
    let items = app
        .remotes()
        .iter()
        .map(|remote| {
            let label = format!("{} -> {}/{}", remote.name, remote.slug.owner, remote.slug.repo);
            ListItem::new(label)
        })
        .collect::<Vec<ListItem>>();
    let list = List::new(items)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        main.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_remote()),
    );

    draw_status(frame, app, footer);
}

fn draw_issues(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(main);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(sections[1]);

    let visible_issues = app.issues_for_view();
    let (open_count, closed_count) = app.issue_counts();
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
    let total_count = app.issues().len();
    let header_text = Text::from(vec![
        issue_tabs_line(app.issue_filter(), open_count, closed_count),
        Line::from(vec![
            Span::styled("assignee: ", Style::default().fg(GITHUB_MUTED)),
            if app.has_assignee_filter() {
                Span::styled(
                    assignee.clone(),
                    Style::default()
                        .fg(Color::Black)
                        .bg(GITHUB_BLUE)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(assignee.clone(), Style::default().fg(GITHUB_MUTED))
            },
            Span::raw("  "),
            Span::styled("(a/A cycle)", Style::default().fg(GITHUB_MUTED)),
            Span::raw("  "),
            Span::styled(
                format!("showing {} of {}", visible_count, total_count),
                Style::default().fg(GITHUB_MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("search: ", Style::default().fg(GITHUB_MUTED)),
            Span::raw(query_display.clone()),
        ]),
    ]);
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PANEL_BORDER))
        .style(Style::default().bg(GITHUB_PANEL));
    let header_area = sections[0].inner(Margin {
        vertical: 0,
        horizontal: 2,
    });
    frame.render_widget(
        Paragraph::new(header_text)
            .block(header_block)
            .style(Style::default().fg(Color::White)),
        header_area,
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
    let block = panel_block_with_border("Issue list", focus_border(list_focused));
    let items = if visible_issues.is_empty() {
        if app.issues().is_empty() {
            vec![ListItem::new("No cached issues yet. Run `glyph sync`.")]
        } else {
            vec![ListItem::new("No issues match current filter. Press `f` to switch.")]
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
                let line1 = Line::from(vec![
                    Span::styled(
                        format!("#{} ", issue.number),
                        Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("[{}] ", issue.state),
                        Style::default().fg(issue_state_color(issue.state.as_str())),
                    ),
                    Span::raw(issue.title.clone()),
                    pending_issue_span(app.pending_issue_badge(issue.number)),
                ]);
                let line2 = Line::from(format!(
                    "@{}  comments:{}  labels:{}",
                    ellipsize(assignees, 20),
                    issue.comments_count,
                    ellipsize(labels, 24)
                ))
                .style(Style::default().fg(GITHUB_MUTED));
                ListItem::new(vec![line1, line2])
            })
            .collect()
    };
    let list = List::new(items)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        panes[0].inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(selected_for_list(app.selected_issue(), visible_issues.len())),
    );

    let (preview_title, preview_lines) = match app.selected_issue_row() {
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
                    format!("#{}", issue.number),
                    Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", issue.state),
                    Style::default().fg(issue_state_color(issue.state.as_str())),
                ),
            ]));
            lines.push(Line::from(format!("assignees: {}", assignees)));
            lines.push(Line::from(format!("comments: {}", issue.comments_count)));
            lines.push(Line::from(format!(
                "labels: {}",
                ellipsize(labels.as_str(), 80)
            )));
            if let Some(updated) = format_datetime(issue.updated_at.as_deref()) {
                lines.push(Line::from(format!("updated: {}", updated)));
            }
            lines.push(Line::from(""));

            let rendered = markdown::render(issue.body.as_str());
            if rendered.lines.is_empty() {
                lines.push(Line::from("No description."));
            } else {
                lines.extend(rendered.lines);
            }
            ("Issue Preview".to_string(), lines)
        }
        None => (
            "Issue Preview".to_string(),
            vec![Line::from("Select an issue to preview.")],
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
    let preview_block = panel_block_with_border(&preview_title, focus_border(preview_focused));
    let preview_widget = Paragraph::new(Text::from(preview_lines))
        .block(preview_block)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL_ALT))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(preview_widget, preview_area);

    draw_status(frame, app, footer);
}

fn draw_issue_detail(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(main);
    let content_area = sections[1].inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let body_focused = app.focus() == Focus::IssueBody;
    let comments_focused = app.focus() == Focus::IssueRecentComments;
    let (issue_number, issue_title, issue_state, body, assignees, labels, comment_count, updated_at) =
        match app.current_issue_row() {
            Some(issue) => (
                Some(issue.number),
                format!("#{} {}", issue.number, issue.title),
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
        Text::from(Line::from("Issue detail"))
    } else {
        let pending = issue_number.and_then(|number| app.pending_issue_badge(number));
        Text::from(vec![Line::from(vec![
            Span::styled(issue_title.clone(), Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                issue_state.clone(),
                Style::default()
                    .fg(Color::Black)
                    .bg(issue_state_color(issue_state.as_str()))
                    .add_modifier(Modifier::BOLD),
            ),
            pending_issue_span(pending),
        ])])
    };
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PANEL_BORDER))
        .style(Style::default().bg(GITHUB_PANEL));
    frame.render_widget(
        Paragraph::new(header_text)
            .block(header_block)
            .style(Style::default().fg(Color::White)),
        sections[0].inner(Margin {
            vertical: 0,
            horizontal: 2,
        }),
    );

    let mut body_lines = Vec::new();
    if issue_title.is_empty() {
        body_lines.push(Line::from("No issue selected."));
    } else {
        body_lines.push(Line::from(Span::styled(
            issue_title,
            Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
        )));
    }
    let metadata = Line::from(format!(
        "assignees: {} | comments: {} | labels: {}",
        assignees,
        comment_count,
        ellipsize(labels.as_str(), 44)
    ));
    body_lines.push(metadata.style(Style::default().fg(GITHUB_MUTED)));
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

    let mut comment_lines = Vec::new();
    if app.comments().is_empty() {
        comment_lines.push(Line::from("No comments cached yet."));
    } else {
        let start = app.comments().len().saturating_sub(3);
        for (index, comment) in app.comments()[start..].iter().enumerate() {
            comment_lines.push(comment_header(
                start + index + 1,
                comment.author.as_str(),
                comment.created_at.as_deref(),
            ));
            let rendered_comment = markdown::render(comment.body.as_str());
            if rendered_comment.lines.is_empty() {
                comment_lines.push(Line::from(""));
            } else {
                for line in rendered_comment.lines {
                    comment_lines.push(line);
                }
            }
            comment_lines.push(Line::from(""));
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

    let body_block = Block::default()
        .title(Line::from(Span::styled(
            "Issue description",
            Style::default()
                .fg(if body_focused { GITHUB_BLUE } else { GITHUB_MUTED })
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(focus_border(body_focused)))
        .style(Style::default().bg(if body_focused {
            GITHUB_PANEL_ALT
        } else {
            GITHUB_PANEL
        }));
    let body_paragraph = Paragraph::new(Text::from(body_lines))
        .block(body_block)
        .style(Style::default().fg(Color::White).bg(if body_focused {
            GITHUB_PANEL_ALT
        } else {
            GITHUB_PANEL
        }))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(body_paragraph, panes[0]);

    let comments_content_width = panes[1].width.saturating_sub(2);
    let comments_viewport = panes[1].height.saturating_sub(2) as usize;
    let comments_total_lines = wrapped_line_count(&comment_lines, comments_content_width);
    let comments_max_scroll = comments_total_lines.saturating_sub(comments_viewport) as u16;
    app.set_issue_recent_comments_max_scroll(comments_max_scroll);
    let comments_scroll = app.issue_recent_comments_scroll();
    let comments_border = focus_border(comments_focused);
    let comments_title = format!("Recent comments ({})", app.comments().len());
    let comment_block = Block::default()
        .title(Line::from(Span::styled(
            comments_title,
            Style::default()
                .fg(if comments_focused {
                    GITHUB_BLUE
                } else {
                    GITHUB_MUTED
                })
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(if comments_focused {
            GITHUB_PANEL_ALT
        } else {
            GITHUB_PANEL
        }))
        .border_style(Style::default().fg(comments_border));
    let comment_paragraph = Paragraph::new(Text::from(comment_lines))
        .block(comment_block)
        .style(Style::default().fg(Color::White).bg(if comments_focused {
            GITHUB_PANEL_ALT
        } else {
            GITHUB_PANEL
        }))
        .wrap(Wrap { trim: false })
        .scroll((comments_scroll, 0));
    frame.render_widget(comment_paragraph, panes[1]);

    draw_status(frame, app, footer);
}

fn draw_issue_comments(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
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
        Some(issue) => format!("Comments #{}", issue.number),
        None => "Comments (n/p jump)".to_string(),
    };
    let header = Text::from(vec![
        Line::from(Span::styled(title.clone(), Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("jump comments with n/p", Style::default().fg(GITHUB_MUTED))),
    ]);
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PANEL_BORDER))
        .style(Style::default().bg(GITHUB_PANEL));
    frame.render_widget(
        Paragraph::new(header)
            .block(header_block)
            .style(Style::default().fg(Color::White)),
        sections[0].inner(Margin {
            vertical: 0,
            horizontal: 2,
        }),
    );

    let block = panel_block(&title);
    let mut lines = Vec::new();
    if app.comments().is_empty() {
        lines.push(Line::from("No comments cached yet."));
    } else {
        for (index, comment) in app.comments().iter().enumerate() {
            lines.push(comment_header(
                index + 1,
                comment.author.as_str(),
                comment.created_at.as_deref(),
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
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, content_area);

    draw_status(frame, app, footer);
}

fn draw_preset_picker(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let block = panel_block("Close Issue");
    let mut items = Vec::new();
    items.push(ListItem::new("Close without comment"));
    items.push(ListItem::new("Custom message..."));
    for preset in app.comment_defaults() {
        items.push(ListItem::new(preset.name.as_str()));
    }
    items.push(ListItem::new("Add preset..."));

    let list = List::new(items)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        main.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_preset()),
    );

    draw_status(frame, app, footer);
}

fn draw_preset_name(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let input_area = main.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let block = panel_block("Preset Name");
    let text = app.editor().name();
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
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
            .min(text_area.x.saturating_add(text_area.width.saturating_sub(1)));
        frame.set_cursor_position((cursor_x, text_area.y));
    }

    draw_status(frame, app, footer);
}

fn draw_comment_editor(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let title = match app.editor_mode() {
        EditorMode::CloseIssue => "Close Issue Comment",
        EditorMode::AddComment => "Add Issue Comment",
        EditorMode::AddPreset => "Preset Body",
        EditorMode::EditLabels => "Edit Labels",
        EditorMode::EditAssignees => "Edit Assignees",
    };
    let editor_area = main.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let block = panel_block(title);
    let text = app.editor().text();
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White).bg(GITHUB_PANEL))
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

    draw_status(frame, app, footer);
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let status = app.status();
    let context = status_context(app);
    let help = help_text(app);
    let mut lines = Vec::new();
    if !status.is_empty() {
        lines.push(Line::from(status));
    }
    lines.push(Line::from(context));
    lines.push(Line::from(help));
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(GITHUB_MUTED).bg(GITHUB_BG))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(PANEL_BORDER)),
        );
    frame.render_widget(paragraph, area.inner(Margin { vertical: 0, horizontal: 2 }));
}

fn panel_block(title: &str) -> Block<'_> {
    panel_block_with_border(title, PANEL_BORDER)
}

fn panel_block_with_border(title: &str, border: Color) -> Block<'_> {
    Block::default()
        .title(Line::from(Span::styled(
            title.to_string(),
            Style::default()
                .fg(GITHUB_BLUE)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(GITHUB_PANEL).fg(Color::White))
        .border_style(Style::default().fg(border))
}

fn focus_border(focused: bool) -> Color {
    if focused {
        GITHUB_BLUE
    } else {
        PANEL_BORDER
    }
}

fn split_area(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(4)])
        .split(area);
    (chunks[0], chunks[1])
}

fn help_text(app: &App) -> String {
    match app.view() {
        View::RepoPicker => {
            if app.repo_search_mode() {
                return "Search repos: type query • Enter keep • Esc clear • Ctrl+u clear"
                    .to_string();
            }
            "Ctrl+R rescan • j/k or ↑/↓ move • Ctrl+u/d page • gg/G top/bottom • / search • Enter select • q quit"
                .to_string()
        }
        View::RemoteChooser => {
            "j/k or ↑/↓ move • gg/G top/bottom • Enter select • Ctrl+G repos • q quit"
                .to_string()
        }
        View::Issues => {
            if app.issue_search_mode() {
                return "Search: type terms/qualifiers (is:, label:, assignee:, #num) • Enter keep • Esc clear • Ctrl+u clear"
                    .to_string();
            }
            "Ctrl+h/j/k/l pane • j/k or ↑/↓ move/scroll • Ctrl+u/d page • gg/G top/bottom • / search • a/A assignee filter • l labels • Shift+a assignees • 1/2 or [ ] tabs • f cycle • m comment • u reopen • dd close • r refresh • o browser • Ctrl+G repos • q quit"
                .to_string()
        }
        View::IssueDetail => {
            "Ctrl+h/j/k/l pane • j/k scroll • Ctrl+u/d page • gg/G top/bottom • dd close • l labels • a assignees • m comment • u reopen • c all comments • b/Esc back • r sync issue+comments • o browser • Ctrl+G repos • q quit"
                .to_string()
        }
        View::IssueComments => {
            "j/k or ↑/↓ scroll • Ctrl+u/d page • gg/G top/bottom • n/p next/prev comment • dd close • l labels • a assignees • m comment • u reopen • b/Esc back • r sync issue+comments • o browser • q quit"
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
            if matches!(app.editor_mode(), EditorMode::EditLabels | EditorMode::EditAssignees) {
                return "Type comma-separated values • Enter submit • Esc cancel".to_string();
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
    let sync = if app.syncing() {
        "syncing"
    } else if app.comment_syncing() {
        "syncing comments"
    } else if app.scanning() {
        "scanning"
    } else {
        "idle"
    };
    if app.view() == View::Issues {
        let query = app.issue_query().trim();
        let query = if query.is_empty() {
            "none".to_string()
        } else {
            ellipsize(query, 24)
        };
        let assignee = ellipsize(app.assignee_filter_label().as_str(), 18);
        let mode = if app.issue_search_mode() { "search" } else { "browse" };
        return format!(
            "repo: {}  |  mode: {}  |  assignee: {}  |  query: {}  |  status: {}",
            repo, mode, assignee, query, sync
        );
    }
    format!("repo: {}  |  status: {}", repo, sync)
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

fn issue_tabs_line(filter: IssueFilter, open_count: usize, closed_count: usize) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(filter_tab("1 Open", open_count, filter == IssueFilter::Open, GITHUB_GREEN));
    spans.push(Span::raw("  "));
    spans.push(filter_tab(
        "2 Closed",
        closed_count,
        filter == IssueFilter::Closed,
        GITHUB_RED,
    ));
    spans.push(Span::raw("  "));
    spans.push(Span::styled("([ ] cycle)", Style::default().fg(GITHUB_MUTED)));
    Line::from(spans)
}

fn filter_tab(label: &str, count: usize, active: bool, color: Color) -> Span<'static> {
    let text = format!("{} ({})", label, count);
    if active {
        return Span::styled(
            text,
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        );
    }
    Span::styled(text, Style::default().fg(color))
}

fn issue_state_color(state: &str) -> Color {
    if state.eq_ignore_ascii_case("closed") {
        return GITHUB_RED;
    }
    GITHUB_GREEN
}

fn pending_issue_span(pending: Option<&str>) -> Span<'static> {
    match pending {
        Some(label) => Span::styled(
            format!("  [{}]", label),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(210, 153, 34))
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
    if input.chars().count() <= max {
        return input.to_string();
    }
    let head = input.chars().take(max.saturating_sub(3)).collect::<String>();
    format!("{}...", head)
}

fn comment_header(index: usize, author: &str, created_at: Option<&str>) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(Span::styled(
        format!("{}  {}", index, author),
        Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
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
