use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, View};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    match app.view() {
        View::RepoPicker => draw_repo_picker(frame, app, area),
        View::RemoteChooser => draw_remote_chooser(frame, app, area),
        View::Issues => draw_issues(frame, app, area),
        View::IssueDetail => draw_issue_detail(frame, app, area),
        View::CommentPresetPicker => draw_preset_picker(frame, app, area),
        View::CommentPresetName => draw_preset_name(frame, app, area),
        View::CommentEditor => draw_comment_editor(frame, app, area),
    }
}

fn draw_repo_picker(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let block = Block::default().title("Repos").borders(Borders::ALL);
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
    let list = List::new(items).block(block).highlight_symbol("> ");
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
    let block = Block::default().title("Choose Remote").borders(Borders::ALL);
    let items = app
        .remotes()
        .iter()
        .map(|remote| {
            let label = format!("{} -> {}/{}", remote.name, remote.slug.owner, remote.slug.repo);
            ListItem::new(label)
        })
        .collect::<Vec<ListItem>>();
    let list = List::new(items).block(block).highlight_symbol("> ");
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
    let block = Block::default().title("Issues").borders(Borders::ALL);
    let items = if app.issues().is_empty() {
        vec![ListItem::new("No cached issues yet. Run `glyph sync`.")]
    } else {
        app.issues()
            .iter()
            .map(|issue| ListItem::new(issue.title.as_str()))
            .collect()
    };
    let list = List::new(items).block(block).highlight_symbol("> ");
    frame.render_stateful_widget(
        list,
        main.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_issue()),
    );

    draw_status(frame, app, footer);
}

fn draw_issue_detail(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let title = app
        .issues()
        .get(app.selected_issue())
        .map(|issue| issue.title.as_str())
        .unwrap_or("Issue");
    let block = Block::default().title(title).borders(Borders::ALL);
    let items = if app.comments().is_empty() {
        vec![ListItem::new("No comments cached yet.")]
    } else {
        app.comments()
            .iter()
            .map(|comment| ListItem::new(comment.body.as_str()))
            .collect()
    };
    let list = List::new(items).block(block).highlight_symbol("> ");
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
    let block = Block::default().title("Close Issue").borders(Borders::ALL);
    let mut items = Vec::new();
    items.push(ListItem::new("Close without comment"));
    items.push(ListItem::new("Custom message..."));
    for preset in app.comment_defaults() {
        items.push(ListItem::new(preset.name.as_str()));
    }
    items.push(ListItem::new("Add preset..."));

    let list = List::new(items).block(block).highlight_symbol("> ");
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
    let block = Block::default().title("Preset Name").borders(Borders::ALL);
    let text = app.editor().name();
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, main.inner(Margin { vertical: 1, horizontal: 2 }));

    draw_status(frame, app, footer);
}

fn draw_comment_editor(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let (main, footer) = split_area(area);
    let block = Block::default().title("Comment").borders(Borders::ALL);
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
        .block(Block::default());
    frame.render_widget(paragraph, area.inner(Margin { vertical: 0, horizontal: 2 }));
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
            "j/k or ↑/↓ move • gg/G top/bottom • b/Esc back • r refresh • o browser • Ctrl+G repos • q quit"
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
