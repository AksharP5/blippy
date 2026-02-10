use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct BindingSpec {
    pub action: &'static str,
    pub default: &'static str,
    #[allow(dead_code)]
    pub description: &'static str,
}

pub const BINDING_SPECS: &[BindingSpec] = &[
    BindingSpec { action: "quit", default: "q", description: "Quit app" },
    BindingSpec {
        action: "clear_and_repo_picker",
        default: "ctrl+g",
        description: "Clear search and open repo picker",
    },
    BindingSpec { action: "repo_search", default: "/", description: "Search repos" },
    BindingSpec { action: "issue_search", default: "/", description: "Search issues" },
    BindingSpec {
        action: "cycle_issue_filter",
        default: "f",
        description: "Cycle open/closed filter",
    },
    BindingSpec {
        action: "toggle_work_item_mode",
        default: "p",
        description: "Toggle issues/pull requests mode",
    },
    BindingSpec {
        action: "cycle_assignee_filter",
        default: "a",
        description: "Cycle assignee filter",
    },
    BindingSpec {
        action: "issue_filter_open",
        default: "1",
        description: "Open issues tab",
    },
    BindingSpec {
        action: "issue_filter_closed",
        default: "2",
        description: "Closed issues tab",
    },
    BindingSpec { action: "refresh", default: "r", description: "Refresh data" },
    BindingSpec { action: "jump_prefix", default: "g", description: "Jump prefix" },
    BindingSpec { action: "jump_bottom", default: "shift+g", description: "Jump bottom" },
    BindingSpec {
        action: "open_comments",
        default: "c",
        description: "Open comments view",
    },
    BindingSpec {
        action: "add_comment",
        default: "m",
        description: "Add comment",
    },
    BindingSpec {
        action: "toggle_file_viewed",
        default: "w",
        description: "Toggle PR file viewed state",
    },
    BindingSpec {
        action: "collapse_hunk",
        default: "z",
        description: "Collapse/expand current diff hunk",
    },
    BindingSpec {
        action: "edit_comment",
        default: "e",
        description: "Edit selected comment",
    },
    BindingSpec {
        action: "delete_comment",
        default: "x",
        description: "Delete selected comment",
    },
    BindingSpec {
        action: "resolve_thread",
        default: "shift+r",
        description: "Resolve/reopen selected review thread",
    },
    BindingSpec {
        action: "next_line_comment",
        default: "n",
        description: "Next line comment",
    },
    BindingSpec {
        action: "prev_line_comment",
        default: "p",
        description: "Previous line comment",
    },
    BindingSpec {
        action: "review_side_left",
        default: "h",
        description: "Select left diff side",
    },
    BindingSpec {
        action: "review_side_right",
        default: "l",
        description: "Select right diff side",
    },
    BindingSpec {
        action: "visual_mode",
        default: "shift+v",
        description: "Toggle visual range selection",
    },
    BindingSpec { action: "edit_labels", default: "l", description: "Edit labels" },
    BindingSpec {
        action: "edit_assignees",
        default: "shift+a",
        description: "Edit assignees",
    },
    BindingSpec { action: "reopen_issue", default: "u", description: "Reopen issue" },
    BindingSpec {
        action: "popup_toggle",
        default: "space",
        description: "Toggle popup item",
    },
    BindingSpec {
        action: "submit",
        default: "enter",
        description: "Submit or activate selection",
    },
    BindingSpec { action: "back", default: "b", description: "Back" },
    BindingSpec {
        action: "back_escape",
        default: "esc",
        description: "Back via escape",
    },
    BindingSpec { action: "move_up", default: "k", description: "Move up" },
    BindingSpec { action: "move_down", default: "j", description: "Move down" },
    BindingSpec {
        action: "open_browser",
        default: "o",
        description: "Open issue/PR in browser",
    },
    BindingSpec {
        action: "open_linked_pr_browser",
        default: "shift+o",
        description: "Open linked PR in browser",
    },
    BindingSpec {
        action: "open_linked_pr_tui",
        default: "shift+p",
        description: "Open linked PR in TUI",
    },
    BindingSpec {
        action: "checkout_pr",
        default: "v",
        description: "Checkout selected pull request",
    },
    BindingSpec {
        action: "copy_status",
        default: "ctrl+y",
        description: "Copy status text",
    },
    BindingSpec { action: "page_up", default: "ctrl+u", description: "Page up" },
    BindingSpec {
        action: "page_down",
        default: "ctrl+d",
        description: "Page down",
    },
    BindingSpec {
        action: "focus_left",
        default: "ctrl+h",
        description: "Move pane focus left",
    },
    BindingSpec {
        action: "focus_right",
        default: "ctrl+l",
        description: "Move pane focus right",
    },
    BindingSpec {
        action: "rescan_repos",
        default: "ctrl+r",
        description: "Rescan local repositories",
    },
    BindingSpec {
        action: "diff_scroll_left",
        default: "[",
        description: "Pan PR diff left",
    },
    BindingSpec {
        action: "diff_scroll_right",
        default: "]",
        description: "Pan PR diff right",
    },
    BindingSpec {
        action: "diff_scroll_reset",
        default: "0",
        description: "Reset PR diff horizontal pan",
    },
];

#[derive(Debug, Default, Clone)]
pub struct Keybinds {
    remap: HashMap<String, KeyEvent>,
    disabled_defaults: HashSet<String>,
}

