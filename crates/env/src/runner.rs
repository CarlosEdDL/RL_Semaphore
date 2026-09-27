//! The episode runner: one loop for the CLI, the server and the evaluations.

use rl_semaphore_sim::{
    Demand, EpisodeMetrics, EpisodeSummary, MetricsError, Scenario, Simulation, SpawnError,
    StepOutcome,
};

use crate::controller::Controller;

/// Why an episode could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EnvError {
    /// A generated arrival could not be spawned.
    #[error("cannot spawn an arrival: {0}")]
    Spawn(#[from] SpawnError),
    /// The metrics collector refused an observation.
    #[error("cannot record metrics: {0}")]
    Metrics(#[from] MetricsError),
}

/// How the signal treated the commands of an episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SignalCounts {
    /// Steps in which the signal accepted a switch command.
    pub switches_started: u64,
    /// Steps in which the signal started a switch itself to meet max-red.
    pub switches_forced: u64,
    /// Steps in which the signal refused a command.
    pub commands_ignored: u64,
}

/// The result of [`run_episode`].
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeReport {
    /// The metrics of the episode, taken after the last step.
    pub summary: EpisodeSummary,
    /// What the signal did with the controller's commands.
    pub signal: SignalCounts,
}

/// Runs one episode of `steps` steps on a fresh simulation of `scenario`.
///
/// Each step, in this order: draw the arrivals and spawn each one; ask
/// `controller` for a command (so it sees this step's arrivals); step the
/// simulation with it; and observe the step report with the metrics collector.
/// Arrivals are drawn from a `Demand` seeded with `seed`, so the same scenario,
/// controller and seed always give the same report.
///
/// This function does not log, read the clock or do I/O.
///
/// # Errors
///
/// Returns [`EnvError`] if an arrival cannot be spawned or the metrics collector
/// refuses an observation. Neither can happen for a validated scenario.
///
/// # Examples
///
/// ```
/// use rl_semaphore_env::{FixedTime, run_episode};
/// use rl_semaphore_sim::Scenario;
///
/// let text = include_str!("../../../configs/single-intersection.toml");
/// let scenario = Scenario::from_toml_str(text).unwrap();
/// let plan = scenario.fixed_time().unwrap();
/// let mut controller = FixedTime::new(plan);
/// let report = run_episode(&scenario, &mut controller, 42, 300).unwrap();
/// assert_eq!(report.summary.steps, 300);
/// assert_eq!(report.signal.switches_forced, 0);
/// ```
pub fn run_episode(
    scenario: &Scenario,
    controller: &mut impl Controller,
    seed: u64,
    steps: u64,
) -> Result<EpisodeReport, EnvError> {
    let mut demand = Demand::new(scenario, seed);
    let mut sim = Simulation::new(scenario.clone());
    let mut metrics = EpisodeMetrics::new(&sim);
    let mut signal = SignalCounts::default();
    for _ in 0..steps {
        for arrival in demand.arrivals() {
            sim.spawn(arrival.approach, arrival.movement)?;
        }
        let command = controller.command(&sim);
        let report = sim.step(command);
        match report.signal {
            StepOutcome::Held => {}
            StepOutcome::SwitchStarted(_) => signal.switches_started += 1,
            StepOutcome::SwitchForced { .. } => signal.switches_forced += 1,
            StepOutcome::Ignored(_) => signal.commands_ignored += 1,
        }
        metrics.observe(&sim, &report)?;
    }
    Ok(EpisodeReport {
        summary: metrics.summary(&sim),
        signal,
    })
}
