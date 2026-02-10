use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub keymap: Option<String>,
    #[serde(default)]
    pub keybinds: HashMap<String, String>,
    #[serde(default)]
    pub comment_defaults: Vec<CommentDefault>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CommentDefault {
    pub name: String,
    pub body: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let mut config = if !path.exists() {
            Self::default()
        } else {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config at {}", path.display()))?;
            toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config at {}", path.display()))?
        };

        let keybinds_path = keybinds_path();
        if keybinds_path.exists() {
            let contents = fs::read_to_string(&keybinds_path)
                .with_context(|| format!("Failed to read config at {}", keybinds_path.display()))?;
            let keybinds_file: KeybindsFile = toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config at {}", keybinds_path.display()))?;
            config.keybinds.extend(keybinds_file.keybinds);
        }
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir at {}", parent.display()))?;
        }
        let contents = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write config at {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parses_comment_defaults() {
        let input = r#"
            [[comment_defaults]]
            name = "close_default"
            body = "Closing this issue"
        "#;

        let config: Config = toml::from_str(input).expect("parse config");
        assert_eq!(config.comment_defaults.len(), 1);
        assert_eq!(config.comment_defaults[0].name, "close_default");
    }

    #[test]
    fn parses_keybind_overrides() {
        let input = r#"
            [keybinds]
            quit = "ctrl+q"
            refresh = "ctrl+s"
        "#;

        let config: Config = toml::from_str(input).expect("parse config");
        assert_eq!(config.keybinds.get("quit"), Some(&"ctrl+q".to_string()));
        assert_eq!(config.keybinds.get("refresh"), Some(&"ctrl+s".to_string()));
    }
}

#[derive(Debug, Default, Deserialize)]
struct KeybindsFile {
    #[serde(default)]
    keybinds: HashMap<String, String>,
}

fn config_path() -> PathBuf {
    config_dir().join("glyph").join("config.toml")
}

fn keybinds_path() -> PathBuf {
    config_dir().join("glyph").join("keybinds.toml")
}

fn config_dir() -> PathBuf {
    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        return Path::new(&dir).to_path_buf();
    }

    if let Ok(home) = env::var("HOME") {
        return Path::new(&home).join(".config");
    }

    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