impl Keybinds {
    pub fn from_overrides(overrides: &HashMap<String, String>) -> Self {
        let mut remap = HashMap::new();
        let mut default_usage = HashMap::new();
        let mut overridden_usage = HashMap::new();

        for spec in BINDING_SPECS {
            let default_key = normalize_binding(spec.default).unwrap_or_default();
            *default_usage.entry(default_key).or_insert(0usize) += 1;
        }

        for spec in BINDING_SPECS {
            let default_event = match parse_binding(spec.default) {
                Some(default_event) => default_event,
                None => continue,
            };
            let default_key = normalize_event(default_event);
            let override_key = overrides
                .get(spec.action)
                .and_then(|binding| normalize_binding(binding));

            if let Some(override_key) = override_key {
                remap.insert(override_key.clone(), default_event);
                if override_key != default_key {
                    *overridden_usage.entry(default_key.clone()).or_insert(0usize) += 1;
                }
                continue;
            }

            remap.insert(default_key, default_event);
        }

        let mut disabled_defaults = HashSet::new();
        for (default_key, total) in default_usage {
            let overridden = overridden_usage.get(default_key.as_str()).copied().unwrap_or(0usize);
            if overridden >= total && total > 0 {
                disabled_defaults.insert(default_key);
            }
        }

        Self {
            remap,
            disabled_defaults,
        }
    }

    pub fn remap_key(&self, key: KeyEvent) -> Option<KeyEvent> {
        let normalized = normalize_event(key);
        if let Some(mapped) = self.remap.get(normalized.as_str()) {
            return Some(KeyEvent::new(mapped.code, mapped.modifiers));
        }
        if self.disabled_defaults.contains(normalized.as_str()) {
            return None;
        }
        Some(key)
    }
}

pub fn parse_binding(binding: &str) -> Option<KeyEvent> {
    let tokens = binding
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<&str>>();
    if tokens.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::empty();
    for token in &tokens[..tokens.len().saturating_sub(1)] {
        let token = token.to_ascii_lowercase();
        if token == "ctrl" || token == "control" {
            modifiers |= KeyModifiers::CONTROL;
            continue;
        }
        if token == "alt" {
            modifiers |= KeyModifiers::ALT;
            continue;
        }
        if token == "shift" {
            modifiers |= KeyModifiers::SHIFT;
            continue;
        }
        return None;
    }

    let key_token_raw = tokens[tokens.len() - 1].trim();
    let key_token = key_token_raw.to_ascii_lowercase();
    let code = if key_token == "esc" || key_token == "escape" {
        KeyCode::Esc
    } else if key_token == "enter" || key_token == "return" {
        KeyCode::Enter
    } else if key_token == "tab" {
        KeyCode::Tab
    } else if key_token == "backspace" {
        KeyCode::Backspace
    } else if key_token == "space" {
        KeyCode::Char(' ')
    } else if key_token == "up" {
        KeyCode::Up
    } else if key_token == "down" {
        KeyCode::Down
    } else if key_token == "left" {
        KeyCode::Left
    } else if key_token == "right" {
        KeyCode::Right
    } else if key_token == "home" {
        KeyCode::Home
    } else if key_token == "end" {
        KeyCode::End
    } else if key_token == "pageup" {
        KeyCode::PageUp
    } else if key_token == "pagedown" {
        KeyCode::PageDown
    } else if key_token_raw.chars().count() == 1 {
        let ch = key_token_raw.chars().next().unwrap_or_default();
        if modifiers.contains(KeyModifiers::SHIFT) && ch.is_ascii_alphabetic() {
            KeyCode::Char(ch.to_ascii_uppercase())
        } else {
            KeyCode::Char(ch.to_ascii_lowercase())
        }
    } else {
        return None;
    };

    Some(KeyEvent::new(code, modifiers))
}

pub fn normalize_binding(binding: &str) -> Option<String> {
    parse_binding(binding).map(normalize_event)
}

pub fn normalize_event(event: KeyEvent) -> String {
    let mut tokens = Vec::new();
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        tokens.push("ctrl".to_string());
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        tokens.push("alt".to_string());
    }
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        tokens.push("shift".to_string());
    }

    let key = match event.code {
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) => c.to_ascii_lowercase().to_string(),
        _ => "".to_string(),
    };
    if key.is_empty() {
        return String::new();
    }
    tokens.push(key);
    tokens.join("+")
}

#[cfg(test)]
mod tests {
    use super::{normalize_binding, parse_binding, Keybinds};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::collections::HashMap;

    #[test]
    fn parse_binding_supports_named_keys_and_modifiers() {
        let key = parse_binding("ctrl+shift+left").expect("parse binding");
        assert_eq!(key.code, KeyCode::Left);
        assert!(key.modifiers.contains(KeyModifiers::CONTROL));
        assert!(key.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn normalize_binding_converts_aliases() {
        let binding = normalize_binding("Control+Return").expect("normalize binding");
        assert_eq!(binding, "ctrl+enter");
    }

    #[test]
    fn keybinds_remap_override_to_default_command() {
        let mut overrides = HashMap::new();
        overrides.insert("quit".to_string(), "ctrl+q".to_string());
        let keybinds = Keybinds::from_overrides(&overrides);

        let remapped = keybinds
            .remap_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL))
            .expect("remapped key");
        assert_eq!(remapped.code, KeyCode::Char('q'));
        assert!(remapped.modifiers.is_empty());

        let disabled_default = keybinds.remap_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(disabled_default.is_none());
    }
}
