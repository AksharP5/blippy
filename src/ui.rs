use ratatui::layout::Margin;
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
    }
}

fn draw_repo_picker(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
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
        area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_repo()),
    );

    draw_status(frame, app, area);
}

fn draw_remote_chooser(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default().title("Choose Remote").borders(Borders::ALL);
    let items = app
        .remotes()
        .iter()
        .map(|remote| {
            let label = format!("{} → {}/{}", remote.name, remote.slug.owner, remote.slug.repo);
            ListItem::new(label)
        })
        .collect::<Vec<ListItem>>();
    let list = List::new(items).block(block).highlight_symbol("> ");
    frame.render_stateful_widget(
        list,
        area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_remote()),
    );

    draw_status(frame, app, area);
}

fn draw_issues(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
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
        area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
        &mut list_state(app.selected_issue()),
    );

    draw_status(frame, app, area);
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let status = app.status();
    if status.is_empty() {
        return;
    }

    let text = Text::from(Line::from(status));
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .block(Block::default());
    let status_area = area.inner(Margin {
        vertical: 0,
        horizontal: 2,
    });
    frame.render_widget(paragraph, status_area);
}

fn list_state(selected: usize) -> ListState {
    let mut state = ListState::default();
    state.select(Some(selected));
    state
}
