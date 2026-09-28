//! [`Episode`], a step-by-step driver of one episode, shared by [`crate::run_episode`] and the
//! server (2.2).

use rl_semaphore_sim::{Demand, EpisodeMetrics, EpisodeSummary, Scenario, Simulation, StepOutcome};

use crate::controller::Controller;
use crate::runner::{EnvError, EpisodeReport, SignalCounts};

/// Drives one episode step by step: a simulation, its demand, its metrics collector and its
/// signal counts, with no controller of its own.
///
/// [`step`](Self::step) does one iteration of the loop [`crate::run_episode`] runs in full: draw
/// and spawn the arrivals, ask the controller for a command (so it sees this step's arrivals),
/// step the simulation, count the signal outcome, then observe the metrics. `run_episode` is
/// `Episode::new`, then `steps` calls to `step`, then [`report`](Self::report): there is exactly
/// one copy of this loop in the workspace.
///
/// Like `run_episode`, `Episode` does not log, read the clock or do I/O.
#[derive(Debug)]
pub struct Episode {
    sim: Simulation,
    demand: Demand,
    metrics: EpisodeMetrics,
    signal: SignalCounts,
    seed: u64,
}

impl Episode {
    /// Starts a fresh episode at step 0: a new [`Simulation`] of `scenario`, and a [`Demand`]
    /// seeded with `seed`.
    #[must_use]
    pub fn new(scenario: &Scenario, seed: u64) -> Self {
        let demand = Demand::new(scenario, seed);
        let sim = Simulation::new(scenario.clone());
        let metrics = EpisodeMetrics::new(&sim);
        Self {
            sim,
            demand,
            metrics,
            signal: SignalCounts::default(),
            seed,
        }
    }

    /// Runs one step: draws and spawns this step's arrivals, asks `controller` for a command,
    /// steps the simulation, counts the signal's outcome, and observes the metrics.
    ///
    /// # Errors
    ///
    /// Returns [`EnvError`] if an arrival cannot be spawned or the metrics collector refuses the
    /// observation. Neither can happen for a validated scenario.
    pub fn step(&mut self, controller: &mut impl Controller) -> Result<(), EnvError> {
        for arrival in self.demand.arrivals() {
            self.sim.spawn(arrival.approach, arrival.movement)?;
        }
        let command = controller.command(&self.sim);
        let report = self.sim.step(command);
        match report.signal {
            StepOutcome::Held => {}
            StepOutcome::SwitchStarted(_) => self.signal.switches_started += 1,
            StepOutcome::SwitchForced { .. } => self.signal.switches_forced += 1,
            StepOutcome::Ignored(_) => self.signal.commands_ignored += 1,
        }
        self.metrics.observe(&self.sim, &report)?;
        Ok(())
    }

    /// The episode's simulation, at its current step.
    #[must_use]
    pub const fn sim(&self) -> &Simulation {
        &self.sim
    }

    /// The seed the episode's demand was created with.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// How the signal has treated the controller's commands so far.
    #[must_use]
    pub const fn signal_counts(&self) -> SignalCounts {
        self.signal
    }

    /// The metrics collector's summary at the current step.
    #[must_use]
    pub fn summary(&self) -> EpisodeSummary {
        self.metrics.summary(&self.sim)
    }

    /// The report [`crate::run_episode`] would return after the same steps.
    #[must_use]
    pub fn report(&self) -> EpisodeReport {
        EpisodeReport {
            summary: self.summary(),
            signal: self.signal,
        }
    }
}
