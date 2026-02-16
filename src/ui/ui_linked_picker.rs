use super::*;

pub(super) fn draw_linked_picker(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: ratatui::layout::Rect,
    theme: &ThemePalette,
) {
    let popup = ui_status_overlay::centered_rect(64, 64, area);
    frame.render_widget(Clear, popup);

    let block = popup_block(app.linked_picker_title(), theme);
    frame.render_widget(block, popup);

    let options = app.linked_picker_labels();
    let items = options
        .iter()
        .map(|label| ListItem::new(label.as_str()))
        .collect::<Vec<ListItem>>();
    let list_area = popup.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let list = List::new(items)
        .style(Style::default().fg(theme.text_primary).bg(theme.bg_popup))
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(theme.bg_selected)
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        list,
        list_area,
        &mut list_state(app.selected_linked_picker_index()),
    );

    let list_inner = list_area.inner(Margin {
        vertical: 0,
        horizontal: 1,
    });
    let max_rows = list_inner.height.saturating_sub(1) as usize;
    for index in 0..options.len().min(max_rows) {
        let y = list_inner.y.saturating_add(index as u16);
        app.register_mouse_region(
            MouseTarget::LinkedPickerOption(index),
            list_inner.x,
            y,
            list_inner.width,
            1,
        );
    }

    if list_inner.height > 0 {
        let hint_y = list_inner
            .y
            .saturating_add(list_inner.height.saturating_sub(1));
        let hint = "Esc to cancel";
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(theme.text_muted).bg(theme.bg_popup)),
            Rect {
                x: list_inner.x,
                y: hint_y,
                width: list_inner.width,
                height: 1,
            },
        );
        app.register_mouse_region(
            MouseTarget::LinkedPickerCancel,
            list_inner.x,
            hint_y,
            hint.chars().count() as u16,
            1,
        );
    }
}
