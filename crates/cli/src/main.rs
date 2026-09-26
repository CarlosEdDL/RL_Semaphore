//! `rl-semaphore` command-line entry point.

mod logging;

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};

use crate::logging::LogFormat;

/// Train and evaluate RL agents that control simulated traffic lights.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Format of log output on stderr
    #[arg(
        long,
        global = true,
        value_enum,
        env = "RL_SEMAPHORE_LOG_FORMAT",
        default_value = "pretty"
    )]
    log_format: LogFormat,

    /// Log more (`-v` for debug, `-vv` for trace). Ignored when RUST_LOG is set
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,

    /// Log less (`-q` for warn, `-qq` for error, `-qqq` for off). Ignored when RUST_LOG is set
    #[arg(short, long, global = true, action = ArgAction::Count)]
    quiet: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the traffic simulator with a baseline controller and print metrics
    Simulate,
    /// Train an RL agent from a config file
    Train,
    /// Evaluate a policy or baseline over several seeds
    Eval,
    /// Start the web server and dashboard
    Serve,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Simulate => "simulate",
            Self::Train => "train",
            Self::Eval => "eval",
            Self::Serve => "serve",
        }
    }
}

fn run(cli: &Cli) -> anyhow::Result<()> {
    match &cli.command {
        Command::Simulate | Command::Train | Command::Eval | Command::Serve => {
            anyhow::bail!("{}: not implemented yet", cli.command.name())
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let level = logging::level_from_verbosity(cli.verbose, cli.quiet);
    if let Err(err) = logging::init(cli.log_format, level) {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }
    tracing::debug!(
        version = env!("CARGO_PKG_VERSION"),
        command = cli.command.name(),
        "starting"
    );
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = format!("{err:#}"), "command failed");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_each_subcommand() {
        let cases = [
            ("simulate", "simulate"),
            ("train", "train"),
            ("eval", "eval"),
            ("serve", "serve"),
        ];
        for (arg, name) in cases {
            let cli = Cli::try_parse_from(["rl-semaphore", arg]).unwrap();
            assert_eq!(cli.command.name(), name);
        }
    }

    #[test]
    fn log_format_defaults_to_pretty() {
        let cli = Cli::try_parse_from(["rl-semaphore", "simulate"]).unwrap();
        assert_eq!(cli.log_format, LogFormat::Pretty);
    }

    #[test]
    fn parses_log_format_before_and_after_subcommand() {
        for args in [
            ["rl-semaphore", "--log-format", "json", "simulate"],
            ["rl-semaphore", "simulate", "--log-format", "json"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(cli.log_format, LogFormat::Json);
        }
    }

    #[test]
    fn rejects_unknown_log_format() {
        assert!(Cli::try_parse_from(["rl-semaphore", "--log-format", "xml", "simulate"]).is_err());
    }

    #[test]
    fn parses_verbosity_flags() {
        let cases = [
            (vec!["-v"], (1, 0)),
            (vec!["-vv"], (2, 0)),
            (vec!["-q"], (0, 1)),
            (vec!["-v", "-q"], (1, 1)),
            (vec!["--verbose", "--quiet", "--quiet"], (1, 2)),
        ];
        for (flags, expected) in cases {
            let mut args = vec!["rl-semaphore", "simulate"];
            args.extend(flags);
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!((cli.verbose, cli.quiet), expected);
        }
    }

    #[test]
    fn unimplemented_command_fails_with_its_name() {
        let cli = Cli::try_parse_from(["rl-semaphore", "train"]).unwrap();
        let err = run(&cli).unwrap_err();
        assert!(err.to_string().contains("train: not implemented yet"));
    }

    #[test]
    fn missing_subcommand_is_an_error() {
        assert!(Cli::try_parse_from(["rl-semaphore"]).is_err());
    }
}
