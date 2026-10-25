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

    let diff_area = if diff_expanded { panes[0] } else { panes[1] };
    let selected_file = app
        .selected_pull_request_file_row()
        .map(|file| (file.filename.clone(), file.patch.clone()));
    let rows = selected_file
        .as_ref()
        .filter(|_| !app.pull_request_files_syncing())
        .map(|(_, patch)| parse_patch(patch.as_deref()))
        .unwrap_or_default();
    let panel_width = diff_area.width.saturating_sub(2) as usize;
    let cells_width = panel_width.saturating_sub(2);
    let left_width = cells_width.saturating_sub(5) / 2;
    let right_width = cells_width.saturating_sub(left_width + 3);
    let horizontal_max = split_diff_horizontal_limit(&rows, left_width, right_width);
    app.set_pull_request_diff_horizontal_max(horizontal_max.min(u16::MAX as usize) as u16);

    let title = match app.current_issue_row() {
        Some(issue) => format!("PR review #{}", issue.number),
        None => "PR review".to_string(),
    };
    let focused = match app.pull_request_review_focus() {
        PullRequestReviewFocus::Files => "files",
        PullRequestReviewFocus::Diff => "diff",
    };
    let horizontal_scroll = app.pull_request_diff_horizontal_scroll();
    let back_label = "[Back]";
    let files_label = "[Files]";
    let diff_label = "[Diff]";
    let header = Text::from(vec![
        Line::from(Span::styled(
            title.clone(),
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                back_label,
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                files_label,
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
                diff_label,
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
    if header_content.height > 1 {
        let mut x = header_content.x;
        for (label, target) in [
            (back_label, MouseTarget::Back),
            (files_label, MouseTarget::PullRequestFocusFiles),
            (diff_label, MouseTarget::PullRequestFocusDiff),
        ] {
            let width = Span::raw(label).width() as u16;
            app.register_mouse_region(
                target,
                x,
                header_content.y + 1,
                width.min(header_content.right().saturating_sub(x)),
                1,
            );
            x = x.saturating_add(width + 2);
        }
    }

    let files_focused = app.pull_request_review_focus() == PullRequestReviewFocus::Files;
    if !diff_expanded {
        let files_inner = panes[0].inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let max_file_rows = files_inner.height as usize;
        let selected = selected_for_list(
            app.selected_pull_request_file(),
            app.pull_request_files().len(),
        );
        let start = list_window_start(selected, app.pull_request_files().len(), max_file_rows);
        let file_items = if app.pull_request_files().is_empty() {
            vec![ListItem::new(
                "No changed files cached yet. Press r to refresh.",
            )]
        } else {
            let view_state_loaded = app.pull_request_view_state_loaded();
            app.pull_request_files()
                .iter()
                .skip(start)
                .take(max_file_rows)
                .map(|file| {
                    let comment_count =
                        app.pull_request_comments_count_for_path(file.filename.as_str());
                    let viewed = app.pull_request_file_is_viewed(file.filename.as_str());
                    let viewed_marker = match (view_state_loaded, viewed) {
                        (false, _) => "?",
                        (true, true) => "✓",
                        (true, false) => "·",
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            viewed_marker,
                            if view_state_loaded && viewed {
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
        let mut files_state = list_state(selected - start);
        frame.render_stateful_widget(files_list, panes[0], &mut files_state);
        register_mouse_region(app, MouseTarget::PullRequestFilesPane, panes[0]);
        for (row, index) in (start + files_state.offset()..app.pull_request_files().len())
            .take(max_file_rows)
            .enumerate()
        {
            let y = files_inner.y.saturating_add(row as u16);
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
    let mut lines = Vec::new();
    let mut row_offsets = Vec::new();
    let mut selected_comment_offset = None;

    if app.pull_request_files_syncing() {
        lines.push(Line::from("Loading pull request changes"));
    } else if selected_file.is_none() {
        lines.push(Line::from("Select a file to start reviewing."));
    } else {
        let (file_name, _) = selected_file.as_ref().expect("selected file exists");
        if rows.is_empty() {
            lines.push(Line::from(Span::styled(
                "No textual patch available for this file.",
                Style::default().fg(theme.text_muted),
            )));
        } else {
            row_offsets = vec![None; rows.len()];
            let horizontal_offset = app.pull_request_diff_horizontal_scroll() as usize;
            let visual_range = app.pull_request_visual_range();
            for (index, row) in app.visible_pull_request_diff_rows(file_name.as_str(), &rows) {
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
                    if ctx.selected {
                        selected_comment_offset = Some(lines.len());
                    }
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
                    if ctx.selected {
                        selected_comment_offset = Some(lines.len());
                    }
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
    let wrap = content_width == 0
        || lines
            .iter()
            .any(|line| line.width() > content_width as usize);
    let total_lines = if wrap {
        wrapped_line_count(&lines, content_width)
    } else {
        lines.len()
    };
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;
    app.set_pull_request_diff_max_scroll(max_scroll);

    let selected_offset = selected_comment_offset
        .map(|offset| {
            if wrap {
                wrapped_line_count(&lines[..offset], content_width) as u16
            } else {
                offset as u16
            }
        })
        .or_else(|| {
            row_offsets
                .get(app.selected_pull_request_diff_line())
                .and_then(|offset| *offset)
        })
        .unwrap_or(0);
    let mut scroll = app.pull_request_diff_scroll();
    if selected_offset < scroll {
        scroll = selected_offset;
    }
    let viewport = viewport_height as u16;
    if viewport > 0 && selected_offset >= scroll.saturating_add(viewport) {
        scroll = selected_offset.saturating_sub(viewport.saturating_sub(1));
    }
    let last_visible_index = row_offsets
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, offset)| offset.map(|_| index));
    if selected_comment_offset.is_none()
        && last_visible_index.is_some_and(|index| app.selected_pull_request_diff_line() >= index)
    {
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
    let mut paragraph = Paragraph::new(Text::from(lines))
        .block(panel_block_with_border(
            diff_block_title.as_str(),
            ui_status_overlay::focus_border(diff_focused, theme),
            theme,
        ))
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_panel))
        .scroll((scroll, 0));
    if wrap {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{PullRequestFile, PullRequestReviewComment},
        config::Config,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    fn inline_comment_app(line: usize, side: ReviewSide) -> App {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        let patch = std::iter::once("@@ -1,20 +1,20 @@".to_string())
            .chain((1..=20).map(|line| format!(" row{line:02}")))
            .collect::<Vec<_>>()
            .join("\n");
        app.set_pull_request_files(
            1,
            "head".to_string(),
            vec![PullRequestFile {
                filename: "example.rs".to_string(),
                status: "modified".to_string(),
                additions: 0,
                deletions: 0,
                patch: Some(patch),
            }],
        );
        app.set_pull_request_review_comments(
            (1..=12)
                .map(|id| PullRequestReviewComment {
                    id,
                    thread_id: None,
                    resolved: false,
                    anchored: true,
                    path: "example.rs".to_string(),
                    line: line as i64,
                    side,
                    body: format!("c{id:02}"),
                    author: "r".to_string(),
                    created_at: None,
                })
                .collect(),
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        if side == ReviewSide::Left {
            app.on_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        }
        for _ in 0..line {
            app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        }
        app
    }

    fn review_screen(terminal: &mut Terminal<TestBackend>, app: &mut App) -> String {
        terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        buffer
            .content
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn cycling_inline_comments_keeps_selected_comment_visible() {
        for (width, height) in [(80, 16), (120, 24)] {
            for expanded in [false, true] {
                for side in [ReviewSide::Right, ReviewSide::Left] {
                    for line in [3, 10, 20] {
                        let mut app = inline_comment_app(line, side);
                        if expanded {
                            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                        }
                        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                        let initial = review_screen(&mut terminal, &mut app);
                        for (key, count, id) in [
                            ('n', 5, 6),
                            ('n', 6, 12),
                            ('n', 1, 1),
                            ('p', 1, 12),
                            ('p', 6, 6),
                            ('p', 5, 1),
                        ] {
                            for _ in 0..count {
                                app.on_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
                            }
                            let screen = review_screen(&mut terminal, &mut app);
                            assert_eq!(app.selected_pull_request_review_comment_id(), Some(id));
                            assert_eq!(app.selected_pull_request_diff_line(), line);
                            assert_eq!(app.pull_request_review_side(), side);
                            assert_eq!(
                                app.pull_request_review_focus(),
                                PullRequestReviewFocus::Diff
                            );
                            assert_eq!(app.pull_request_diff_expanded(), expanded);
                            let side_label = if side == ReviewSide::Right {
                                "new"
                            } else {
                                "old"
                            };
                            assert!(
                                screen.contains(&format!("> [{side_label} open @r] c{id:02}")),
                                "{width}x{height}, expanded={expanded}, side={side:?}, line={line}, selected={id}\nInitial:\n{initial}\nAfter {count} {key} keys:\n{screen}"
                            );
                            assert_eq!(screen, review_screen(&mut terminal, &mut app));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn diff_row_navigation_and_clicks_work_after_cycling_inline_comments() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

        let mut app = inline_comment_app(3, ReviewSide::Right);
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        review_screen(&mut terminal, &mut app);
        for _ in 0..5 {
            app.on_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        }
        review_screen(&mut terminal, &mut app);
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        let screen = review_screen(&mut terminal, &mut app);
        assert_eq!(app.selected_pull_request_diff_line(), 4);
        assert_eq!(app.selected_pull_request_review_comment_id(), None);
        assert!(screen.contains("R    4 row04"), "{screen}");
        let (y, row) = screen
            .lines()
            .enumerate()
            .find(|(_, row)| row.contains("row04"))
            .unwrap();
        let x = row[..row.find("row04").unwrap()].chars().count() as u16;
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y as u16,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.selected_pull_request_diff_line(), 4);
        assert_eq!(app.pull_request_review_side(), ReviewSide::Left);
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        let screen = review_screen(&mut terminal, &mut app);
        assert_eq!(app.selected_pull_request_diff_line(), 3);
        assert_eq!(app.selected_pull_request_review_comment_id(), Some(1));
        assert!(screen.contains("> [new open @r] c01"), "{screen}");
    }

    #[test]
    fn review_toolbar_buttons_match_their_visible_cells() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        for width in [80, 120] {
            let mut app = App::new(Config::default());
            app.set_view(View::PullRequestFiles);
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal
                .draw(|frame| crate::ui::draw(frame, &mut app))
                .unwrap();
            let rows = terminal
                .backend()
                .buffer()
                .content
                .chunks(width as usize)
                .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
                .collect::<Vec<_>>();
            let (y, row) = rows
                .iter()
                .enumerate()
                .find(|(_, row)| row.contains("[Files]"))
                .unwrap();
            for (label, expected, opposite) in [
                (
                    "[Files]",
                    PullRequestReviewFocus::Files,
                    PullRequestReviewFocus::Diff,
                ),
                (
                    "[Diff]",
                    PullRequestReviewFocus::Diff,
                    PullRequestReviewFocus::Files,
                ),
            ] {
                let x = row[..row.find(label).unwrap()].chars().count() as u16;
                for offset in 0..label.len() as u16 {
                    app.set_pull_request_review_focus(opposite);
                    app.on_mouse(MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column: x + offset,
                        row: y as u16,
                        modifiers: KeyModifiers::NONE,
                    });
                    assert_eq!(
                        app.pull_request_review_focus(),
                        expected,
                        "{label} cell {offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn panning_reveals_line_endings_with_wide_line_numbers() {
        for number in [9999, 10000, 1_000_000, i64::MAX - 2] {
            let next = number + 1;
            let old = format!("{}END!", "x".repeat(80));
            let new = format!("{}END!", "y".repeat(80));
            for (kind, patch, endings) in [
                (
                    DiffKind::Added,
                    format!("@@ -{number},0 +{number} @@\n+{new}"),
                    1,
                ),
                (
                    DiffKind::Removed,
                    format!("@@ -{number} +{number},0 @@\n-{old}"),
                    1,
                ),
                (
                    DiffKind::Changed,
                    format!("@@ -{number} +{next} @@\n-{old}\n+{new}"),
                    2,
                ),
                (
                    DiffKind::Context,
                    format!("@@ -{number} +{next} @@\n {old}"),
                    2,
                ),
            ] {
                for expanded in [false, true] {
                    let mut app = App::new(Config::default());
                    app.set_view(View::PullRequestFiles);
                    app.set_pull_request_files(
                        1,
                        "head".to_string(),
                        vec![PullRequestFile {
                            filename: "gutter.txt".to_string(),
                            status: "modified".to_string(),
                            additions: i64::from(matches!(
                                kind,
                                DiffKind::Added | DiffKind::Changed
                            )),
                            deletions: i64::from(matches!(
                                kind,
                                DiffKind::Removed | DiffKind::Changed
                            )),
                            patch: Some(patch.clone()),
                        }],
                    );
                    app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
                    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
                    if expanded {
                        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                    }
                    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
                    review_screen(&mut terminal, &mut app);
                    for _ in 0..app.pull_request_diff_horizontal_max() {
                        app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
                    }
                    let screen = review_screen(&mut terminal, &mut app);
                    assert_eq!(
                        screen.matches("END!").count(),
                        endings,
                        "line={number}, kind={kind:?}, expanded={expanded}\n{screen}"
                    );
                }
            }
        }
    }

    #[test]
    fn resizing_a_panned_diff_renders_correctly_on_the_first_frame() {
        let mut app = App::new(Config::default());
        app.set_view(View::PullRequestFiles);
        app.set_pull_request_files(
            1,
            "head".to_string(),
            vec![PullRequestFile {
                filename: "wide.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some(format!("@@ -1 +1 @@\n {}END", "content ".repeat(10))),
            }],
        );
        app.set_pull_request_review_focus(PullRequestReviewFocus::Diff);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw(frame, &mut app))
            .unwrap();
        for _ in 0..30 {
            app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        }
        assert!(app.pull_request_diff_horizontal_scroll() > 0);

        terminal.backend_mut().resize(320, 36);
        terminal
            .draw(|frame| crate::ui::draw(frame, &mut app))
            .unwrap();
        assert_eq!(app.pull_request_diff_horizontal_scroll(), 0);
        let first_frame = terminal.backend().buffer().clone();
        terminal
            .draw(|frame| crate::ui::draw(frame, &mut app))
            .unwrap();
        assert_eq!(&first_frame, terminal.backend().buffer());
    }

    #[test]
    fn scrolled_file_rows_keep_their_highlight_and_mouse_targets() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

        for (width, height) in [(80, 16), (120, 24)] {
            let mut app = App::new(Config::default());
            app.set_view(View::PullRequestFiles);
            app.set_pull_request_files(
                1,
                "head".to_string(),
                (0..40)
                    .map(|index| PullRequestFile {
                        filename: format!("file_{index:02}.rs"),
                        status: "modified".to_string(),
                        additions: 1,
                        deletions: 0,
                        patch: Some("@@ -1 +1 @@\n context".to_string()),
                    })
                    .collect(),
            );
            app.set_pull_request_review_focus(PullRequestReviewFocus::Files);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for selected in [0, 39, 38] {
                app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
                app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
                for _ in 0..selected {
                    app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
                }
                terminal
                    .draw(|frame| crate::ui::draw(frame, &mut app))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let visible = buffer
                    .content
                    .chunks(width as usize)
                    .enumerate()
                    .filter_map(|(y, row)| {
                        let text = row[..width as usize / 3]
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>();
                        let byte = text.find("file_")?;
                        let x = text[..byte].chars().count() as u16;
                        let index = text[byte + 5..byte + 7].parse::<usize>().unwrap();
                        Some((x, y as u16, index))
                    })
                    .collect::<Vec<_>>();
                assert!(!visible.is_empty());
                let &(x, y, _) = visible
                    .iter()
                    .find(|(_, _, index)| *index == selected)
                    .expect("selected file remains visible");
                assert_eq!(buffer[(x, y)].bg, resolve_theme(None).bg_selected);
                for (x, y, index) in visible {
                    app.on_mouse(MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column: x,
                        row: y,
                        modifiers: KeyModifiers::NONE,
                    });
                    assert_eq!(app.selected_pull_request_file(), index);
                }
            }
        }
    }
}
