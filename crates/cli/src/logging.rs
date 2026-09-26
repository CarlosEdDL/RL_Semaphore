//! Subscriber setup for the `rl-semaphore` binary. Logs always go to stderr.

use std::io::IsTerminal;

use anyhow::Context;
use clap::ValueEnum;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// Output format of log events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogFormat {
    /// Human-readable lines, with colors when stderr is a terminal.
    Pretty,
    /// One JSON object per line.
    Json,
}

/// Maps the `-v` and `-q` counts to a level. The default is `info`, and the result saturates at
/// `trace` and off.
pub fn level_from_verbosity(verbose: u8, quiet: u8) -> LevelFilter {
    match i16::from(verbose) - i16::from(quiet) {
        i16::MIN..=-3 => LevelFilter::OFF,
        -2 => LevelFilter::ERROR,
        -1 => LevelFilter::WARN,
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        2..=i16::MAX => LevelFilter::TRACE,
    }
}

/// Installs the global subscriber.
///
/// `RUST_LOG` wins when set and non-empty. If it is invalid, `level` is used instead and a `warn`
/// event is emitted once the subscriber exists.
pub fn init(format: LogFormat, level: LevelFilter) -> anyhow::Result<()> {
    let mut ignored_rust_log = None;
    let filter = match std::env::var("RUST_LOG") {
        Ok(directives) if !directives.trim().is_empty() => match EnvFilter::try_new(&directives) {
            Ok(filter) => filter,
            Err(err) => {
                ignored_rust_log = Some((directives, err));
                EnvFilter::default().add_directive(level.into())
            }
        },
        _ => EnvFilter::default().add_directive(level.into()),
    };

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr);
    let result = match format {
        LogFormat::Pretty => builder
            .with_ansi(std::io::stderr().is_terminal())
            .try_init(),
        LogFormat::Json => builder.json().try_init(),
    };
    result
        .map_err(|err| anyhow::anyhow!(err))
        .context("failed to initialize logging")?;

    if let Some((directives, err)) = ignored_rust_log {
        tracing::warn!(rust_log = %directives, error = %err, "ignoring invalid RUST_LOG");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_info() {
        assert_eq!(level_from_verbosity(0, 0), LevelFilter::INFO);
    }

    #[test]
    fn each_verbose_step() {
        assert_eq!(level_from_verbosity(1, 0), LevelFilter::DEBUG);
        assert_eq!(level_from_verbosity(2, 0), LevelFilter::TRACE);
    }

    #[test]
    fn each_quiet_step() {
        assert_eq!(level_from_verbosity(0, 1), LevelFilter::WARN);
        assert_eq!(level_from_verbosity(0, 2), LevelFilter::ERROR);
        assert_eq!(level_from_verbosity(0, 3), LevelFilter::OFF);
    }

    #[test]
    fn saturates() {
        assert_eq!(level_from_verbosity(9, 0), LevelFilter::TRACE);
        assert_eq!(level_from_verbosity(0, 9), LevelFilter::OFF);
        assert_eq!(level_from_verbosity(u8::MAX, 0), LevelFilter::TRACE);
        assert_eq!(level_from_verbosity(0, u8::MAX), LevelFilter::OFF);
    }

    #[test]
    fn counts_net_out() {
        assert_eq!(level_from_verbosity(1, 1), LevelFilter::INFO);
        assert_eq!(level_from_verbosity(3, 2), LevelFilter::DEBUG);
        assert_eq!(level_from_verbosity(1, 2), LevelFilter::WARN);
    }
}
