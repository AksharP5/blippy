use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    pub keymap: Option<String>,
    pub theme: Option<String>,
    #[serde(default)]
    pub keybinds: HashMap<String, String>,
    #[serde(skip)]
    pub file_keybinds: HashMap<String, String>,
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
            let mut file = fs::File::open(&path)
                .with_context(|| format!("Failed to read config at {}", path.display()))?;
            file.lock_shared()
                .with_context(|| format!("Failed to lock config at {}", path.display()))?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)
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
            config.file_keybinds = keybinds_file.keybinds;
        }
        Ok(config)
    }

    pub fn effective_keybinds(&self) -> HashMap<String, String> {
        let mut keybinds = self.keybinds.clone();
        keybinds.extend(self.file_keybinds.clone());
        keybinds
    }

    pub fn save_comment_default(&mut self, preset: CommentDefault) -> Result<()> {
        let path = config_path();
        let new_file = !path.exists();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir at {}", parent.display()))?;
        }
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("Failed to open config at {}", path.display()))?;
        file.lock()
            .with_context(|| format!("Failed to lock config at {}", path.display()))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let mut current: Self = if new_file && contents.trim().is_empty() {
            self.clone()
        } else {
            toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config at {}", path.display()))?
        };
        current.comment_defaults.push(preset);
        let contents = toml::to_string_pretty(&current).context("Failed to serialize config")?;
        file.rewind()
            .with_context(|| format!("Failed to seek config at {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .and_then(|()| file.set_len(contents.len() as u64))
            .with_context(|| format!("Failed to write config at {}", path.display()))?;
        self.comment_defaults = current.comment_defaults;
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
    fn saving_presets_keeps_keybinding_file_overrides_out_of_the_main_config() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        const CONFIG_ENV: &str = "BLIPPY_TEST_BINDING_CONFIG";
        let Some(config_dir) = std::env::var_os(CONFIG_ENV) else {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("timestamp")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("blippy-config-overrides-{nanos}"));
            std::fs::create_dir_all(dir.join("blippy")).expect("config directory");
            std::fs::write(
                dir.join("blippy/config.toml"),
                "theme = \"midnight\"\n[keybinds]\nquit = \"ctrl+q\"\nrefresh = \"ctrl+s\"\n",
            )
            .expect("base config");
            std::fs::write(
                dir.join("blippy/keybinds.toml"),
                "[keybinds]\nquit = \"ctrl+x\"\n",
            )
            .expect("keybinding override");
            let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "config::tests::saving_presets_keeps_keybinding_file_overrides_out_of_the_main_config",
                ])
                .env(CONFIG_ENV, &dir)
                .env("XDG_CONFIG_HOME", &dir)
                .output()
                .expect("isolated config test");
            std::fs::remove_dir_all(&dir).expect("remove test config");
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        };

        let dir = std::path::PathBuf::from(config_dir).join("blippy");
        let mut app = crate::app::App::new(Config::load().expect("load effective config"));
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
        assert!(
            app.should_quit(),
            "keybinds.toml takes precedence at runtime"
        );

        let mut config = Config::load().expect("load config for preset");
        config
            .save_comment_default(super::CommentDefault {
                name: "resolved".into(),
                body: "Closing as resolved.".into(),
            })
            .expect("save preset");
        let saved: Config = toml::from_str(
            &std::fs::read_to_string(dir.join("config.toml")).expect("read saved config"),
        )
        .expect("parse saved config");
        assert_eq!(
            saved.keybinds.get("quit").map(String::as_str),
            Some("ctrl+q")
        );
        assert_eq!(
            saved.keybinds.get("refresh").map(String::as_str),
            Some("ctrl+s")
        );
        assert_eq!(saved.theme.as_deref(), Some("midnight"));
        assert_eq!(saved.comment_defaults, config.comment_defaults);

        std::fs::write(dir.join("keybinds.toml"), "[keybinds]\n").expect("remove override");
        let mut app = crate::app::App::new(Config::load().expect("load without override"));
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
        assert!(!app.should_quit());
        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(
            app.should_quit(),
            "removing the override restores the base binding"
        );
    }

    #[test]
    fn preset_saves_preserve_changes_from_other_sessions() {
        const CONFIG_ENV: &str = "BLIPPY_TEST_SHARED_PRESETS";
        let Some(config_dir) = std::env::var_os(CONFIG_ENV) else {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("blippy-shared-presets-{nanos}"));
            std::fs::create_dir_all(dir.join("blippy")).unwrap();
            std::fs::write(
                dir.join("blippy/config.toml"),
                "[keybinds]\nquit = \"ctrl+q\"\n",
            )
            .unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "config::tests::preset_saves_preserve_changes_from_other_sessions",
                    "--nocapture",
                ])
                .env(CONFIG_ENV, &dir)
                .env("XDG_CONFIG_HOME", &dir)
                .output()
                .unwrap();
            std::fs::remove_dir_all(&dir).unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        };
        let mut first = crate::app::App::new(Config::load().unwrap());
        let mut second = crate::app::App::new(Config::load().unwrap());
        let path = std::path::PathBuf::from(config_dir).join("blippy/config.toml");
        // A config edit and both sessions' saves affect the same shared file.
        std::fs::write(
            &path,
            "theme = \"midnight\"\n[keybinds]\nquit = \"ctrl+x\"\n",
        )
        .unwrap();
        for (app, name) in [(&mut first, "first"), (&mut second, "second")] {
            app.save_comment_default(super::CommentDefault {
                name: name.into(),
                body: format!("{name} body"),
            })
            .unwrap();
        }
        let saved = Config::load().unwrap();
        let names = saved
            .comment_defaults
            .iter()
            .map(|preset| preset.name.as_str())
            .collect::<Vec<_>>();
        eprintln!(
            "saved presets={names:?}, theme={:?}, quit={:?}",
            saved.theme,
            saved.keybinds.get("quit")
        );
        assert_eq!(names, ["first", "second"]);
        assert_eq!(saved.theme.as_deref(), Some("midnight"));
        assert_eq!(
            saved.keybinds.get("quit").map(String::as_str),
            Some("ctrl+x")
        );
        assert_eq!(second.comment_defaults(), saved.comment_defaults);

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let writers = (0..2)
            .map(|index| {
                let mut app = crate::app::App::new(Config::load().unwrap());
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    app.save_comment_default(super::CommentDefault {
                        name: format!("writer{index}"),
                        body: "Parallel save".into(),
                    })
                    .unwrap();
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        for writer in writers {
            writer.join().unwrap();
        }
        let saved = Config::load().unwrap();
        let mut names = saved
            .comment_defaults
            .iter()
            .map(|preset| preset.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(names, ["first", "second", "writer0", "writer1"]);

        std::fs::write(&path, "").unwrap();
        second
            .save_comment_default(super::CommentDefault {
                name: "after-reset".into(),
                body: "Only the new preset".into(),
            })
            .unwrap();
        let saved = Config::load().unwrap();
        assert_eq!(saved.comment_defaults.len(), 1);
        assert_eq!(saved.comment_defaults[0].name, "after-reset");
        assert!(saved.theme.is_none());
        assert!(saved.keybinds.is_empty());
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
