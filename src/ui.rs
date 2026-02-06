use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, View};
use crate::markdown;

const GITHUB_BLUE: Color = Color::Rgb(88, 166, 255);
const GITHUB_GREEN: Color = Color::Rgb(63, 185, 80);
const PANEL_BORDER: Color = Color::Rgb(48, 54, 61);
const SELECT_BG: Color = Color::Rgb(30, 41, 59);

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
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
    let block = panel_block("Repositories");
    let items = if app.repos().is_empty() {
        vec![ListItem::new("No repos found. Run `glyph sync` or press Ctrl+R to rescan.")]
    } else {
        app.repos()
            .iter()
            .map(|repo| {
                let label = format!("{} / {} ({})", repo.owner, repo.repo, repo.remote_name);
                ListItem::new(label)
            })
            .collect()
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(Style::default().bg(SELECT_BG));
    frame.render_stateful_widget(
        list,
        main.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_repo()),
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
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(Style::default().bg(SELECT_BG));
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

fn draw_issues(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(main);

    let block = panel_block("Issues");
    let items = if app.issues().is_empty() {
        vec![ListItem::new("No cached issues yet. Run `glyph sync`.")]
    } else {
        app.issues()
            .iter()
            .map(|issue| {
                let assignees = if issue.assignees.is_empty() {
                    "unassigned"
                } else {
                    issue.assignees.as_str()
                };
                let line1 = Line::from(vec![
                    Span::styled(
                        format!("#{} ", issue.number),
                        Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("[{}] ", issue.state),
                        Style::default().fg(GITHUB_GREEN),
                    ),
                    Span::raw(issue.title.as_str()),
                ]);
                let line2 = Line::from(format!(
                    "assignees: {}  comments: {}",
                    assignees, issue.comments_count
                ));
                ListItem::new(vec![line1, line2])
            })
            .collect()
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(Style::default().bg(SELECT_BG));
    frame.render_stateful_widget(
        list,
        panes[0].inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_issue()),
    );

    let preview_block = panel_block("Issue Preview");
    let preview = match app.issues().get(app.selected_issue()) {
        Some(issue) => {
            let assignees = if issue.assignees.is_empty() {
                "unassigned"
            } else {
                issue.assignees.as_str()
            };
            let mut lines = Vec::new();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("#{}", issue.number),
                    Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {}", issue.state)),
            ]));
            lines.push(Line::from(format!("assignees: {}", assignees)));
            lines.push(Line::from(format!("comments: {}", issue.comments_count)));
            lines.push(Line::from(""));

            let rendered = markdown::render(issue.body.as_str());
            let preview_lines = rendered.lines.into_iter().take(18).collect::<Vec<Line<'static>>>();
            if preview_lines.is_empty() {
                lines.push(Line::from("No description."));
            } else {
                lines.extend(preview_lines);
            }
            Text::from(lines)
        }
        None => Text::from("Select an issue to preview."),
    };

    let preview_widget = Paragraph::new(preview)
        .block(preview_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(
        preview_widget,
        panes[1].inner(Margin {
            vertical: 1,
            horizontal: 1,
        }),
    );

    draw_status(frame, app, footer);
}

