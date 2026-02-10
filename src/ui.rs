use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{
    App,
    EditorMode,
    Focus,
    IssueFilter,
    PullRequestReviewFocus,
    ReviewSide,
    View,
};
use crate::markdown;
use crate::pr_diff::{parse_patch, DiffKind};

const GITHUB_BLUE: Color = Color::Rgb(65, 105, 225);
const GITHUB_GREEN: Color = Color::Rgb(74, 222, 128);
const GITHUB_RED: Color = Color::Rgb(234, 92, 124);
const GITHUB_VIOLET: Color = Color::Rgb(145, 171, 255);
const GITHUB_BG: Color = Color::Rgb(0, 0, 0);
const GITHUB_PANEL: Color = Color::Rgb(0, 0, 0);
const GITHUB_PANEL_ALT: Color = Color::Rgb(0, 0, 0);
const GITHUB_MUTED: Color = Color::Rgb(124, 138, 175);
const PANEL_BORDER: Color = Color::Rgb(35, 50, 88);
const FOCUS_BORDER: Color = Color::Rgb(105, 138, 255);
const POPUP_BORDER: Color = Color::Rgb(128, 160, 255);
const POPUP_BG: Color = Color::Rgb(0, 0, 0);
const OVERLAY_BG: Color = Color::Rgb(0, 0, 0);
const TEXT_PRIMARY: Color = Color::Rgb(226, 235, 255);
const SELECT_BG: Color = Color::Rgb(12, 24, 54);
const VISUAL_RANGE_BG: Color = Color::Rgb(7, 15, 36);
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
        View::PullRequestFiles => draw_pull_request_files(frame, app, area),
        View::LabelPicker => draw_label_picker(frame, app, area),
        View::AssigneePicker => draw_assignee_picker(frame, app, area),
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
    let visible_count = app.filtered_repo_rows().len();
    let total_count = app.repos().len();
    let header = Text::from(vec![
        Line::from(vec![
            Span::styled("Repositories", Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                format!("{} shown", visible_count),
                Style::default().fg(TEXT_PRIMARY),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} total", total_count),
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
            .style(Style::default().fg(TEXT_PRIMARY)),
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

    let block = panel_block("Repositories");
    let items = if app.filtered_repo_rows().is_empty() {
        if app.repos().is_empty() {
            vec![ListItem::new("No repos found. Run `glyph sync` or press Ctrl+R to rescan.")]
        } else {
            vec![ListItem::new("No repos match current search. Press Esc to clear.")]
        }
    } else {
        app.filtered_repo_rows()
            .iter()
            .map(|repo| {
                let line1 = Line::from(vec![
                    Span::styled(
                        format!("{} / {}", repo.owner, repo.repo),
                        Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("{}", repo.remote_name),
                        Style::default().fg(GITHUB_MUTED),
                    ),
                ]);
                let line2 = Line::from(ellipsize(repo.path.as_str(), 96))
                    .style(Style::default().fg(GITHUB_MUTED));
                ListItem::new(vec![line1, line2])
            })
            .collect()
    };
    let list = List::new(items)
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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
            app.filtered_repo_rows().len(),
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
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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
        issue_tabs_line(app.issue_filter(), open_count, closed_count),
        Line::from(vec![
            Span::styled("mode: ", Style::default().fg(GITHUB_MUTED)),
            Span::styled(
                item_label,
                Style::default()
                    .fg(GITHUB_BLUE)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            Span::raw("  "),
            Span::styled("(p toggle)", Style::default().fg(GITHUB_MUTED)),
            Span::raw("  "),
            Span::styled("assignee: ", Style::default().fg(GITHUB_MUTED)),
            if app.has_assignee_filter() {
                Span::styled(
                    assignee.clone(),
                    Style::default()
                        .fg(GITHUB_BLUE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )
            } else {
                Span::styled(assignee.clone(), Style::default().fg(GITHUB_MUTED))
            },
            Span::raw("  "),
            Span::styled("(a cycle)", Style::default().fg(GITHUB_MUTED)),
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
            .style(Style::default().fg(TEXT_PRIMARY)),
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
    let list_block_title = focused_title(list_title, list_focused);
    let block = panel_block_with_border(list_block_title.as_str(), focus_border(list_focused));
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
                let line1 = Line::from(vec![
                    Span::styled(
                        if issue.is_pr {
                            format!("PR #{} ", issue.number)
                        } else {
                            format!("#{} ", issue.number)
                        },
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
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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
                    if issue.is_pr {
                        format!("PR #{}", issue.number)
                    } else {
                        format!("#{}", issue.number)
                    },
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
            (preview_title_text.to_string(), lines)
        }
        None => (
            preview_title_text.to_string(),
            vec![Line::from(if item_mode == crate::app::WorkItemMode::PullRequests {
                "Select a pull request to preview."
            } else {
                "Select an issue to preview."
            })],
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
        focus_border(preview_focused),
    );
    let preview_widget = Paragraph::new(Text::from(preview_lines))
        .block(preview_block)
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL_ALT))
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
        Text::from(Line::from("Issue detail"))
    } else {
        let pending = issue_number.and_then(|number| app.pending_issue_badge(number));
        Text::from(vec![Line::from(vec![
            Span::styled(issue_title.clone(), Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", issue_state),
                Style::default()
                    .fg(issue_state_color(issue_state.as_str()))
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
            .style(Style::default().fg(TEXT_PRIMARY)),
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

    let is_pr = app.current_issue_row().is_some_and(|issue| issue.is_pr);
    let mut side_lines = Vec::new();
    if is_pr {
        side_lines.push(Line::from(Span::styled(
            "Press Enter for full-screen changes",
            Style::default().fg(POPUP_BORDER).add_modifier(Modifier::BOLD),
        )));
        side_lines.push(Line::from(""));
    } else {
        side_lines.push(Line::from(Span::styled(
            "Press Enter for full comments",
            Style::default().fg(POPUP_BORDER).add_modifier(Modifier::BOLD),
        )));
        side_lines.push(Line::from(""));
    }
    if is_pr {
        if app.pull_request_files_syncing() {
            side_lines.push(Line::from("Loading pull request changes..."));
        } else if app.pull_request_files().is_empty() {
            side_lines.push(Line::from("No changed files cached yet. Press r to refresh."));
        } else {
            for file in app.pull_request_files() {
                side_lines.push(Line::from(vec![
                    Span::styled(file_status_symbol(file.status.as_str()), Style::default().fg(file_status_color(file.status.as_str()))),
                    Span::raw(" "),
                    Span::styled(file.filename.clone(), Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                ]));
                side_lines.push(
                    Line::from(format!(
                        "  +{} -{}",
                        file.additions,
                        file.deletions
                    ))
                    .style(Style::default().fg(GITHUB_MUTED)),
                );
                if let Some(patch) = file.patch.as_deref() {
                    for patch_line in patch.lines().take(8) {
                        side_lines.push(styled_patch_line(patch_line, 100));
                    }
                    if patch.lines().count() > 8 {
                        side_lines.push(
                            Line::from("  ...")
                                .style(Style::default().fg(GITHUB_MUTED)),
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
        .style(Style::default().fg(TEXT_PRIMARY).bg(if body_focused {
            GITHUB_PANEL_ALT
        } else {
            GITHUB_PANEL
        }))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(body_paragraph, panes[0]);

    let side_content_width = panes[1].width.saturating_sub(2);
    let side_viewport = panes[1].height.saturating_sub(2) as usize;
    let side_total_lines = wrapped_line_count(&side_lines, side_content_width);
    let side_max_scroll = side_total_lines.saturating_sub(side_viewport) as u16;
    app.set_issue_recent_comments_max_scroll(side_max_scroll);
    let side_scroll = app.issue_recent_comments_scroll();
    let side_border = focus_border(comments_focused);
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
        .border_style(Style::default().fg(side_border));
    let side_paragraph = Paragraph::new(Text::from(side_lines))
        .block(side_block)
        .style(Style::default().fg(TEXT_PRIMARY).bg(if comments_focused {
            GITHUB_PANEL_ALT
        } else {
            GITHUB_PANEL
        }))
        .wrap(Wrap { trim: false })
        .scroll((side_scroll, 0));
    frame.render_widget(side_paragraph, panes[1]);

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
        Line::from(Span::styled(title.clone(), Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(
            format!("j/k jump comments • selected {} • e edit • x delete", selected),
            Style::default().fg(GITHUB_MUTED),
        )),
    ]);
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PANEL_BORDER))
        .style(Style::default().bg(GITHUB_PANEL));
    frame.render_widget(
        Paragraph::new(header)
            .block(header_block)
            .style(Style::default().fg(TEXT_PRIMARY)),
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
                index == app.selected_comment(),
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
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, content_area);

    draw_status(frame, app, footer);
}

fn draw_pull_request_files(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
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
    let header = Text::from(vec![
        Line::from(Span::styled(
            title.clone(),
            Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Ctrl+h/l pane • h/l side • w viewed • z collapse hunk • Shift+V visual • m comment • e edit • x delete • Shift+R resolve thread • focus:{} side:{} mode:{} range:{}", focused, side, visual, visual_range),
            Style::default().fg(GITHUB_MUTED),
        )),
    ]);
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PANEL_BORDER))
        .style(Style::default().bg(GITHUB_PANEL));
    frame.render_widget(
        Paragraph::new(header)
            .block(header_block)
            .style(Style::default().fg(TEXT_PRIMARY)),
        sections[0].inner(Margin {
            vertical: 0,
            horizontal: 2,
        }),
    );

    let file_items = if app.pull_request_files().is_empty() {
        vec![ListItem::new("No changed files cached yet. Press r to refresh.")]
    } else {
        app.pull_request_files()
            .iter()
            .map(|file| {
                let comment_count = app.pull_request_comments_count_for_path(file.filename.as_str());
                let viewed = app.pull_request_file_is_viewed(file.filename.as_str());
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if viewed { "✓" } else { "·" },
                        if viewed {
                            Style::default().fg(GITHUB_GREEN).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(GITHUB_MUTED)
                        },
                    ),
                    Span::raw(" "),
                    Span::styled(
                        file_status_symbol(file.status.as_str()),
                        Style::default().fg(file_status_color(file.status.as_str())),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        ellipsize(file.filename.as_str(), 34),
                        Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("+{} -{}", file.additions, file.deletions),
                        Style::default().fg(GITHUB_MUTED),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("c:{}", comment_count),
                        Style::default().fg(POPUP_BORDER),
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
            focus_border(files_focused),
        ))
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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
                Style::default().fg(GITHUB_MUTED),
            )));
        } else {
            row_offsets = vec![None; rows.len()];
            let panel_width = panes[1].width.saturating_sub(2) as usize;
            let cells_width = panel_width.saturating_sub(2);
            let left_width = cells_width.saturating_sub(5) / 2;
            let right_width = cells_width.saturating_sub(left_width + 3);
            let visual_range = app.pull_request_visual_range();
            for (index, row) in rows.iter().enumerate() {
                if app.pull_request_diff_row_hidden(file_name.as_str(), rows.as_slice(), index) {
                    continue;
                }
                row_offsets[index] = Some(lines.len() as u16);
                let selected = index == app.selected_pull_request_diff_line();
                let in_visual_range = visual_range
                    .is_some_and(|(start, end)| index >= start && index <= end);

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
                    let mut style = Style::default().fg(POPUP_BORDER).add_modifier(Modifier::BOLD);
                    if in_visual_range {
                        style = style.bg(VISUAL_RANGE_BG);
                    }
                    if selected {
                        style = style.bg(SELECT_BG);
                    }
                    let text = format!(
                        " {} {}  [{} lines hidden]",
                        indicator,
                        ellipsize(row.raw.as_str(), panel_width.saturating_sub(24)),
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
                ));

                let target_right = row
                    .new_line
                    .map(|line| app.pull_request_comments_for_path_and_line(
                        file_name.as_str(),
                        ReviewSide::Right,
                        line,
                    ))
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
                    ));
                }

                let target_left = row
                    .old_line
                    .map(|line| app.pull_request_comments_for_path_and_line(
                        file_name.as_str(),
                        ReviewSide::Left,
                        line,
                    ))
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
        .map(|(file_name, _)| format!("Diff: {}", file_name))
        .unwrap_or_else(|| "Diff".to_string());
    let diff_block_title = focused_title(diff_title.as_str(), diff_focused);
    let paragraph = Paragraph::new(Text::from(lines))
        .block(panel_block_with_border(
            diff_block_title.as_str(),
            focus_border(diff_focused),
        ))
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, panes[1]);

    draw_status(frame, app, footer);
}

fn draw_label_picker(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    draw_modal_background(frame, app, area);
    let popup = centered_rect(74, 76, area);
    frame.render_widget(Clear, popup);
    let shell = popup_block("Label Picker");
    let popup_inner = shell.inner(popup).inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(shell, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
        .split(popup_inner);

    let filtered = app.filtered_label_indices();
    let query_display = if app.label_query().trim().is_empty() {
        "none".to_string()
    } else {
        ellipsize(app.label_query().trim(), 56)
    };
    let header = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Edit Labels",
            Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(GITHUB_MUTED)),
            Span::raw(query_display),
        ]),
        Line::from(Span::styled(
            "Type to filter • Space toggle • Enter apply • Ctrl+u clear • Esc cancel",
            Style::default().fg(GITHUB_MUTED),
        )),
    ]))
    .block(panel_block_with_border("Labels", POPUP_BORDER))
    .style(Style::default().fg(TEXT_PRIMARY).bg(POPUP_BG));
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
                ListItem::new(Line::from(vec![
                    Span::styled(checked, Style::default().fg(GITHUB_BLUE)),
                    Span::raw(" "),
                    Span::raw(label.clone()),
                ]))
            })
            .collect::<Vec<ListItem>>()
    };
    let list = List::new(items)
        .block(panel_block_with_border("Available labels", POPUP_BORDER))
        .style(Style::default().fg(TEXT_PRIMARY).bg(POPUP_BG))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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

    let selection = if app.selected_labels_csv().is_empty() {
        "selected: none".to_string()
    } else {
        format!("selected: {}", ellipsize(app.selected_labels_csv().as_str(), 80))
    };
    let footer = Paragraph::new(selection)
        .style(Style::default().fg(GITHUB_MUTED))
        .block(panel_block_with_border("Selection", POPUP_BORDER));
    frame.render_widget(footer, sections[2]);
}

