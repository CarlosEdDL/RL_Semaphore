//! `rl-semaphore` command-line entry point.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Train and evaluate RL agents that control simulated traffic lights.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!("rl-semaphore {}: not implemented yet", cli.command.name());
    ExitCode::FAILURE
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
    fn missing_subcommand_is_an_error() {
        assert!(Cli::try_parse_from(["rl-semaphore"]).is_err());
    }
}
