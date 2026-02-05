use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    AuthReset,
    CacheReset,
}

pub fn parse_args(_args: &[String]) -> Result<Option<CliCommand>> {
    if _args.len() <= 1 {
        return Ok(None);
    }

    let command = _args.get(1).map(String::as_str);
    let subcommand = _args.get(2).map(String::as_str);

    if command == Some("auth") && subcommand == Some("reset") {
        return Ok(Some(CliCommand::AuthReset));
    }

    if command == Some("cache") && subcommand == Some("reset") {
        return Ok(Some(CliCommand::CacheReset));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliCommand};

    #[test]
    fn parse_args_returns_auth_reset() {
        let args = vec![
            "glyph".to_string(),
            "auth".to_string(),
            "reset".to_string(),
        ];

        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::AuthReset));
    }

    #[test]
    fn parse_args_returns_none_for_empty() {
        let args = vec!["glyph".to_string()];
        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, None);
    }

    #[test]
    fn parse_args_returns_cache_reset() {
        let args = vec![
            "glyph".to_string(),
            "cache".to_string(),
            "reset".to_string(),
        ];

        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::CacheReset));
    }
}