fn draw_assignee_picker(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    draw_modal_background(frame, app, area);
    let popup = centered_rect(74, 76, area);
    frame.render_widget(Clear, popup);
    let shell = popup_block("Assignee Picker");
    let popup_inner = shell.inner(popup).inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(shell, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
        .split(popup_inner);

    let filtered = app.filtered_assignee_indices();
    let query_display = if app.assignee_query().trim().is_empty() {
        "none".to_string()
    } else {
        ellipsize(app.assignee_query().trim(), 56)
    };
    let header = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Edit Assignees",
            Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(GITHUB_MUTED)),
            Span::raw(query_display),
        ]),
        Line::from(Span::styled(
            "Type to filter • Space toggle • Enter apply • Ctrl+u clear • Esc cancel",
            Style::default().fg(GITHUB_MUTED),
        )),
    ]))
    .block(panel_block_with_border("Assignees", POPUP_BORDER))
    .style(Style::default().fg(TEXT_PRIMARY).bg(POPUP_BG));
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
                ListItem::new(Line::from(vec![
                    Span::styled(checked, Style::default().fg(GITHUB_BLUE)),
                    Span::raw(" "),
                    Span::raw(assignee.clone()),
                ]))
            })
            .collect::<Vec<ListItem>>()
    };
    let list = List::new(items)
        .block(panel_block_with_border("Available assignees", POPUP_BORDER))
        .style(Style::default().fg(TEXT_PRIMARY).bg(POPUP_BG))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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

    let selection = if app.selected_assignees_csv().is_empty() {
        "selected: none".to_string()
    } else {
        format!(
            "selected: {}",
            ellipsize(app.selected_assignees_csv().as_str(), 80)
        )
    };
    let footer = Paragraph::new(selection)
        .style(Style::default().fg(GITHUB_MUTED))
        .block(panel_block_with_border("Selection", POPUP_BORDER));
    frame.render_widget(footer, sections[2]);
}

