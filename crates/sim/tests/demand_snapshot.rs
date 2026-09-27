//! Determinism: one full trajectory of the example scenario is pinned by a snapshot.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fmt::Write;

use common::{EXAMPLE, run_with_demand};
use rl_semaphore_sim::{Command, Demand, Scenario, Simulation};

#[test]
fn example_trajectory_is_pinned() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut demand = Demand::new(&scenario, 2024);
    let mut sim = Simulation::new(scenario);
    let log = run_with_demand(&mut sim, &mut demand, 600, |_| Command::Hold);

    let mut text = String::new();
    for step in log
        .iter()
        .filter(|s| !s.arrivals.is_empty() || !s.departures.is_empty())
    {
        write!(text, "t={}", step.t).unwrap();
        if !step.arrivals.is_empty() {
            text.push_str(" arrive");
            for (id, m) in &step.arrivals {
                write!(text, " {id}:{m}").unwrap();
            }
        }
        if !step.departures.is_empty() {
            text.push_str(" depart");
            for id in &step.departures {
                write!(text, " {id}").unwrap();
            }
        }
        text.push('\n');
    }
    writeln!(
        text,
        "final spawned={} backlog={} on_lane={} departed={}",
        sim.spawned_count(),
        sim.backlog_count(),
        sim.on_lane_count(),
        sim.departed_count()
    )
    .unwrap();
    insta::assert_snapshot!(text);
}
