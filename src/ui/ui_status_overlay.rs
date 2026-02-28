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
            key_cap(key.as_str(), theme),
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
        format!("Press ? or {} to close", bind(app, "back_escape")),
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

fn bind(app: &App, action: &str) -> String {
    app.keybind_label(action)
}

fn bind_any(app: &App, actions: &[&str], separator: &str) -> String {
    let mut labels = Vec::new();
    for action in actions {
        let label = bind(app, action);
        if label.is_empty() {
            continue;
        }
        if labels
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(label.as_str()))
        {
            continue;
        }
        labels.push(label);
    }
    labels.join(separator)
}

fn help_toggle_available(app: &App) -> bool {
    if matches!(app.view(), View::CommentPresetName | View::CommentEditor) {
        return false;
    }
    if app.view() == View::RepoPicker && app.repo_search_mode() {
        return false;
    }
    if app.view() == View::Issues && app.issue_search_mode() {
        return false;
    }
    if matches!(app.view(), View::LabelPicker | View::AssigneePicker) {
        return false;
    }
    true
}

fn with_help_hint(app: &App, text: String) -> String {
    if !help_toggle_available(app) {
        return text;
    }
    format!("{} • ? help", text)
}

fn help_rows(app: &App) -> Vec<(String, String)> {
    let move_keys = bind_any(app, &["move_down", "move_up"], " / ");
    let back_keys = bind_any(app, &["back", "back_escape"], " / ");
    let pane_keys = bind_any(app, &["focus_left", "focus_right"], "/");
    let comment_keys = bind_any(app, &["add_comment", "edit_comment", "delete_comment"], "/");
    let diff_pan_keys = bind_any(app, &["diff_scroll_left", "diff_scroll_right"], " / ");

    match app.view() {
        View::RepoPicker => vec![
            (move_keys, "Move repositories".to_string()),
            (bind(app, "repo_search"), "Search repositories".to_string()),
            (bind(app, "submit"), "Open selected repository".to_string()),
            (bind(app, "rescan_repos"), "Rescan repositories".to_string()),
            (
                bind(app, "clear_and_repo_picker"),
                "Open repository picker".to_string(),
            ),
            (bind(app, "quit"), "Quit".to_string()),
        ],
        View::Issues => vec![
            (move_keys, "Move issues".to_string()),
            (bind(app, "submit"), "Open selected item".to_string()),
            (
                bind(app, "cycle_issue_filter"),
                "Switch open/closed".to_string(),
            ),
            (
                format!(
                    "{} / {}",
                    bind(app, "issue_filter_open"),
                    bind(app, "issue_filter_closed")
                ),
                "Jump to open/closed tab".to_string(),
            ),
            (
                bind(app, "cycle_assignee_filter"),
                "Cycle assignee filter".to_string(),
            ),
            ("Ctrl+a".to_string(), "Reset assignee to all".to_string()),
            (
                bind(app, "toggle_work_item_mode"),
                "Toggle issues/PR mode".to_string(),
            ),
            (bind(app, "create_issue"), "Create issue".to_string()),
            (
                bind(app, "issue_search"),
                "Search with qualifiers".to_string(),
            ),
        ],
        View::IssueDetail => vec![
            (pane_keys, "Switch panes".to_string()),
            (move_keys, "Scroll focused pane".to_string()),
            (bind(app, "submit"), "Open focused pane".to_string()),
            (bind(app, "open_comments"), "Open comments".to_string()),
            (bind(app, "create_issue"), "Create issue".to_string()),
            (back_keys, "Back".to_string()),
            (bind(app, "open_browser"), "Open in browser".to_string()),
        ],
        View::IssueComments => vec![
            (move_keys, "Jump comments".to_string()),
            (
                bind(app, "edit_comment"),
                "Edit selected comment".to_string(),
            ),
            (
                bind(app, "delete_comment"),
                "Delete selected comment".to_string(),
            ),
            (bind(app, "add_comment"), "Add comment".to_string()),
            (bind(app, "create_issue"), "Create issue".to_string()),
            (back_keys, "Back".to_string()),
            (bind(app, "open_browser"), "Open in browser".to_string()),
        ],
        View::PullRequestFiles => {
            if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
                return vec![
                    (pane_keys, "Switch files/diff pane".to_string()),
                    (move_keys, "Move changed files".to_string()),
                    (bind(app, "submit"), "Open full-width diff pane".to_string()),
                    (
                        bind(app, "toggle_file_viewed"),
                        "Toggle file viewed state".to_string(),
                    ),
                    (back_keys, "Back".to_string()),
                    (bind(app, "open_browser"), "Open in browser".to_string()),
                ];
            }
            if app.pull_request_diff_expanded() {
                return vec![
                    (pane_keys, "Switch files/diff pane".to_string()),
                    (move_keys, "Move diff lines".to_string()),
                    (
                        bind_any(app, &["submit", "back", "back_escape"], " / "),
                        "Return to split files+diff".to_string(),
                    ),
                    (
                        bind(app, "collapse_hunk"),
                        "Collapse/expand selected hunk".to_string(),
                    ),
                    (diff_pan_keys, "Pan horizontal diff".to_string()),
                    (
                        bind(app, "diff_scroll_reset"),
                        "Reset horizontal pan".to_string(),
                    ),
                    (comment_keys, "Add/edit/delete comment".to_string()),
                    (
                        bind(app, "resolve_thread"),
                        "Resolve/reopen thread".to_string(),
                    ),
                ];
            }
            vec![
                (pane_keys, "Switch files/diff pane".to_string()),
                (move_keys, "Move diff lines".to_string()),
                (bind(app, "submit"), "Expand diff to full width".to_string()),
                (
                    bind(app, "collapse_hunk"),
                    "Collapse/expand selected hunk".to_string(),
                ),
                (diff_pan_keys, "Pan horizontal diff".to_string()),
                (
                    bind(app, "diff_scroll_reset"),
                    "Reset horizontal pan".to_string(),
                ),
                (comment_keys, "Add/edit/delete comment".to_string()),
                (
                    bind(app, "resolve_thread"),
                    "Resolve/reopen thread".to_string(),
                ),
            ]
        }
        View::LinkedPicker => vec![
            (move_keys, "Move linked items".to_string()),
            (bind(app, "submit"), "Open selected linked item".to_string()),
            (back_keys, "Cancel".to_string()),
            (bind(app, "quit"), "Quit".to_string()),
            ("?".to_string(), "Toggle help".to_string()),
        ],
        View::LabelPicker | View::AssigneePicker => vec![
            ("Type".to_string(), "Filter options".to_string()),
            (move_keys, "Move options".to_string()),
            (bind(app, "popup_toggle"), "Toggle option".to_string()),
            (bind(app, "submit"), "Apply selection".to_string()),
            (bind(app, "back_escape"), "Cancel".to_string()),
        ],
        View::CommentPresetPicker => vec![
            (move_keys, "Move presets".to_string()),
            (bind(app, "submit"), "Select preset".to_string()),
            (bind(app, "back_escape"), "Cancel".to_string()),
            (bind(app, "quit"), "Quit".to_string()),
            ("?".to_string(), "Toggle help".to_string()),
        ],
        View::CommentPresetName => vec![
            ("Type".to_string(), "Preset name".to_string()),
            (bind(app, "submit"), "Continue".to_string()),
            (bind(app, "back_escape"), "Cancel".to_string()),
            (bind(app, "quit"), "Quit".to_string()),
        ],
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::CreateIssue {
                return vec![
                    ("Type".to_string(), "Edit title/body".to_string()),
                    ("Ctrl+j / Ctrl+k".to_string(), "Jump body/title".to_string()),
                    (
                        "Tab / Shift+Tab".to_string(),
                        "Toggle cancel/create".to_string(),
                    ),
                    (bind(app, "submit"), "Open/confirm create".to_string()),
                    (
                        "Shift+Enter".to_string(),
                        "Insert newline in body".to_string(),
                    ),
                    (bind(app, "back_escape"), "Cancel".to_string()),
                ];
            }
            vec![
                ("Type".to_string(), "Edit body".to_string()),
                (bind(app, "submit"), "Submit".to_string()),
                ("Shift+Enter".to_string(), "Insert newline".to_string()),
                (bind(app, "back_escape"), "Cancel".to_string()),
            ]
        }
        View::RemoteChooser => vec![
            (move_keys, "Move remotes".to_string()),
            (bind(app, "submit"), "Select remote".to_string()),
            (
                bind(app, "clear_and_repo_picker"),
                "Back to repos".to_string(),
            ),
            (bind(app, "quit"), "Quit".to_string()),
            ("?".to_string(), "Toggle help".to_string()),
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
    let move_keys = bind_any(app, &["move_down", "move_up"], "/");
    let submit = bind(app, "submit");
    let back_keys = bind_any(app, &["back", "back_escape"], "/");
    let pane_keys = bind_any(app, &["focus_left", "focus_right"], "/");

    match app.view() {
        View::RepoPicker => {
            if app.repo_search_mode() {
                return with_help_hint(
                    app,
                    format!(
                        "Search mode • {} keep • {} clear",
                        submit,
                        bind(app, "back_escape")
                    ),
                );
            }
            with_help_hint(
                app,
                format!(
                    "{} move • {} search • {} select • {} rescan",
                    move_keys,
                    bind(app, "repo_search"),
                    submit,
                    bind(app, "rescan_repos")
                ),
            )
        }
        View::RemoteChooser => with_help_hint(
            app,
            format!(
                "{} move • {} select • {} repos",
                move_keys,
                submit,
                bind(app, "clear_and_repo_picker")
            ),
        ),
        View::Issues => {
            if app.issue_search_mode() {
                return with_help_hint(
                    app,
                    format!(
                        "Search mode • {} keep • {} clear",
                        submit,
                        bind(app, "back_escape")
                    ),
                );
            }
            with_help_hint(
                app,
                format!(
                    "{} move • {} open • {} open/closed • {} create • {} assignee • {} all • {} search",
                    move_keys,
                    submit,
                    bind(app, "cycle_issue_filter"),
                    bind(app, "create_issue"),
                    bind(app, "cycle_assignee_filter"),
                    "Ctrl+a",
                    bind(app, "issue_search")
                ),
            )
        }
        View::IssueDetail => {
            if app.focus() == Focus::IssueRecentComments {
                if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
                    return with_help_hint(
                        app,
                        format!(
                            "{} recent comments • {} open review • {} panes • {} back",
                            move_keys, submit, pane_keys, back_keys
                        ),
                    );
                }
                return with_help_hint(
                    app,
                    format!(
                        "{} recent comments • {} open comments • {} panes • {} back",
                        move_keys, submit, pane_keys, back_keys
                    ),
                );
            }
            with_help_hint(
                app,
                format!(
                    "{} panes • {} open pane • {} comments • {} create • {} back",
                    pane_keys,
                    submit,
                    bind(app, "open_comments"),
                    bind(app, "create_issue"),
                    back_keys
                ),
            )
        }
        View::IssueComments => with_help_hint(
            app,
            format!(
                "{} comments • {} edit • {} delete • {} create • {} back",
                move_keys,
                bind(app, "edit_comment"),
                bind(app, "delete_comment"),
                bind(app, "create_issue"),
                back_keys
            ),
        ),
        View::PullRequestFiles => {
            if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
                return with_help_hint(
                    app,
                    format!(
                        "{} files • {} full diff • {} panes • {} viewed • {} back",
                        move_keys,
                        submit,
                        pane_keys,
                        bind(app, "toggle_file_viewed"),
                        back_keys
                    ),
                );
            }
            let toggle_hint = if app.pull_request_diff_expanded() {
                format!(
                    "{} split diff",
                    bind_any(app, &["submit", "back", "back_escape"], "/")
                )
            } else {
                format!("{} full diff", submit)
            };
            with_help_hint(
                app,
                format!(
                    "{} diff • {} • {} collapse hunk • {} add • {} thread • {} resolve",
                    move_keys,
                    toggle_hint,
                    bind(app, "collapse_hunk"),
                    bind(app, "add_comment"),
                    bind_any(app, &["next_line_comment", "prev_line_comment"], "/"),
                    bind(app, "resolve_thread")
                ),
            )
        }
        View::LinkedPicker => with_help_hint(
            app,
            format!(
                "{} move • {} open • {} cancel",
                move_keys,
                submit,
                bind(app, "back_escape")
            ),
        ),
        View::LabelPicker | View::AssigneePicker => format!(
            "Type filter • {} move • {} toggle • {} apply • {} cancel",
            move_keys,
            bind(app, "popup_toggle"),
            submit,
            bind(app, "back_escape")
        ),
        View::CommentPresetPicker => with_help_hint(
            app,
            format!(
                "{} move • {} select • {} cancel",
                move_keys,
                submit,
                bind(app, "back_escape")
            ),
        ),
        View::CommentPresetName => format!(
            "Type name • {} next • {} cancel",
            submit,
            bind(app, "back_escape")
        ),
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::CreateIssue {
                return format!(
                    "Type title/body • Ctrl+j/k jump fields • Tab switch cancel/create • {} open/confirm • Shift+Enter newline body • {} cancel",
                    submit,
                    bind(app, "back_escape")
                );
            }
            format!(
                "Type text • {} submit • Shift+Enter newline • {} cancel",
                submit,
                bind(app, "back_escape")
            )
        }
    }
}

