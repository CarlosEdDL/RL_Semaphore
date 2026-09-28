//! The `serve` command: run the Axum server on a live, looping fixed-time simulation.

use std::fs;
use std::net::SocketAddr;
use std::num::NonZeroU64;

use anyhow::Context;
use clap::Args;
use rl_semaphore_server::{Pace, ServerConfig};
use rl_semaphore_sim::Scenario;

/// Rejects a speed that is not finite, not greater than 0, or greater than 1000.
fn parse_speed(s: &str) -> Result<f64, String> {
    let speed: f64 = s.parse().map_err(|_| format!("invalid speed: {s}"))?;
    if !speed.is_finite() || speed <= 0.0 || speed > 1000.0 {
        return Err(format!(
            "speed must be finite, greater than 0 and at most 1000, got {speed}"
        ));
    }
    Ok(speed)
}

/// Arguments of `serve`.
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Scenario TOML file (must contain a `[fixed_time]` table)
    #[arg(long, value_name = "PATH", env = "RL_SEMAPHORE_CONFIG")]
    pub config: String,

    /// Demand seed of episode 0
    #[arg(long, default_value_t = 0, env = "RL_SEMAPHORE_SEED")]
    pub seed: u64,

    /// Number of simulation steps per episode
    #[arg(long, default_value = "3600", env = "RL_SEMAPHORE_STEPS")]
    pub steps: NonZeroU64,

    /// Wall-clock speed factor (finite, greater than 0, at most 1000)
    #[arg(long, default_value_t = 1.0, value_parser = parse_speed, env = "RL_SEMAPHORE_SPEED")]
    pub speed: f64,

    /// Steps between `Metrics` messages
    #[arg(long, default_value = "5", env = "RL_SEMAPHORE_METRICS_EVERY")]
    pub metrics_every: NonZeroU64,

    /// Address to bind the HTTP/WebSocket server to
    #[arg(long, default_value = "127.0.0.1:3000", env = "RL_SEMAPHORE_BIND")]
    pub bind: SocketAddr,
}

/// Waits for Ctrl-C, and on Unix also for SIGTERM (R8.4).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "cannot install a SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

/// Runs the command: builds a Tokio runtime and blocks on the server until shutdown.
pub fn run(args: &ServeArgs) -> anyhow::Result<()> {
    let text = fs::read_to_string(&args.config)
        .with_context(|| format!("cannot read config file {}", args.config))?;
    let scenario = Scenario::from_toml_str(&text)
        .with_context(|| format!("invalid scenario in {}", args.config))?;
    scenario.fixed_time().with_context(|| {
        format!(
            "{} has no [fixed_time] table, which `serve` requires",
            args.config
        )
    })?;

    tracing::info!(
        config = args.config,
        seed = args.seed,
        steps = args.steps.get(),
        speed = args.speed,
        metrics_every = args.metrics_every.get(),
        bind = %args.bind,
        "serve started"
    );

    let mut config = ServerConfig::new(scenario, args.bind);
    config.seed = args.seed;
    config.steps_per_episode = args.steps;
    config.metrics_every = args.metrics_every;
    config.pace = Pace::RealTime { speed: args.speed };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot build the tokio runtime")?;
    runtime
        .block_on(rl_semaphore_server::run(config, shutdown_signal()))
        .context("the server failed")
}
