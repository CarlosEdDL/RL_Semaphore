//! `rl-semaphore` command-line entry point.

mod logging;
mod report;
mod serve;
mod simulate;

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};

use crate::logging::LogFormat;
use crate::serve::ServeArgs;
use crate::simulate::SimulateArgs;

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
    Simulate(SimulateArgs),
    /// Train an RL agent from a config file
    Train,
    /// Evaluate a policy or baseline over several seeds
    Eval,
    /// Start the Axum server and stream a live, looping fixed-time simulation
    Serve(ServeArgs),
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Simulate(_) => "simulate",
            Self::Train => "train",
            Self::Eval => "eval",
            Self::Serve(_) => "serve",
        }
    }
}

fn run(cli: &Cli) -> anyhow::Result<()> {
    match &cli.command {
        Command::Simulate(args) => simulate::run(args),
        Command::Serve(args) => serve::run(args),
        Command::Train | Command::Eval => {
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
            (vec!["simulate", "--config", "x.toml"], "simulate"),
            (vec!["train"], "train"),
            (vec!["eval"], "eval"),
            (vec!["serve", "--config", "x.toml"], "serve"),
        ];
        for (args, name) in cases {
            let cli = Cli::try_parse_from(["rl-semaphore"].into_iter().chain(args)).unwrap();
            assert_eq!(cli.command.name(), name);
        }
    }

    #[test]
    fn log_format_defaults_to_pretty() {
        let cli = Cli::try_parse_from(["rl-semaphore", "train"]).unwrap();
        assert_eq!(cli.log_format, LogFormat::Pretty);
    }

    #[test]
    fn parses_log_format_before_and_after_subcommand() {
        for args in [
            ["rl-semaphore", "--log-format", "json", "train"],
            ["rl-semaphore", "train", "--log-format", "json"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(cli.log_format, LogFormat::Json);
        }
    }

    #[test]
    fn rejects_unknown_log_format() {
        assert!(Cli::try_parse_from(["rl-semaphore", "--log-format", "xml", "train"]).is_err());
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
            let mut args = vec!["rl-semaphore", "train"];
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

    fn parse_serve(args: &[&str]) -> ServeArgs {
        match Cli::try_parse_from(["rl-semaphore"].into_iter().chain(args.iter().copied()))
            .unwrap()
            .command
        {
            Command::Serve(args) => args,
            other => panic!("expected Serve, got {other:?}"),
        }
    }

    #[test]
    fn serve_config_is_required() {
        assert!(Cli::try_parse_from(["rl-semaphore", "serve"]).is_err());
    }

    #[test]
    fn serve_defaults() {
        let args = parse_serve(&["serve", "--config", "x.toml"]);
        assert_eq!(args.config, "x.toml");
        assert_eq!(args.seed, 0);
        assert_eq!(args.steps.get(), 3600);
        assert!((args.speed - 1.0).abs() < f64::EPSILON);
        assert_eq!(args.metrics_every.get(), 5);
        assert_eq!(args.bind, "127.0.0.1:3000".parse().unwrap());
    }

    #[test]
    fn serve_parses_every_flag() {
        let args = parse_serve(&[
            "serve",
            "--config",
            "x.toml",
            "--seed",
            "42",
            "--steps",
            "100",
            "--speed",
            "10.5",
            "--metrics-every",
            "7",
            "--bind",
            "0.0.0.0:8080",
        ]);
        assert_eq!(args.seed, 42);
        assert_eq!(args.steps.get(), 100);
        assert!((args.speed - 10.5).abs() < f64::EPSILON);
        assert_eq!(args.metrics_every.get(), 7);
        assert_eq!(args.bind, "0.0.0.0:8080".parse().unwrap());
    }

    #[test]
    fn serve_rejects_invalid_speeds() {
        for speed in ["0", "-1"] {
            assert!(
                Cli::try_parse_from([
                    "rl-semaphore",
                    "serve",
                    "--config",
                    "x.toml",
                    "--speed",
                    speed
                ])
                .is_err(),
                "speed={speed}"
            );
        }
    }

    #[test]
    fn serve_rejects_zero_steps() {
        assert!(
            Cli::try_parse_from([
                "rl-semaphore",
                "serve",
                "--config",
                "x.toml",
                "--steps",
                "0"
            ])
            .is_err()
        );
    }

    #[test]
    fn serve_rejects_an_invalid_bind_address() {
        assert!(
            Cli::try_parse_from([
                "rl-semaphore",
                "serve",
                "--config",
                "x.toml",
                "--bind",
                "not-an-address"
            ])
            .is_err()
        );
    }
}
