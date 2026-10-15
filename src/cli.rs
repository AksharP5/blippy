use anyhow::{Result, bail};

pub const USAGE: &str = "Usage: blippy [COMMAND]\n\nCommands:\n  sync         Scan local repos and cache GitHub remotes\n  auth reset   Remove the stored auth token\n  cache reset  Remove the local cache\n  -h, --help   Show this help\n  -V, --version\n               Show version information";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    AuthReset,
    CacheReset,
    Help,
    Sync,
    Version,
}

pub fn parse_args(args: &[String]) -> Result<Option<CliCommand>> {
    let command = args.iter().skip(1).map(String::as_str).collect::<Vec<_>>();
    let parsed = match command.as_slice() {
        [] => None,
        ["--version"] | ["-V"] => Some(CliCommand::Version),
        ["--help"] | ["-h"] | ["help"] => Some(CliCommand::Help),
        ["auth", "reset"] => Some(CliCommand::AuthReset),
        ["cache", "reset"] => Some(CliCommand::CacheReset),
        ["sync"] => Some(CliCommand::Sync),
        _ => bail!("Unknown command: {}\n\n{}", command.join(" "), USAGE),
    };
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::{CliCommand, parse_args};

    #[test]
    fn parse_args_returns_auth_reset() {
        let args = vec![
            "blippy".to_string(),
            "auth".to_string(),
            "reset".to_string(),
        ];

        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::AuthReset));
    }

    #[test]
    fn parse_args_returns_none_for_empty() {
        let args = vec!["blippy".to_string()];
        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, None);
    }

    #[test]
    fn parse_args_returns_cache_reset() {
        let args = vec![
            "blippy".to_string(),
            "cache".to_string(),
            "reset".to_string(),
        ];

        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::CacheReset));
    }

    #[test]
    fn parse_args_returns_sync() {
        let args = vec!["blippy".to_string(), "sync".to_string()];
        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::Sync));
    }

    #[test]
    fn parse_args_returns_version() {
        let args = vec!["blippy".to_string(), "--version".to_string()];
        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::Version));
    }

    #[test]
    fn parse_args_returns_short_version() {
        let args = vec!["blippy".to_string(), "-V".to_string()];
        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::Version));
    }

    #[test]
    fn parse_args_returns_help() {
        let args = vec!["blippy".to_string(), "--help".to_string()];
        let parsed = parse_args(&args).expect("parse succeeds");
        assert_eq!(parsed, Some(CliCommand::Help));
    }

    #[test]
    fn parse_args_rejects_unknown_commands() {
        let args = vec!["blippy".to_string(), "typo".to_string()];
        let error = parse_args(&args).expect_err("unknown command fails");
        assert!(error.to_string().contains("Unknown command: typo"));
    }

    #[test]
    fn parse_args_rejects_extra_arguments() {
        let args = vec![
            "blippy".to_string(),
            "cache".to_string(),
            "reset".to_string(),
            "extra".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }
}
