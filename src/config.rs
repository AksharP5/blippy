use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub keymap: Option<String>,
    pub theme: Option<String>,
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
            let keybinds_file: KeybindsFile = toml::from_str(&contents).with_context(|| {
                format!("Failed to parse config at {}", keybinds_path.display())
            })?;
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
        let contents =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize config")?;
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

    #[test]
    fn parses_theme_name() {
        let input = r#"
            theme = "midnight"
        "#;

        let config: Config = toml::from_str(input).expect("parse config");
        assert_eq!(config.theme.as_deref(), Some("midnight"));
    }

    #[test]
    fn config_paths_use_the_windows_profile_without_home() {
        use std::path::PathBuf;
        use std::process::Command;

        const EXPECTED_DIR: &str = "BLIPPY_TEST_CONFIG_DIR";
        if let Some(expected) = std::env::var_os(EXPECTED_DIR) {
            let expected = PathBuf::from(expected).join("blippy");
            assert_eq!(super::config_path(), expected.join("config.toml"));
            assert_eq!(super::keybinds_path(), expected.join("keybinds.toml"));
            return;
        }

        let profile = std::env::temp_dir().join("blippy-test-profile");
        let xdg = profile.join("custom-config");
        for override_dir in [None, Some(PathBuf::new()), Some(xdg)] {
            let expected = override_dir
                .as_ref()
                .filter(|dir| !dir.as_os_str().is_empty())
                .cloned()
                .unwrap_or_else(|| profile.join(".config"));
            let mut command = Command::new(std::env::current_exe().expect("test executable"));
            command
                .args([
                    "--exact",
                    "config::tests::config_paths_use_the_windows_profile_without_home",
                ])
                .env_remove("HOME")
                .env_remove("XDG_CONFIG_HOME")
                .env("USERPROFILE", &profile)
                .env(EXPECTED_DIR, expected);
            if let Some(override_dir) = override_dir {
                command.env("XDG_CONFIG_HOME", override_dir);
            }
            let output = command.output().expect("isolated config test");
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct KeybindsFile {
    #[serde(default)]
    keybinds: HashMap<String, String>,
}

fn config_path() -> PathBuf {
    config_dir().join("blippy").join("config.toml")
}

fn keybinds_path() -> PathBuf {
    config_dir().join("blippy").join("keybinds.toml")
}

fn config_dir() -> PathBuf {
    if let Ok(dir) = env::var("XDG_CONFIG_HOME")
        && !dir.is_empty()
    {
        return Path::new(&dir).to_path_buf();
    }

    if let Some(home) = crate::discovery::home_dir() {
        return home.join(".config");
    }

    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
