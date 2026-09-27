//! The fixed-time controller.

use rl_semaphore_sim::{Command, FixedTimePlan, PhaseId, SignalState, Simulation};

use crate::controller::Controller;

/// Cycles through the phases in list order, holding each green for its planned time.
///
/// For a plan that passed validation (`cycle - green[j] <= max_red` for every phase),
/// phase `p` is green for exactly `green[p]` consecutive entries each time it is
/// served, phases are served in list order starting with phase 0 at entry 0, and
/// each full cycle lasts exactly `Σ (green[i] + yellow + all_red)` steps. Every step
/// outcome is `Held` or `SwitchStarted`: the signal never has to force a switch or
/// ignore a command, so the run measures the plan and not the plan plus corrections.
///
/// The controller keeps no clock of its own. It reads the signal state at every step:
/// while phase `p` has been green for fewer than `green[p]` entries it holds, and
/// when it reaches `green[p]` it asks for the next phase. During yellow and all-red it
/// holds. That makes it correct even when attached to a simulation that is already
/// running, and it cannot drift from the signal.
#[derive(Debug, Clone)]
pub struct FixedTime {
    green: Vec<u32>,
}

impl FixedTime {
    /// Builds the controller from a validated plan.
    #[must_use]
    pub fn new(plan: &FixedTimePlan) -> Self {
        Self {
            green: plan.green_steps().to_vec(),
        }
    }
}

impl Controller for FixedTime {
    fn command(&mut self, sim: &Simulation) -> Command {
        let n = self.green.len();
        match sim.signal().state() {
            SignalState::Green { phase, elapsed }
                if n >= 2 && self.green.get(phase.index()).is_some_and(|&g| elapsed >= g) =>
            {
                Command::SwitchTo(PhaseId::new((phase.index() + 1) % n))
            }
            _ => Command::Hold,
        }
    }

    fn name(&self) -> &'static str {
        "fixed_time"
    }
}
