use super::*;

pub(super) fn draw_status(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: &ThemePalette) {
    frame.render_widget(Clear, area);

    let (mode, mode_color) = mode_meta(app, theme);
    let sync = sync_state_label(app);
    let status = app.status();
    let context = status_context(app);
    let help_raw = primary_help_text(app);
    let sync_label = format!("[{}]", sync);
    let mode_badge = format!("{:^10}", mode);
    let mode_badge_width = mode_badge.chars().count();
    let status_text = if status.is_empty() { "ready" } else { status };

    let mut spans = vec![Span::styled(
        mode_badge,
        Style::default()
            .fg(theme.bg_app)
            .bg(mode_color)
            .add_modifier(Modifier::BOLD),
    )];
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        sync_label,
        Style::default()
            .fg(sync_state_color(sync, theme))
            .add_modifier(Modifier::BOLD),
    ));
    if !status_text.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            status_text,
            Style::default().fg(theme.text_primary),
        ));
    }
    if !context.is_empty() {
        spans.push(Span::styled(" • ", Style::default().fg(theme.border_panel)));
        spans.push(Span::styled(context, Style::default().fg(theme.text_muted)));
    }
    if !help_raw.is_empty() {
        spans.push(Span::styled(" • ", Style::default().fg(theme.border_panel)));
        spans.push(Span::styled(
            help_raw,
            Style::default().fg(theme.text_muted),
        ));
    }

    let status_line = Line::from(spans);
    let paragraph = Paragraph::new(status_line)
        .style(Style::default().bg(theme.bg_panel_alt))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
    app.register_mouse_region(
        MouseTarget::RepoPicker,
        area.x,
        area.y,
        mode_badge_width.saturating_add(1) as u16,
        1,
    );
}

pub(super) fn draw_help_overlay(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    theme: &ThemePalette,
) {
    let popup = centered_rect(84, 72, area);
    frame.render_widget(Clear, popup);
    let shell = popup_block("Keyboard Help", theme);
    let inner = shell.inner(popup).inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(shell, popup);

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "View",
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            primary_help_text(app),
            Style::default().fg(theme.text_primary),
        ),
    ])];
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Common shortcuts",
        Style::default()
            .fg(theme.accent_subtle)
            .add_modifier(Modifier::BOLD),
    )));

    for (key, action) in help_rows(app) {
        lines.push(Line::from(vec![
            key_cap(key, theme),
            Span::raw(" "),
            Span::styled(action, Style::default().fg(theme.text_primary)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Extended shortcuts",
        Style::default()
            .fg(theme.accent_subtle)
            .add_modifier(Modifier::BOLD),
    )));
    for (key, action) in extended_help_rows(app) {
        let mut row = vec![key_cap(key.as_str(), theme)];
        if !action.is_empty() {
            row.push(Span::raw(" "));
            row.push(Span::styled(action, Style::default().fg(theme.text_muted)));
        }
        lines.push(Line::from(row));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press ? or Esc to close",
        Style::default()
            .fg(theme.accent_success)
            .add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(theme.bg_popup)),
        inner,
    );
}

fn key_cap(key: &str, theme: &ThemePalette) -> Span<'static> {
    Span::styled(
        format!(" {} ", key),
        Style::default()
            .fg(theme.bg_app)
            .bg(theme.border_popup)
            .add_modifier(Modifier::BOLD),
    )
}