fn draw_preset_picker(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let close_title = if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        "Close Pull Request"
    } else {
        "Close Issue"
    };
    let block = panel_block(close_title);
    let mut items = Vec::new();
    items.push(ListItem::new("Close without comment"));
    items.push(ListItem::new("Custom message..."));
    for preset in app.comment_defaults() {
        items.push(ListItem::new(preset.name.as_str()));
    }
    items.push(ListItem::new("Add preset..."));

    let list = List::new(items)
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(SELECT_BG)
                .fg(TEXT_PRIMARY)
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
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
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
    let block = panel_block(title);
    let text = app.editor().text();
    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(TEXT_PRIMARY).bg(GITHUB_PANEL))
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
        lines.push(Line::from(vec![
            Span::styled("status ", Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(status, Style::default().fg(TEXT_PRIMARY)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("context ", Style::default().fg(GITHUB_GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(context, Style::default().fg(GITHUB_MUTED)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("keys ", Style::default().fg(GITHUB_VIOLET).add_modifier(Modifier::BOLD)),
        Span::styled(help, Style::default().fg(GITHUB_MUTED)),
    ]));
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(GITHUB_MUTED).bg(GITHUB_PANEL))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default().bg(GITHUB_PANEL))
                .border_style(Style::default().fg(PANEL_BORDER)),
        );
    frame.render_widget(paragraph, area.inner(Margin { vertical: 0, horizontal: 2 }));
}

fn panel_block(title: &str) -> Block<'_> {
    panel_block_with_border(title, PANEL_BORDER)
}

