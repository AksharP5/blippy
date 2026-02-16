use super::*;

pub(crate) fn handle_preset_selection(
    app: &mut App,
    _conn: &rusqlite::Connection,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    match app.preset_selection() {
        PresetSelection::CloseWithoutComment => {
            close_issue_with_comment(app, token, None, event_tx)?;
        }
        PresetSelection::CustomMessage => {
            app.open_close_comment_editor();
        }
        PresetSelection::Preset(index) => {
            let body = app
                .comment_defaults()
                .get(index)
                .map(|preset| preset.body.clone());
            if body.is_none() {
                app.set_status("Preset not found".to_string());
                return Ok(());
            }
            close_issue_with_comment(app, token, body, event_tx)?;
        }
        PresetSelection::AddPreset => {
            app.editor_mut().reset_for_preset_name();
            app.set_view(View::CommentPresetName);
        }
    }
    Ok(())
}

pub(crate) fn save_preset_from_editor(app: &mut App) -> Result<()> {
    let name = app.editor().name().trim().to_string();
    if name.is_empty() {
        app.set_status("Preset name required".to_string());
        return Ok(());
    }
    let body = app.editor().text().to_string();
    if body.trim().is_empty() {
        app.set_status("Preset body required".to_string());
        return Ok(());
    }

    app.add_comment_default(crate::config::CommentDefault { name, body });
    app.save_config()?;
    app.set_status("Preset saved".to_string());
    Ok(())
}
