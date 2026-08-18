use anyhow::{Context, Result};

#[derive(Default)]
pub struct SystemClipboard {
    inner: Option<arboard::Clipboard>,
}

impl SystemClipboard {
    pub fn set_text(&mut self, text: &str) -> Result<()> {
        if self.inner.is_none() {
            self.inner = Some(arboard::Clipboard::new().context("clipboard unavailable")?);
        }

        self.inner
            .as_mut()
            .context("clipboard unavailable")?
            .set_text(text)
            .context("failed to write to clipboard")
    }
}