fn popup_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Line::from(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(POPUP_BORDER)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .style(Style::default().bg(POPUP_BG).fg(TEXT_PRIMARY))
        .border_style(Style::default().fg(POPUP_BORDER))
}

fn focused_title(title: &str, focused: bool) -> String {
    if focused {
        return format!("> {}", title);
    }
    title.to_string()
}

fn panel_block_with_border(title: &str, border: Color) -> Block<'_> {
    let title_color = if border == FOCUS_BORDER {
        FOCUS_BORDER
    } else {
        GITHUB_BLUE
    };
    let border_type = if border == FOCUS_BORDER {
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
        .style(Style::default().bg(GITHUB_PANEL).fg(TEXT_PRIMARY))
        .border_style(Style::default().fg(border))
}

fn focus_border(focused: bool) -> Color {
    if focused {
        FOCUS_BORDER
    } else {
        PANEL_BORDER
    }
}

fn draw_modal_background(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    match app.editor_cancel_view() {
        View::Issues => draw_issues(frame, app, area),
        View::IssueDetail => draw_issue_detail(frame, app, area),
        View::IssueComments => draw_issue_comments(frame, app, area),
        View::PullRequestFiles => draw_pull_request_files(frame, app, area),
        _ => {
            let (main, footer) = split_area(area);
            frame.render_widget(panel_block("Glyph"), main);
            draw_status(frame, app, footer);
        }
    }
    frame.render_widget(Block::default().style(Style::default().bg(OVERLAY_BG)), area);
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
            let reviewing_pr = app
                .selected_issue_row()
                .is_some_and(|issue| issue.is_pr)
                || app.work_item_mode() == crate::app::WorkItemMode::PullRequests;
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
                return "Ctrl+h/l pane • j/k scroll • Enter on description opens comments • Enter on changes opens review • c comments • h/l side in review • m comment • l labels • Shift+A assignees • u reopen • dd close • v checkout • r refresh • Esc back • q quit"
                    .to_string();
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
                return "j/k comments • e edit • x delete • m comment • l labels • Shift+A assignees • u reopen • dd close • v checkout • r refresh • Esc back • q quit"
                    .to_string();
            }
            if app.selected_issue_has_known_linked_pr() {
                return "j/k comments • e edit • x delete • m comment • l labels • Shift+A assignees • u reopen • dd close • Shift+P linked PR (TUI) • Shift+O linked PR (web) • r refresh • Esc back • q quit"
                    .to_string();
            }
            "j/k comments • e edit • x delete • m comment • l labels • Shift+A assignees • u reopen • dd close • r refresh • Esc back • q quit"
                .to_string()
        }
        View::PullRequestFiles => {
            "Ctrl+h/l pane • j/k move file/line • w viewed • z collapse hunk • h/l old/new side • Shift+V visual range • m add • e edit • x delete • Shift+R resolve/reopen • n/p cycle line comments • r refresh • v checkout • Esc back • q quit"
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
    let sync = if app.syncing() {
        "syncing"
    } else if app.pull_request_files_syncing() {
        "loading pr files"
    } else if app.pull_request_review_comments_syncing() {
        "loading review comments"
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
        let item_mode = app.work_item_mode().label();
        return format!(
            "repo: {}  |  mode: {} ({})  |  assignee: {}  |  query: {}  |  status: {}",
            repo, mode, item_mode, assignee, query, sync
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
    Line::from(spans)
}

fn filter_tab(label: &str, count: usize, active: bool, color: Color) -> Span<'static> {
    let text = format!("{} ({})", label, count);
    if active {
        return Span::styled(
            format!("[{}]", text),
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        );
    }
    Span::styled(text, Style::default().fg(GITHUB_MUTED))
}

fn issue_state_color(state: &str) -> Color {
    if state.eq_ignore_ascii_case("closed") {
        return GITHUB_RED;
    }
    GITHUB_GREEN
}

fn styled_patch_line(line: &str, width: usize) -> Line<'static> {
    let trimmed = ellipsize(line, width);
    if trimmed.starts_with("+++") || trimmed.starts_with("---") {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default().fg(FOCUS_BORDER).add_modifier(Modifier::BOLD),
        ));
    }
    if trimmed.starts_with("@@") {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default().fg(POPUP_BORDER).add_modifier(Modifier::BOLD),
        ));
    }
    if trimmed.starts_with('+') {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default().fg(GITHUB_GREEN),
        ));
    }
    if trimmed.starts_with('-') {
        return Line::from(Span::styled(
            format!("  {}", trimmed),
            Style::default().fg(GITHUB_RED),
        ));
    }
    Line::from(Span::styled(
        format!("  {}", trimmed),
        Style::default().fg(GITHUB_MUTED),
    ))
}

