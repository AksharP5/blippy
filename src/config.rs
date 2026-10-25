use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
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
        let path = resolve_config_path(config_path())?;
        let parent = path
            .parent()
            .context("Config path has no parent directory")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config dir at {}", parent.display()))?;
        // Lock a stable file: replacing config.toml changes its file identity.
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))
            .with_context(|| format!("Failed to lock config at {}", path.display()))?;
        lock.lock()
            .with_context(|| format!("Failed to lock config at {}", path.display()))?;
        let mut current: Self = match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config at {}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to read config at {}", path.display()));
            }
        };
        current.comment_defaults.push(preset);
        let contents = toml::to_string_pretty(&current).context("Failed to serialize config")?;
        let mut file = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("Failed to stage config at {}", path.display()))?;
        if path.exists() {
            let permissions = fs::metadata(&path)
                .with_context(|| {
                    format!("Failed to read config permissions at {}", path.display())
                })?
                .permissions();
            file.as_file()
                .set_permissions(permissions)
                .with_context(|| {
                    format!(
                        "Failed to preserve config permissions at {}",
                        path.display()
                    )
                })?;
        }
        file.write_all(contents.as_bytes())
            .and_then(|()| file.as_file().sync_all())
            .with_context(|| format!("Failed to write config at {}", path.display()))?;
        file.persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("Failed to replace config at {}", path.display()))?;
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

        std::fs::remove_file(&path).unwrap();
        second
            .save_comment_default(super::CommentDefault {
                name: "after-delete".into(),
                body: "Only the new preset after deletion".into(),
            })
            .unwrap();
        let saved = Config::load().unwrap();
        assert_eq!(saved.comment_defaults.len(), 1);
        assert_eq!(saved.comment_defaults[0].name, "after-delete");
        assert!(saved.theme.is_none());
        assert!(saved.keybinds.is_empty());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let target = path.with_file_name("linked-config.toml");
            std::fs::rename(&path, &target).unwrap();
            std::os::unix::fs::symlink(&target, &path).unwrap();
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
            second
                .save_comment_default(super::CommentDefault {
                    name: "linked".into(),
                    body: "Keep the symlink".into(),
                })
                .unwrap();
            assert!(path.is_symlink());
            assert_eq!(Config::load().unwrap().comment_defaults[1].name, "linked");
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o640
            );

            for name in ["recreated", "relative"] {
                std::fs::remove_file(&target).unwrap();
                if name == "relative" {
                    std::fs::remove_file(&path).unwrap();
                    std::os::unix::fs::symlink("linked-config.toml", &path).unwrap();
                }
                second
                    .save_comment_default(super::CommentDefault {
                        name: name.into(),
                        body: "Recreate the target, keeping the link".into(),
                    })
                    .unwrap();
                assert!(path.is_symlink(), "keep the {name} config link");
                let saved: Config =
                    toml::from_str(&std::fs::read_to_string(&target).unwrap()).unwrap();
                assert_eq!(saved.comment_defaults.len(), 1);
                assert_eq!(saved.comment_defaults[0].name, name);
            }

            let destination = path.with_file_name("missing-config.toml");
            std::fs::remove_file(&target).unwrap();
            std::os::unix::fs::symlink("missing-config.toml", &target).unwrap();
            second
                .save_comment_default(super::CommentDefault {
                    name: "chain".into(),
                    body: "Follow the whole link chain".into(),
                })
                .unwrap();
            assert!(path.is_symlink());
            assert!(target.is_symlink());
            assert!(destination.is_file());

            std::fs::remove_file(&destination).unwrap();
            std::os::unix::fs::symlink("config.toml", &destination).unwrap();
            assert!(
                second
                    .save_comment_default(super::CommentDefault {
                        name: "cycle".into(),
                        body: "Reject a symlink cycle".into(),
                    })
                    .is_err()
            );
            assert!(path.is_symlink());
            assert!(target.is_symlink());
            assert!(destination.is_symlink());
            std::fs::remove_file(&destination).unwrap();
        }

        std::fs::write(&path, "theme = \"").unwrap();
        assert!(
            second
                .save_comment_default(super::CommentDefault {
                    name: "invalid".into(),
                    body: "Do not overwrite malformed config".into(),
                })
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = \"");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_preset_writes_leave_config_available_for_retry() {
        const CONFIG_ENV: &str = "BLIPPY_TEST_PARTIAL_PRESET";
        const FAIL_ENV: &str = "BLIPPY_TEST_PARTIAL_PRESET_FAIL";
        let Some(config_dir) = std::env::var_os(CONFIG_ENV) else {
            let dir = tempfile::tempdir().unwrap();
            let config_dir = dir.path().join("blippy");
            std::fs::create_dir(&config_dir).unwrap();
            std::fs::write(
                config_dir.join("config.toml"),
                "theme = \"midnight\"\n[[comment_defaults]]\nname = \"existing\"\nbody = \"Keep me\"\n",
            )
            .unwrap();
            for fail in [true, false] {
                let executable = std::env::current_exe().unwrap();
                let mut command = if fail {
                    let mut command = std::process::Command::new("/bin/sh");
                    // Restrict only this child; a later child retries without the limit.
                    command
                        .args(["-c", "trap '' XFSZ; ulimit -f 1; exec \"$@\"", "sh"])
                        .arg(&executable);
                    command
                } else {
                    std::process::Command::new(executable)
                };
                let output = command
                    .args([
                        "--exact",
                        "config::tests::failed_preset_writes_leave_config_available_for_retry",
                        "--nocapture",
                    ])
                    .env(CONFIG_ENV, dir.path())
                    .env(FAIL_ENV, if fail { "1" } else { "0" })
                    .env("XDG_CONFIG_HOME", dir.path())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            assert_eq!(std::fs::read_dir(config_dir).unwrap().count(), 2);
            return;
        };
        let path = std::path::PathBuf::from(config_dir).join("blippy/config.toml");
        let before = std::fs::read(&path).unwrap();
        let mut config = Config::load().unwrap();
        let result = config.save_comment_default(super::CommentDefault {
            name: "new".into(),
            body: "x".repeat(16384),
        });
        if std::env::var(FAIL_ENV).unwrap() == "1" {
            let error = result.expect_err("force a partial write with the child file-size limit");
            eprintln!("{error:#}");
            assert!(error.to_string().starts_with("Failed to write config"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(config.comment_defaults.len(), 1);
            return;
        }
        result.expect("retry after removing the write restriction");
        let saved = Config::load().unwrap();
        assert_eq!(saved.theme.as_deref(), Some("midnight"));
        assert_eq!(saved.comment_defaults, config.comment_defaults);
        assert_eq!(saved.comment_defaults.len(), 2);
        assert_eq!(saved.comment_defaults[0].body, "Keep me");
        assert_eq!(saved.comment_defaults[1].body.len(), 16384);
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

fn resolve_config_path(mut path: PathBuf) -> Result<PathBuf> {
    for _ in 0..40 {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() => {
                let target = fs::read_link(&path)
                    .with_context(|| format!("Failed to resolve config at {}", path.display()))?;
                path = if target.is_absolute() {
                    target
                } else {
                    path.with_file_name(target)
                };
            }
            Ok(_) => {
                return path
                    .canonicalize()
                    .with_context(|| format!("Failed to resolve config at {}", path.display()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(path),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to resolve config at {}", path.display()));
            }
        }
    }
    anyhow::bail!(
        "Too many symlinks while resolving config at {}",
        path.display()
    )
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