fn help_text(app: &App) -> String {
    let move_keys = bind_any(app, &["move_down", "move_up"], "/");
    let pane_keys = bind_any(app, &["focus_left", "focus_right"], "/");
    let submit = bind(app, "submit");
    let back_keys = bind_any(app, &["back", "back_escape"], "/");

    match app.view() {
        View::RepoPicker => {
            if app.repo_search_mode() {
                return format!(
                    "Search repos: type query • {} keep • {} clear • Ctrl+u clear",
                    submit,
                    bind(app, "back_escape")
                );
            }
            format!(
                "{} rescan • {} move • gg/G top/bottom • {} search • {} select • {} quit",
                bind(app, "rescan_repos"),
                move_keys,
                bind(app, "repo_search"),
                submit,
                bind(app, "quit")
            )
        }
        View::RemoteChooser => {
            format!(
                "{} move • gg/G top/bottom • {} select • {} repos • {} quit",
                move_keys,
                submit,
                bind(app, "clear_and_repo_picker"),
                bind(app, "quit")
            )
        }
        View::Issues => {
            if app.issue_search_mode() {
                return format!(
                    "Search: type terms/qualifiers (is:, label:, assignee:, #num) • {} keep • {} clear • Ctrl+u clear",
                    submit,
                    bind(app, "back_escape")
                );
            }
            let selected_is_pr = app.selected_issue_row().is_some_and(|issue| issue.is_pr);
            let reviewing_pr =
                selected_is_pr || app.work_item_mode() == crate::app::WorkItemMode::PullRequests;
            let mut parts = vec![
                format!("{} move", move_keys),
                format!("{} open", submit),
                format!("{} search", bind(app, "issue_search")),
                format!("{} issues/prs", bind(app, "toggle_work_item_mode")),
                format!(
                    "{}/{} tabs",
                    bind(app, "issue_filter_open"),
                    bind(app, "issue_filter_closed")
                ),
                format!("{} open/closed", bind(app, "cycle_issue_filter")),
                format!("{} create issue", bind(app, "create_issue")),
                format!("{} assignee", bind(app, "cycle_assignee_filter")),
                "Ctrl+a all assignees".to_string(),
                format!("{} labels", bind(app, "edit_labels")),
                format!("{} assignees", bind(app, "edit_assignees")),
                format!("{} comment", bind(app, "add_comment")),
                format!("{} refresh", bind(app, "refresh")),
                format!("{} browser", bind(app, "open_browser")),
                format!("{} quit", bind(app, "quit")),
            ];
            if reviewing_pr {
                parts.insert(10, format!("{} reopen", bind(app, "reopen_issue")));
                parts.insert(11, "dd close".to_string());
                parts.insert(12, format!("{} checkout", bind(app, "checkout_pr")));
                parts.insert(
                    13,
                    format!("{} linked issue (TUI)", bind(app, "open_linked_pr_tui")),
                );
                parts.insert(
                    14,
                    format!("{} linked issue (web)", bind(app, "open_linked_pr_browser")),
                );
            } else {
                parts.insert(10, format!("{} reopen", bind(app, "reopen_issue")));
                parts.insert(11, "dd close".to_string());
                if app.selected_issue_has_known_linked_pr() {
                    parts.insert(
                        12,
                        format!("{} linked PR (TUI)", bind(app, "open_linked_pr_tui")),
                    );
                    parts.insert(
                        13,
                        format!("{} linked PR (web)", bind(app, "open_linked_pr_browser")),
                    );
                }
            }
            parts.join(" • ")
        }
        View::IssueDetail => {
            let is_pr = app.current_issue_row().is_some_and(|issue| issue.is_pr);
            if is_pr {
                let linked_hint = if app.selected_pull_request_has_known_linked_issue() {
                    format!(
                        "{} linked issue (TUI) • {} linked issue (web)",
                        bind(app, "open_linked_pr_tui"),
                        bind(app, "open_linked_pr_browser")
                    )
                } else {
                    format!(
                        "{} find/open linked issue • {} open linked issue (web)",
                        bind(app, "open_linked_pr_tui"),
                        bind(app, "open_linked_pr_browser")
                    )
                };
                return format!(
                    "{} pane • {} scroll • {} on description opens comments • {} on changes opens review • {} comments • {} create issue • {}/{} side in review • {} comment • {} labels • {} assignees • {} reopen • dd close • {} checkout • {} • {} refresh • {} back • {} quit",
                    pane_keys,
                    move_keys,
                    submit,
                    submit,
                    bind(app, "open_comments"),
                    bind(app, "create_issue"),
                    bind(app, "review_side_left"),
                    bind(app, "review_side_right"),
                    bind(app, "add_comment"),
                    bind(app, "edit_labels"),
                    bind(app, "edit_assignees"),
                    bind(app, "reopen_issue"),
                    bind(app, "checkout_pr"),
                    linked_hint,
                    bind(app, "refresh"),
                    bind(app, "back_escape"),
                    bind(app, "quit")
                );
            }
            if app.selected_issue_has_known_linked_pr() {
                return format!(
                    "{} pane • {} scroll • {} on right pane opens comments • {} comments • {} create issue • {} comment • {} labels • {} assignees • {} reopen • dd close • {} linked PR (TUI) • {} linked PR (web) • {} refresh • {} back • {} quit",
                    pane_keys,
                    move_keys,
                    submit,
                    bind(app, "open_comments"),
                    bind(app, "create_issue"),
                    bind(app, "add_comment"),
                    bind(app, "edit_labels"),
                    bind(app, "edit_assignees"),
                    bind(app, "reopen_issue"),
                    bind(app, "open_linked_pr_tui"),
                    bind(app, "open_linked_pr_browser"),
                    bind(app, "refresh"),
                    bind(app, "back_escape"),
                    bind(app, "quit")
                );
            }
            format!(
                "{} pane • {} scroll • {} on right pane opens comments • {} comments • {} create issue • {} comment • {} labels • {} assignees • {} reopen • dd close • {} refresh • {} back • {} quit",
                pane_keys,
                move_keys,
                submit,
                bind(app, "open_comments"),
                bind(app, "create_issue"),
                bind(app, "add_comment"),
                bind(app, "edit_labels"),
                bind(app, "edit_assignees"),
                bind(app, "reopen_issue"),
                bind(app, "refresh"),
                bind(app, "back_escape"),
                bind(app, "quit")
            )
        }
        View::IssueComments => {
            let is_pr = app.current_issue_row().is_some_and(|issue| issue.is_pr);
            if is_pr {
                let linked_hint = if app.selected_pull_request_has_known_linked_issue() {
                    format!(
                        "{} linked issue (TUI) • {} linked issue (web)",
                        bind(app, "open_linked_pr_tui"),
                        bind(app, "open_linked_pr_browser")
                    )
                } else {
                    format!(
                        "{} find/open linked issue • {} open linked issue (web)",
                        bind(app, "open_linked_pr_tui"),
                        bind(app, "open_linked_pr_browser")
                    )
                };
                return format!(
                    "{} comments • {} edit • {} delete • {} create issue • {} comment • {} labels • {} assignees • {} reopen • dd close • {} checkout • {} • {} refresh • {} back • {} quit",
                    move_keys,
                    bind(app, "edit_comment"),
                    bind(app, "delete_comment"),
                    bind(app, "create_issue"),
                    bind(app, "add_comment"),
                    bind(app, "edit_labels"),
                    bind(app, "edit_assignees"),
                    bind(app, "reopen_issue"),
                    bind(app, "checkout_pr"),
                    linked_hint,
                    bind(app, "refresh"),
                    bind(app, "back_escape"),
                    bind(app, "quit")
                );
            }
            if app.selected_issue_has_known_linked_pr() {
                return format!(
                    "{} comments • {} edit • {} delete • {} create issue • {} comment • {} labels • {} assignees • {} reopen • dd close • {} linked PR (TUI) • {} linked PR (web) • {} refresh • {} back • {} quit",
                    move_keys,
                    bind(app, "edit_comment"),
                    bind(app, "delete_comment"),
                    bind(app, "create_issue"),
                    bind(app, "add_comment"),
                    bind(app, "edit_labels"),
                    bind(app, "edit_assignees"),
                    bind(app, "reopen_issue"),
                    bind(app, "open_linked_pr_tui"),
                    bind(app, "open_linked_pr_browser"),
                    bind(app, "refresh"),
                    bind(app, "back_escape"),
                    bind(app, "quit")
                );
            }
            format!(
                "{} comments • {} edit • {} delete • {} create issue • {} comment • {} labels • {} assignees • {} reopen • dd close • {} refresh • {} back • {} quit",
                move_keys,
                bind(app, "edit_comment"),
                bind(app, "delete_comment"),
                bind(app, "create_issue"),
                bind(app, "add_comment"),
                bind(app, "edit_labels"),
                bind(app, "edit_assignees"),
                bind(app, "reopen_issue"),
                bind(app, "refresh"),
                bind(app, "back_escape"),
                bind(app, "quit")
            )
        }
        View::PullRequestFiles => {
            if app.pull_request_review_focus() == PullRequestReviewFocus::Files {
                return format!(
                    "{} pane • {} move file • {} full diff • {} viewed • {} refresh • {} checkout • {}",
                    pane_keys,
                    move_keys,
                    submit,
                    bind(app, "toggle_file_viewed"),
                    bind(app, "refresh"),
                    bind(app, "checkout_pr"),
                    back_keys
                );
            }
            let toggle_hint = if app.pull_request_diff_expanded() {
                format!(
                    "{} split diff",
                    bind_any(app, &["submit", "back", "back_escape"], "/")
                )
            } else {
                format!("{} full diff", submit)
            };
            format!(
                "{} pane • {} move line • {} • {} collapse hunk • {}/{} pan diff • {} reset pan • {}/{} old/new side • {} visual range • {} add • {} edit • {} delete • {} resolve/reopen • {}/{} cycle line comments • {} refresh • {} checkout • {} quit",
                pane_keys,
                move_keys,
                toggle_hint,
                bind(app, "collapse_hunk"),
                bind(app, "diff_scroll_left"),
                bind(app, "diff_scroll_right"),
                bind(app, "diff_scroll_reset"),
                bind(app, "review_side_left"),
                bind(app, "review_side_right"),
                bind(app, "visual_mode"),
                bind(app, "add_comment"),
                bind(app, "edit_comment"),
                bind(app, "delete_comment"),
                bind(app, "resolve_thread"),
                bind(app, "next_line_comment"),
                bind(app, "prev_line_comment"),
                bind(app, "refresh"),
                bind(app, "checkout_pr"),
                bind(app, "quit")
            )
        }
        View::LinkedPicker => {
            format!(
                "{} move • {} open linked item • {} cancel • {} quit",
                move_keys,
                submit,
                back_keys,
                bind(app, "quit")
            )
        }
        View::LabelPicker => {
            format!(
                "Type to filter • {} move • {} toggle • {} apply • Ctrl+u clear • {} cancel",
                move_keys,
                bind(app, "popup_toggle"),
                submit,
                bind(app, "back_escape")
            )
        }
        View::AssigneePicker => {
            format!(
                "Type to filter • {} move • {} toggle • {} apply • Ctrl+u clear • {} cancel",
                move_keys,
                bind(app, "popup_toggle"),
                submit,
                bind(app, "back_escape")
            )
        }
        View::CommentPresetPicker => {
            format!(
                "{} move • gg/G top/bottom • {} select • {} cancel • {} quit",
                move_keys,
                submit,
                bind(app, "back_escape"),
                bind(app, "quit")
            )
        }
        View::CommentPresetName => format!(
            "Type name • {} next • {} cancel",
            submit,
            bind(app, "back_escape")
        ),
        View::CommentEditor => {
            if app.editor_mode() == EditorMode::CreateIssue {
                return format!(
                    "Type title/body • Ctrl+j/k jump fields • Tab switch cancel/create • {} open/confirm • Shift+Enter newline (body) • {} cancel",
                    submit,
                    bind(app, "back_escape")
                );
            }
            if app.editor_mode() == EditorMode::AddPreset {
                return format!(
                    "Type preset body • {} save • Shift+Enter newline (Ctrl+j fallback) • {} cancel",
                    submit,
                    bind(app, "back_escape")
                );
            }
            format!(
                "Type message • {} submit • Shift+Enter newline (Ctrl+j fallback) • {} cancel",
                submit,
                bind(app, "back_escape")
            )
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
