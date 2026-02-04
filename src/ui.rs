use ratatui::layout::{Alignment, Margin};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame<'_>, _app: &App) {
    let area = frame.area();
    let block = Block::default().title("Glyph").borders(Borders::ALL);
    let text = vec![
        Line::from("Maintainer Inbox Cockpit"),
        Line::from(""),
        Line::from("Press 'q' to quit."),
    ];
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(block);

    frame.render_widget(
        paragraph,
        area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        }),
    );
}
