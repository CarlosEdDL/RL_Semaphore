//! Property tests: for any sequence of spawns and signal commands, vehicles are
//! conserved, never share a cell, never overtake, and only cross on green.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use common::{EXAMPLE, Snapshot, allowed_movements, check_step, shared_lane};
use proptest::prelude::*;
use rl_semaphore_sim::{Command, PhaseId, Scenario, Simulation};

const STEPS: usize = 1000;

/// One step of a script: an optional index into the allowed movements to spawn, then a
/// command (`None` holds, `Some(k)` switches to phase `k % phases`). At most one spawn
/// per step, on about a third of the steps, keeps backlogs (and the cost of each
/// snapshot) bounded while still filling lanes and spilling into backlogs.
type Script = Vec<(Option<usize>, Option<usize>)>;

fn script() -> impl Strategy<Value = Script> {
    prop::collection::vec(
        (
            prop_oneof![2 => Just(None), 1 => (0..64usize).prop_map(Some)],
            prop_oneof![8 => Just(None), 1 => (0..64usize).prop_map(Some)],
        ),
        STEPS,
    )
}

fn run(scenario: &Scenario, script: &Script) {
    let movements = allowed_movements(scenario);
    let phases = scenario.signal_plan().phases().len();
    let mut sim = Simulation::new(scenario.clone());
    let mut last_departed = BTreeMap::new();
    for (spawn, command) in script {
        if let Some(s) = spawn {
            let m = movements[s % movements.len()];
            sim.spawn(m.approach, m.movement).unwrap();
        }
        let command = command.map_or(Command::Hold, |k| {
            Command::SwitchTo(PhaseId::new(k % phases))
        });
        let before = Snapshot::of(&sim);
        let report = sim.step(command);
        check_step(&before, &Snapshot::of(&sim), &report, &mut last_departed);
    }
}

proptest! {
    #[test]
    fn example_scenario_invariants(script in script()) {
        run(&Scenario::from_toml_str(EXAMPLE).unwrap(), &script);
    }

    #[test]
    fn shared_lane_invariants(script in script()) {
        run(&shared_lane(), &script);
    }
}