fn draw_issue_detail(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let selected = app.issues().get(app.selected_issue());
    let title = selected
        .map(|issue| format!("#{} {}", issue.number, issue.title))
        .unwrap_or_else(|| "Issue".to_string());
    let block = panel_block(&title);
    let body = selected.map(|issue| issue.body.as_str()).unwrap_or("");
    let assignees = selected
        .map(|issue| {
            if issue.assignees.is_empty() {
                "unassigned".to_string()
            } else {
                issue.assignees.clone()
            }
        })
        .unwrap_or_else(|| "unassigned".to_string());
    let comment_count = selected.map(|issue| issue.comments_count).unwrap_or(0);
    let mut lines = Vec::new();
    lines.push(Line::from(format!(
        "assignees: {} | comments: {}",
        assignees, comment_count
    )));
    lines.push(Line::from(""));
    let rendered_body = markdown::render(body);
    if rendered_body.lines.is_empty() {
        lines.push(Line::from("No description."));
    } else {
        for line in rendered_body.lines {
            lines.push(line);
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Recent comments",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    if app.comments().is_empty() {
        lines.push(Line::from("No comments cached yet."));
    } else {
        let start = app.comments().len().saturating_sub(3);
        for comment in &app.comments()[start..] {
            lines.push(Line::from(vec![
                Span::styled("- ", Style::default().fg(Color::Gray)),
                Span::styled(
                    comment.author.as_str(),
                    Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
                ),
            ]));
            let rendered_comment = markdown::render(comment.body.as_str());
            if rendered_comment.lines.is_empty() {
                lines.push(Line::from(""));
            } else {
                for line in rendered_comment.lines {
                    lines.push(line);
                }
            }
            lines.push(Line::from(""));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.issue_detail_scroll(), 0));
    frame.render_widget(paragraph, main.inner(Margin { vertical: 1, horizontal: 2 }));

    draw_status(frame, app, footer);
}

fn draw_issue_comments(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let block = panel_block("Comments");
    let items = if app.comments().is_empty() {
        vec![ListItem::new("No comments cached yet.")]
    } else {
        app.comments()
            .iter()
            .map(|comment| {
                let mut lines = Vec::new();
                lines.push(Line::from(Span::styled(
                    comment.author.as_str(),
                    Style::default().fg(GITHUB_BLUE).add_modifier(Modifier::BOLD),
                )));
                let rendered = markdown::render(comment.body.as_str());
                if rendered.lines.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    for line in rendered.lines {
                        lines.push(line);
                    }
                }
                lines.push(Line::from(""));
                ListItem::new(lines)
            })
            .collect()
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(Style::default().bg(SELECT_BG));
    frame.render_stateful_widget(
        list,
        main.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_comment()),
    );

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
        .block(block)
        .highlight_symbol("▸ ")
        .highlight_style(Style::default().bg(SELECT_BG));
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
    let block = panel_block("Preset Name");
    let text = app.editor().name();
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, main.inner(Margin { vertical: 1, horizontal: 2 }));

    draw_status(frame, app, footer);
}

fn draw_comment_editor(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let block = panel_block("Comment");
    let text = app.editor().text();
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, main.inner(Margin { vertical: 1, horizontal: 2 }));

    draw_status(frame, app, footer);
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let status = app.status();
    let help = help_text(app);
    let text = if status.is_empty() {
        Text::from(Line::from(help))
    } else {
        Text::from(vec![Line::from(status), Line::from(help)])
    };
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Rgb(139, 148, 158)))
        .block(Block::default().border_style(Style::default().fg(PANEL_BORDER)));
    frame.render_widget(paragraph, area.inner(Margin { vertical: 0, horizontal: 2 }));
}

fn panel_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Line::from(Span::styled(
            title.to_string(),
            Style::default()
                .fg(GITHUB_BLUE)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_BORDER))
}

fn split_area(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);
    (chunks[0], chunks[1])
}

fn help_text(app: &App) -> String {
    match app.view() {
        View::RepoPicker => {
            "Ctrl+R rescan • j/k or ↑/↓ move • gg/G top/bottom • Enter select • q quit"
                .to_string()
        }
        View::RemoteChooser => {
            "j/k or ↑/↓ move • gg/G top/bottom • Enter select • Ctrl+G repos • q quit"
                .to_string()
        }
        View::Issues => {
            "j/k or ↑/↓ move • gg/G top/bottom • Enter open • dd close • r refresh • o browser • Ctrl+G repos • q quit"
                .to_string()
        }
        View::IssueDetail => {
            "c all comments • b/Esc back • r refresh • o browser • Ctrl+G repos • q quit"
                .to_string()
        }
        View::IssueComments => {
            "j/k or ↑/↓ move • gg/G top/bottom • b/Esc back • r refresh • o browser • q quit"
                .to_string()
        }
        View::CommentPresetPicker => {
            "j/k move • gg/G top/bottom • Enter select • Esc cancel • q quit".to_string()
        }
        View::CommentPresetName => {
            "Type name • Enter next • Esc cancel".to_string()
        }
        View::CommentEditor => {
            "Type message • Ctrl+Enter submit • Esc cancel".to_string()
        }
    }
}

fn list_state(selected: usize) -> ListState {
    let mut state = ListState::default();
    state.select(Some(selected));
    state
}