fn help_rows(app: &App) -> Vec<(&'static str, &'static str)> {
    match app.view() {
        View::RepoPicker => vec![
            ("j / k", "Move repositories"),
            ("/", "Search repositories"),
            ("Enter", "Open selected repository"),
            ("Ctrl+R", "Rescan repositories"),
            ("Ctrl+G", "Open repository picker"),
            ("Ctrl+C", "Quit"),
        ],
        View::Issues => vec![
            ("j / k", "Move issues"),
            ("Enter", "Open selected item"),
            ("Tab", "Switch open/closed"),
            ("1 / 2", "Jump to open/closed tab"),
            ("a", "Cycle assignee filter"),
            ("Ctrl+a", "Reset assignee to all"),
            ("p", "Toggle issues/PR mode"),
            ("Shift+N", "Create issue"),
            ("/", "Search with qualifiers"),
        ],
        View::IssueDetail => vec![
            ("Ctrl+h/l", "Switch panes"),
            ("j / k", "Scroll focused pane"),
            ("Enter", "Open focused pane"),
            ("c", "Open comments"),
            ("Shift+N", "Create issue"),
            ("b or Esc", "Back"),
            ("o", "Open in browser"),
        ],
        View::IssueComments => vec![
            ("j / k", "Jump comments"),
            ("e", "Edit selected comment"),
            ("x", "Delete selected comment"),
            ("m", "Add comment"),
            ("Shift+N", "Create issue"),
            ("b or Esc", "Back"),
            ("o", "Open in browser"),
        ],
        View::PullRequestFiles => {
            if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
                return vec![
                    ("Ctrl+h/l", "Switch files/diff pane"),
                    ("j / k", "Move changed files"),
                    ("Enter", "Open full-width diff pane"),
                    ("w", "Toggle file viewed state"),
                    ("b or Esc", "Back"),
                    ("o", "Open in browser"),
                ];
            }
            if app.pull_request_diff_expanded() {
                return vec![
                    ("Ctrl+h/l", "Switch files/diff pane"),
                    ("j / k", "Move diff lines"),
                    ("Enter", "Return to split files+diff"),
                    ("b or Esc", "Return to split files+diff"),
                    ("c", "Collapse/expand selected hunk"),
                    ("[ / ]", "Pan horizontal diff"),
                    ("0", "Reset horizontal pan"),
                    ("m/e/x", "Add/edit/delete comment"),
                    ("Shift+R", "Resolve/reopen thread"),
                ];
            }
            vec![
                ("Ctrl+h/l", "Switch files/diff pane"),
                ("j / k", "Move diff lines"),
                ("Enter", "Expand diff to full width"),
                ("c", "Collapse/expand selected hunk"),
                ("[ / ]", "Pan horizontal diff"),
                ("0", "Reset horizontal pan"),
                ("m/e/x", "Add/edit/delete comment"),
                ("Shift+R", "Resolve/reopen thread"),
            ]
        }
        View::LinkedPicker => vec![
            ("j / k", "Move linked items"),
            ("Enter", "Open selected linked item"),
            ("b or Esc", "Cancel"),
            ("Ctrl+C", "Quit"),
            ("?", "Toggle help"),
        ],
        View::LabelPicker | View::AssigneePicker => vec![
            ("Type", "Filter options"),
            ("j / k", "Move options"),
            ("Space", "Toggle option"),
            ("Enter", "Apply selection"),
            ("Esc", "Cancel"),
        ],
        View::CommentPresetPicker => vec![
            ("j / k", "Move presets"),
            ("Enter", "Select preset"),
            ("Esc", "Cancel"),
            ("Ctrl+C", "Quit"),
            ("?", "Toggle help"),
        ],
        View::CommentPresetName => vec![
            ("Type", "Preset name"),
            ("Enter", "Continue"),
            ("Esc", "Cancel"),
            ("?", "Toggle help"),
            ("Ctrl+C", "Quit"),
        ],
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::CreateIssue {
                return vec![
                    ("Type", "Edit title/body"),
                    ("Ctrl+j / Ctrl+k", "Jump body/title"),
                    ("Tab / Shift+Tab", "Toggle cancel/create"),
                    ("Enter", "Open/confirm create"),
                    ("Shift+Enter", "Insert newline in body"),
                    ("Esc", "Cancel"),
                ];
            }
            vec![
                ("Type", "Edit body"),
                ("Enter", "Submit"),
                ("Shift+Enter", "Insert newline"),
                ("Esc", "Cancel"),
                ("?", "Toggle help"),
            ]
        }
        View::RemoteChooser => vec![
            ("j / k", "Move remotes"),
            ("Enter", "Select remote"),
            ("Ctrl+G", "Back to repos"),
            ("Ctrl+C", "Quit"),
            ("?", "Toggle help"),
        ],
    }
}

fn extended_help_rows(app: &App) -> Vec<(String, String)> {
    help_text(app)
        .split(" • ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| {
            if let Some((key, action)) = token.split_once(' ') {
                return (key.to_string(), action.to_string());
            }
            (token.to_string(), String::new())
        })
        .collect::<Vec<(String, String)>>()
}

