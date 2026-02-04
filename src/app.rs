use crossterm::event::{KeyCode, KeyEvent};

use crate::config::Config;

pub struct App {
    should_quit: bool,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            should_quit: false,
            config,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
}
