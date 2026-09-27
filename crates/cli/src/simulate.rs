//! The `simulate` command: run the fixed-time baseline on a scenario and print its metrics.

use std::fs;
use std::io::Write;
use std::time::Instant;

use anyhow::Context;
use clap::{Args, ValueEnum};
use rl_semaphore_env::{Controller, FixedTime, run_episode};
use rl_semaphore_sim::Scenario;

use crate::report::{self, RunInfo};

/// How the result is printed on stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// A readable table
    Text,
    /// One JSON document
    Json,
}

/// Arguments of `simulate`.
#[derive(Debug, Args)]
pub struct SimulateArgs {
    /// Scenario TOML file (must contain a `[fixed_time]` table)
    #[arg(long, value_name = "PATH")]
    pub config: String,

    /// Number of simulation steps to run
    #[arg(long, default_value_t = 3600, value_parser = clap::value_parser!(u64).range(1..))]
    pub steps: u64,

    /// Seed of the demand generator
    #[arg(long, default_value_t = 0)]
    pub seed: u64,

    /// Format of the result on stdout
    #[arg(long, value_enum, default_value = "text")]
    pub output: OutputFormat,
}

/// Runs the command and prints the result on stdout.
pub fn run(args: &SimulateArgs) -> anyhow::Result<()> {
    let text = fs::read_to_string(&args.config)
        .with_context(|| format!("cannot read config file {}", args.config))?;
    let scenario = Scenario::from_toml_str(&text)
        .with_context(|| format!("invalid scenario in {}", args.config))?;
    let plan = scenario.fixed_time().with_context(|| {
        format!(
            "{} has no [fixed_time] table, which `simulate` requires",
            args.config
        )
    })?;
    let mut controller = FixedTime::new(plan);

    tracing::info!(
        config = args.config,
        seed = args.seed,
        steps = args.steps,
        "simulation started"
    );
    let started = Instant::now();
    let result = run_episode(&scenario, &mut controller, args.seed, args.steps)
        .context("the episode failed")?;
    let elapsed = started.elapsed().as_secs_f64();
    #[allow(clippy::cast_precision_loss)]
    // INVARIANT: precision only degrades above 2^53 steps, far beyond any run.
    let steps_per_s = if elapsed > 0.0 {
        args.steps as f64 / elapsed
    } else {
        f64::INFINITY
    };
    tracing::info!(elapsed_s = elapsed, steps_per_s, "simulation finished");

    let info = RunInfo {
        config: &args.config,
        seed: args.seed,
        steps: args.steps,
        controller: controller.name(),
        scenario: &scenario,
        plan,
    };
    // Build the whole output first, so an error never leaves half a table on stdout.
    let output = match args.output {
        OutputFormat::Text => report::text(&info, &result),
        OutputFormat::Json => report::json(&info, &result).context("cannot encode the result")?,
    };
    std::io::stdout()
        .lock()
        .write_all(output.as_bytes())
        .context("cannot write to stdout")
}
