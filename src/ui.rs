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
const HEADER_HEIGHT: u16 = 1;

mod ui_editor_views;
mod ui_issue_detail;
mod ui_issues;
mod ui_metadata;
mod ui_pull_request;
mod ui_repo;
mod ui_shared;
mod ui_status_overlay;

use ui_shared::*;

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &ThemePalette) {
    let view_name = match app.view() {
        View::RepoPicker => "Repositories",
        View::RemoteChooser => "Remotes",
        View::Issues => {
            if app.work_item_mode() == crate::app::WorkItemMode::PullRequests {
                "Pull Requests"
            } else {
                "Issues"
            }
        }
        View::IssueDetail => {
            if app.current_issue_row().is_some_and(|issue| issue.is_pr) {
                "Pull Request Detail"
            } else {
                "Issue Detail"
            }
        }
        View::IssueComments => "Comments",
        View::PullRequestFiles => "Files",
        View::LabelPicker => "Labels",
        View::AssigneePicker => "Assignees",
        View::CommentPresetPicker => "Close",
        View::CommentPresetName => "Preset Name",
        View::CommentEditor => "Editor",
    };

    let repo_context = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => format!("{}/{}", owner, repo),
        _ => "no repo selected".to_string(),
    };
    let context = repo_context;

    let title_prefix = format!("{} • ", view_name);
    let title_width = title_prefix.chars().count();
    let max_context = (area.width as usize).saturating_sub(title_width + 10);
    let context = fit_inline(context.as_str(), max_context);

    let line = Line::from(vec![
        Span::styled(
            " blippy ",
            Style::default()
                .fg(theme.bg_app)
                .bg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            title_prefix,
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(context, Style::default().fg(theme.text_muted)),
    ]);
    let header = Paragraph::new(line).style(
        Style::default()
            .bg(theme.bg_panel_alt)
            .fg(theme.text_primary),
    );

    frame.render_widget(header, area);
}

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let theme = resolve_theme(app.theme_name());
    let area = frame.area();
    app.clear_mouse_regions();

    // Clear background
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg_app)),
        area,
    );

    // Standard 3-row layout: header | content | footer
    let [header_area, content_area, footer_area] = Layout::vertical([
        Constraint::Length(HEADER_HEIGHT),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    // Draw header
    draw_header(frame, app, header_area, theme);

    // Draw main content based on view
    match app.view() {
        View::RepoPicker => ui_repo::draw_repo_picker(frame, app, content_area, theme),
        View::RemoteChooser => ui_repo::draw_remote_chooser(frame, app, content_area, theme),
        View::Issues => ui_issues::draw_issues(frame, app, content_area, theme),
        View::IssueDetail => ui_issue_detail::draw_issue_detail(frame, app, content_area, theme),
        View::IssueComments => {
            ui_issue_detail::draw_issue_comments(frame, app, content_area, theme)
        }
        View::PullRequestFiles => {
            ui_pull_request::draw_pull_request_files(frame, app, content_area, theme)
        }
        View::LabelPicker => ui_metadata::draw_label_picker(frame, app, content_area, theme),
        View::AssigneePicker => ui_metadata::draw_assignee_picker(frame, app, content_area, theme),
        View::CommentPresetPicker => {
            ui_editor_views::draw_preset_picker(frame, app, content_area, theme)
        }
        View::CommentPresetName => {
            ui_editor_views::draw_preset_name(frame, app, content_area, theme)
        }
        View::CommentEditor => {
            ui_editor_views::draw_comment_editor(frame, app, content_area, theme)
        }
    }

    // Draw footer status bar
    ui_status_overlay::draw_status(frame, app, footer_area, theme);
    if app.help_overlay_visible() {
        ui_status_overlay::draw_help_overlay(frame, app, area, theme);
    }
}