fn mode_meta(app: &App, theme: &ThemePalette) -> (&'static str, Color) {
    let (label, color) = if app.issue_search_mode() || app.repo_search_mode() {
        ("SEARCH", theme.accent_subtle)
    } else if app.scanning() || app.syncing() {
        ("SYNCING", theme.accent_primary)
    } else {
        match app.view() {
            View::RepoPicker => ("REPOS", theme.accent_primary),
            View::RemoteChooser => ("REMOTES", theme.accent_primary),
            View::Issues => {
                if app.work_item_mode() == crate::app::WorkItemMode::PullRequests {
                    ("PRS", theme.accent_success)
                } else {
                    ("ISSUES", theme.accent_success)
                }
            }
            View::IssueDetail => ("DETAIL", theme.accent_primary),
            View::IssueComments => ("COMMENTS", theme.accent_primary),
            View::PullRequestFiles => ("FILES", theme.accent_primary),
            View::LinkedPicker => ("LINKED", theme.accent_primary),
            View::LabelPicker => ("LABELS", theme.accent_subtle),
            View::AssigneePicker => ("ASSIGNEES", theme.accent_subtle),
            View::CommentPresetPicker => ("CLOSE", theme.accent_danger),
            View::CommentPresetName => ("PRESET", theme.accent_subtle),
            View::CommentEditor => ("EDIT", theme.accent_subtle),
        }
    };

    (label, color)
}

pub(super) fn focused_title(title: &str, focused: bool) -> String {
    if focused {
        return format!("> {}", title);
    }
    title.to_string()
}

pub(super) fn focus_border(focused: bool, theme: &ThemePalette) -> Color {
    if focused {
        theme.border_focus
    } else {
        theme.border_panel
    }
}

pub(super) fn draw_modal_background(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    theme: &ThemePalette,
) {
    match app.editor_cancel_view() {
        View::Issues => ui_issues::draw_issues(frame, app, area, theme),
        View::IssueDetail => ui_issue_detail::draw_issue_detail(frame, app, area, theme),
        View::IssueComments => ui_issue_detail::draw_issue_comments(frame, app, area, theme),
        View::PullRequestFiles => ui_pull_request::draw_pull_request_files(frame, app, area, theme),
        _ => {
            frame.render_widget(panel_block("blippy", theme), area);
        }
    }
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg_overlay)),
        area,
    );
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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

fn primary_help_text(app: &App) -> String {
    match app.view() {
        View::RepoPicker => {
            "j/k move • / search • Enter select • Ctrl+R rescan • ? help".to_string()
        }
        View::RemoteChooser => "j/k move • Enter select • b/Esc back • ? help".to_string(),
        View::Issues => {
            if app.issue_search_mode() {
                return "Search mode • Enter keep • Esc clear • ? help".to_string();
            }
            "j/k move • Enter open • Tab open/closed • Shift+N create • a assignee • Ctrl+a all • / search • ? help"
                .to_string()
        }
        View::IssueDetail => {
            if app.focus() == Focus::IssueRecentComments {
                if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
                    return "j/k recent comments • Enter open review • Ctrl+h/l panes • b/Esc back • ? help"
                        .to_string();
                }
                return "j/k recent comments • Enter open comments • Ctrl+h/l panes • b/Esc back • ? help"
                    .to_string();
            }
            "Ctrl+h/l panes • Enter open pane • c comments • Shift+N create • b/Esc back • ? help"
                .to_string()
        }
        View::IssueComments => {
            "j/k comments • e edit • x delete • Shift+N create • b/Esc back • ? help".to_string()
        }
        View::PullRequestFiles => {
            if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
                return "j/k files • Enter full diff • Ctrl+h/l panes • w viewed • b/Esc back • ? help"
                    .to_string();
            }
            let toggle_hint = if app.pull_request_diff_expanded() {
                "b/Esc split diff"
            } else {
                "Enter full diff"
            };
            format!(
                "j/k diff • {} • c collapse hunk • m add • n/p thread • Shift+R resolve • ? help",
                toggle_hint
            )
        }
        View::LinkedPicker => "j/k move • Enter open • Esc cancel • ? help".to_string(),
        View::LabelPicker | View::AssigneePicker => {
            "Type filter • j/k move • Space toggle • Enter apply • Esc cancel • ? help".to_string()
        }
        View::CommentPresetPicker => "j/k move • Enter select • Esc cancel • ? help".to_string(),
        View::CommentPresetName => "Type name • Enter next • Esc cancel • ? help".to_string(),
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::CreateIssue {
                return "Type title/body • Ctrl+j/k jump fields • Tab switch cancel/create • Enter open/confirm • Shift+Enter newline body • Esc cancel • ? help"
                    .to_string();
            }
            "Type text • Enter submit • Shift+Enter newline • Esc cancel • ? help".to_string()
        }
    }
}