fn render_split_diff_row(
    row: &crate::pr_diff::DiffRow,
    selected: bool,
    in_visual_range: bool,
    selected_side: ReviewSide,
    left_width: usize,
    right_width: usize,
) -> Line<'static> {
    if row.kind == DiffKind::Hunk {
        return Line::from(Span::styled(
            format!(" {}", ellipsize(row.raw.as_str(), left_width + right_width + 4)),
            Style::default().fg(POPUP_BORDER).add_modifier(Modifier::BOLD),
        ));
    }
    if row.kind == DiffKind::Meta {
        return Line::from(Span::styled(
            format!(" {}", ellipsize(row.raw.as_str(), left_width + right_width + 4)),
            Style::default().fg(GITHUB_MUTED),
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
    let left_text = ellipsize(row.left.as_str(), left_width.saturating_sub(5));
    let right_text = ellipsize(row.right.as_str(), right_width.saturating_sub(5));

    let mut left_style = Style::default().fg(GITHUB_MUTED);
    let mut right_style = Style::default().fg(GITHUB_MUTED);
    match row.kind {
        DiffKind::Changed => {
            left_style = Style::default().fg(GITHUB_RED);
            right_style = Style::default().fg(GITHUB_GREEN);
        }
        DiffKind::Added => {
            right_style = Style::default().fg(GITHUB_GREEN);
        }
        DiffKind::Removed => {
            left_style = Style::default().fg(GITHUB_RED);
        }
        DiffKind::Context => {
            left_style = Style::default().fg(TEXT_PRIMARY);
            right_style = Style::default().fg(TEXT_PRIMARY);
        }
        _ => {}
    }

    let mut row_style = Style::default();
    let mut bg_color = None;
    if in_visual_range {
        bg_color = Some(VISUAL_RANGE_BG);
        row_style = Style::default().bg(VISUAL_RANGE_BG);
    }
    if selected {
        bg_color = Some(SELECT_BG);
        row_style = Style::default().bg(SELECT_BG).add_modifier(Modifier::BOLD);
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
                    .fg(POPUP_BORDER)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
                None => Style::default().fg(POPUP_BORDER).add_modifier(Modifier::BOLD),
            },
        ),
        Span::styled(left_cell, left_style),
        Span::styled(
            " | ",
            match bg_color {
                Some(bg) => Style::default().fg(PANEL_BORDER).bg(bg),
                None => Style::default().fg(PANEL_BORDER),
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
    let comment_style = Style::default().fg(POPUP_BORDER).bg(GITHUB_PANEL_ALT);
    let mut line = if side == ReviewSide::Left {
        let left_text = format!("{:width$}", text, width = left_width);
        Line::from(vec![
            Span::styled(left_text, comment_style),
            Span::styled(" | ", Style::default().fg(PANEL_BORDER)),
            Span::styled(muted_right, Style::default().fg(GITHUB_MUTED)),
        ])
    } else {
        let right_text = format!("{:width$}", text, width = right_width);
        Line::from(vec![
            Span::styled(muted_left, Style::default().fg(GITHUB_MUTED)),
            Span::styled(" | ", Style::default().fg(PANEL_BORDER)),
            Span::styled(right_text, comment_style),
        ])
    };
    if selected {
        line = line.style(Style::default().bg(SELECT_BG).add_modifier(Modifier::BOLD));
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

fn file_status_color(status: &str) -> Color {
    if status.eq_ignore_ascii_case("added") {
        return GITHUB_GREEN;
    }
    if status.eq_ignore_ascii_case("removed") {
        return GITHUB_RED;
    }
    if status.eq_ignore_ascii_case("renamed") {
        return GITHUB_BLUE;
    }
    if status.eq_ignore_ascii_case("modified") {
        return GITHUB_VIOLET;
    }
    GITHUB_MUTED
}

fn pending_issue_span(pending: Option<&str>) -> Span<'static> {
    match pending {
        Some(label) => Span::styled(
            format!("  [{}]", label),
            Style::default()
                .fg(GITHUB_VIOLET)
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

fn comment_header(index: usize, author: &str, created_at: Option<&str>, selected: bool) -> Line<'static> {
    let mut spans = Vec::new();
    if selected {
        spans.push(Span::styled("▸ ", Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD)));
    } else {
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(
        format!("{}  {}", index, author),
        Style::default()
            .fg(if selected { TEXT_PRIMARY } else { GITHUB_BLUE })
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
