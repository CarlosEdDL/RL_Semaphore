//! The [`Controller`] trait.

use rl_semaphore_sim::{Command, Simulation};

/// Decides what the signal does at each step.
///
/// [`run_episode`](crate::run_episode) calls [`command`](Self::command) once per step,
/// after that step's arrivals have been spawned and before [`Simulation::step`], so the
/// controller sees the vehicles that arrived in this step.
pub trait Controller {
    /// The command to apply in the next step of `sim`.
    fn command(&mut self, sim: &Simulation) -> Command;

    /// A short name for logs and output.
    fn name(&self) -> &str;
}