fn help_text(app: &App) -> String {
    match app.view() {
        View::RepoPicker => {
            if app.repo_search_mode() {
                return "Search repos: type query • Enter keep • Esc clear • Ctrl+u clear"
                    .to_string();
            }
            "Ctrl+R rescan • j/k move • gg/G top/bottom • / search • Enter select • Ctrl+C quit"
                .to_string()
        }
        View::RemoteChooser => {
            "j/k move • gg/G top/bottom • Enter select • Ctrl+G repos • Ctrl+C quit".to_string()
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
                "Tab open/closed",
                "Shift+N create issue",
                "a assignee",
                "Ctrl+a all assignees",
                "l labels",
                "Shift+A assignees",
                "m comment",
                "r refresh",
                "o browser",
                "Ctrl+C quit",
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
                return "Ctrl+h/l pane • j/k scroll • Enter on description opens comments • Enter on changes opens review • c comments • Shift+N create issue • h/l side in review • m comment • l labels • Shift+A assignees • u reopen • dd close • v checkout • Shift+P linked issue (TUI) • Shift+O linked issue (web) • r refresh • Esc back • Ctrl+C quit"
                    .replace(
                        "Shift+P linked issue (TUI) • Shift+O linked issue (web)",
                        linked_hint,
                    );
            }
            if app.selected_issue_has_known_linked_pr() {
                return "Ctrl+h/l pane • j/k scroll • Enter on right pane opens comments • c comments • Shift+N create issue • m comment • l labels • Shift+A assignees • u reopen • dd close • Shift+P linked PR (TUI) • Shift+O linked PR (web) • r refresh • Esc back • Ctrl+C quit"
                    .to_string();
            }
            "Ctrl+h/l pane • j/k scroll • Enter on right pane opens comments • c comments • Shift+N create issue • m comment • l labels • Shift+A assignees • u reopen • dd close • r refresh • Esc back • Ctrl+C quit"
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
                return "j/k comments • e edit • x delete • Shift+N create issue • m comment • l labels • Shift+A assignees • u reopen • dd close • v checkout • Shift+P linked issue (TUI) • Shift+O linked issue (web) • r refresh • Esc back • Ctrl+C quit"
                    .replace(
                        "Shift+P linked issue (TUI) • Shift+O linked issue (web)",
                        linked_hint,
                    );
            }
            if app.selected_issue_has_known_linked_pr() {
                return "j/k comments • e edit • x delete • Shift+N create issue • m comment • l labels • Shift+A assignees • u reopen • dd close • Shift+P linked PR (TUI) • Shift+O linked PR (web) • r refresh • Esc back • Ctrl+C quit"
                    .to_string();
            }
            "j/k comments • e edit • x delete • Shift+N create issue • m comment • l labels • Shift+A assignees • u reopen • dd close • r refresh • Esc back • Ctrl+C quit"
                .to_string()
        }
        View::PullRequestFiles => {
            if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
                return "Ctrl+h/l pane • j/k move file • Enter full diff • w viewed • r refresh • v checkout • Esc/back"
                    .to_string();
            }
            let toggle_hint = if app.pull_request_diff_expanded() {
                "Enter/b/Esc split diff"
            } else {
                "Enter full diff"
            };
            format!(
                "Ctrl+h/l pane • j/k move line • {} • c collapse hunk • [/ ] pan diff • 0 reset pan • h/l old/new side • Shift+V visual range • m add • e edit • x delete • Shift+R resolve/reopen • n/p cycle line comments • r refresh • v checkout • Ctrl+C quit",
                toggle_hint
            )
        }
        View::LinkedPicker => {
            "j/k move • Enter open linked item • b/Esc cancel • Ctrl+C quit".to_string()
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
            "j/k move • gg/G top/bottom • Enter select • Esc cancel • Ctrl+C quit".to_string()
        }
        View::CommentPresetName => "Type name • Enter next • Esc cancel".to_string(),
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::CreateIssue {
                return "Type title/body • Ctrl+j/k jump fields • Tab switch cancel/create • Enter open/confirm • Shift+Enter newline (body) • Esc cancel"
                    .to_string();
            }
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
