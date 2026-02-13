use std::io;
use std::process::Command;

use anyhow::{Context, Result};

pub trait AuthSources {
    fn gh_token(&self) -> Result<Option<String>>;
    fn keyring_token(&self) -> Result<Option<String>>;
    fn prompt_token(&self) -> Result<String>;
    fn store_token(&self, token: &str) -> Result<()>;
    fn clear_token(&self) -> Result<bool>;
}

const DEFAULT_HOST: &str = "github.com";
const DEFAULT_SERVICE: &str = "blippy";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Gh,
    Keyring,
    Prompt,
}

impl AuthMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Gh => "gh",
            Self::Keyring => "keyring",
            Self::Prompt => "prompt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken {
    pub value: String,
    pub method: AuthMethod,
}

pub fn clear_auth_token<S: AuthSources>(sources: &S) -> Result<bool> {
    sources.clear_token()
}

pub fn resolve_auth_token<S: AuthSources>(sources: &S) -> Result<AuthToken> {
    let token = sources.gh_token()?;
    if let Some(value) = token {
        return Ok(AuthToken {
            value,
            method: AuthMethod::Gh,
        });
    }

    let token = sources.keyring_token()?;
    if let Some(value) = token {
        return Ok(AuthToken {
            value,
            method: AuthMethod::Keyring,
        });
    }

    let token = sources.prompt_token()?;
    sources.store_token(&token)?;
    Ok(AuthToken {
        value: token,
        method: AuthMethod::Prompt,
    })
}

pub struct SystemAuth;

impl SystemAuth {
    pub fn new() -> Self {
        Self
    }

    fn keyring_entry(&self) -> Result<keyring::Entry> {
        let entry = keyring::Entry::new(DEFAULT_SERVICE, DEFAULT_HOST)
            .with_context(|| "Failed to initialize keyring entry")?;
        Ok(entry)
    }
}

impl AuthSources for SystemAuth {
    fn gh_token(&self) -> Result<Option<String>> {
        let output = Command::new("gh")
            .args(["auth", "token", "--hostname", DEFAULT_HOST])
            .output();

        let output = match output {
            Ok(output) => output,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(normalize_token(&stdout))
    }

    fn keyring_token(&self) -> Result<Option<String>> {
        let entry = self.keyring_entry()?;
        let token = match entry.get_password() {
            Ok(token) => token,
            Err(error) => {
                if matches!(error, keyring::Error::NoEntry) {
                    return Ok(None);
                }
                return Err(error.into());
            }
        };

        Ok(normalize_token(&token))
    }

    fn prompt_token(&self) -> Result<String> {
        let prompt = format!(
            "Paste a GitHub Personal Access Token for {}: ",
            DEFAULT_HOST
        );
        let raw = rpassword::prompt_password(prompt)?;
        normalize_token(&raw).context("Token cannot be empty")
    }

    fn store_token(&self, token: &str) -> Result<()> {
        let entry = self.keyring_entry()?;
        entry.set_password(token)?;
        Ok(())
    }

    fn clear_token(&self) -> Result<bool> {
        let entry = self.keyring_entry()?;
        match entry.delete_password() {
            Ok(()) => Ok(true),
            Err(error) => {
                if matches!(error, keyring::Error::NoEntry) {
                    return Ok(false);
                }
                Err(error.into())
            }
        }
    }
}

fn normalize_token(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{AuthMethod, AuthSources, resolve_auth_token};

    #[test]
    fn resolve_token_prefers_gh_token() {
        let sources = TestSources::new().with_gh("gh-token");
        let token = resolve_auth_token(&sources).expect("token resolves");

        assert_eq!(token.value, "gh-token");
        assert_eq!(token.method, AuthMethod::Gh);
        assert_eq!(sources.calls(), vec!["gh"]);
        assert!(sources.stored().is_empty());
    }

    #[test]
    fn resolve_token_uses_keyring_when_gh_missing() {
        let sources = TestSources::new().with_keyring("keyring-token");
        let token = resolve_auth_token(&sources).expect("token resolves");

        assert_eq!(token.value, "keyring-token");
        assert_eq!(token.method, AuthMethod::Keyring);
        assert_eq!(sources.calls(), vec!["gh", "keyring"]);
        assert!(sources.stored().is_empty());
    }

    #[test]
    fn resolve_token_prompts_and_stores_when_missing() {
        let sources = TestSources::new().with_prompt("prompt-token");
        let token = resolve_auth_token(&sources).expect("token resolves");

        assert_eq!(token.value, "prompt-token");
        assert_eq!(token.method, AuthMethod::Prompt);
        assert_eq!(sources.calls(), vec!["gh", "keyring", "prompt", "store"]);
        assert_eq!(sources.stored(), vec!["prompt-token".to_string()]);
    }

    #[test]
    fn normalize_token_trims_and_rejects_empty() {
        assert_eq!(super::normalize_token("  abc\n"), Some("abc".to_string()));
        assert_eq!(super::normalize_token("  \n"), None);
    }

    #[test]
    fn clear_auth_token_clears_stored_token() {
        let sources = TestSources::new().with_clear(true);
        let cleared = super::clear_auth_token(&sources).expect("clear succeeds");

        assert!(cleared);
        assert_eq!(sources.calls(), vec!["clear"]);
    }

    struct TestSources {
        gh: Option<String>,
        keyring: Option<String>,
        prompt: Option<String>,
        clear_result: bool,
        calls: RefCell<Vec<&'static str>>,
        stored: RefCell<Vec<String>>,
    }

    impl TestSources {
        fn new() -> Self {
            Self {
                gh: None,
                keyring: None,
                prompt: None,
                clear_result: false,
                calls: RefCell::new(Vec::new()),
                stored: RefCell::new(Vec::new()),
            }
        }

        fn with_gh(mut self, value: &str) -> Self {
            self.gh = Some(value.to_string());
            self
        }

        fn with_keyring(mut self, value: &str) -> Self {
            self.keyring = Some(value.to_string());
            self
        }

        fn with_prompt(mut self, value: &str) -> Self {
            self.prompt = Some(value.to_string());
            self
        }

        fn with_clear(mut self, value: bool) -> Self {
            self.clear_result = value;
            self
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }

        fn stored(&self) -> Vec<String> {
            self.stored.borrow().clone()
        }
    }

    impl AuthSources for TestSources {
        fn gh_token(&self) -> anyhow::Result<Option<String>> {
            self.calls.borrow_mut().push("gh");
            Ok(self.gh.clone())
        }

        fn keyring_token(&self) -> anyhow::Result<Option<String>> {
            self.calls.borrow_mut().push("keyring");
            Ok(self.keyring.clone())
        }

        fn prompt_token(&self) -> anyhow::Result<String> {
            self.calls.borrow_mut().push("prompt");
            Ok(self
                .prompt
                .clone()
                .unwrap_or_else(|| "prompt-token".to_string()))
        }

        fn store_token(&self, token: &str) -> anyhow::Result<()> {
            self.calls.borrow_mut().push("store");
            self.stored.borrow_mut().push(token.to_string());
            Ok(())
        }

        fn clear_token(&self) -> anyhow::Result<bool> {
            self.calls.borrow_mut().push("clear");
            Ok(self.clear_result)
        }
    }
}
