use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub keymap: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config at {}", path.display()))?;
        Ok(config)
    }
}

fn config_path() -> PathBuf {
    config_dir().join("glyph").join("config.toml")
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
