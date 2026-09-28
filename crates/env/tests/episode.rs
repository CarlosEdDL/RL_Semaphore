//! `Episode`, the step-by-step episode stepper (R2, R9.1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::EXAMPLE;
use rl_semaphore_env::{Controller, Episode, FixedTime, run_episode};
use rl_semaphore_sim::{Demand, EpisodeMetrics, Scenario, Simulation};

fn example() -> Scenario {
    Scenario::from_toml_str(EXAMPLE).unwrap()
}

/// A manual loop of `Episode::step` gives the same report as `run_episode`, for two seeds
/// over 600 steps.
#[test]
fn manual_loop_matches_run_episode() {
    for seed in [11, 12] {
        let scenario = example();
        let mut controller = FixedTime::new(scenario.fixed_time().unwrap());
        let mut episode = Episode::new(&scenario, seed);
        for _ in 0..600 {
            episode.step(&mut controller).unwrap();
        }
        let stepped = episode.report();

        let mut controller = FixedTime::new(scenario.fixed_time().unwrap());
        let run = run_episode(&scenario, &mut controller, seed, 600).unwrap();

        assert_eq!(stepped, run, "seed={seed}");
    }
}

/// `Episode::seed` and `Episode::signal_counts` agree with the report and the seed it was
/// built with.
#[test]
fn seed_and_signal_counts_are_exposed() {
    let scenario = example();
    let mut controller = FixedTime::new(scenario.fixed_time().unwrap());
    let mut episode = Episode::new(&scenario, 99);
    assert_eq!(episode.seed(), 99);
    for _ in 0..50 {
        episode.step(&mut controller).unwrap();
    }
    assert_eq!(episode.signal_counts(), episode.report().signal);
}

/// `Episode::summary` at an intermediate step equals the summary of a loop written by hand
/// over `Simulation`, `Demand` and `EpisodeMetrics`.
#[test]
fn summary_matches_a_hand_written_loop_at_intermediate_steps() {
    let scenario = example();
    let mut controller = FixedTime::new(scenario.fixed_time().unwrap());
    let mut episode = Episode::new(&scenario, 7);

    let mut demand = Demand::new(&scenario, 7);
    let mut sim = Simulation::new(scenario.clone());
    let mut metrics = EpisodeMetrics::new(&sim);
    let mut hand_controller = FixedTime::new(scenario.fixed_time().unwrap());

    for t in 1..=300u64 {
        episode.step(&mut controller).unwrap();

        for arrival in demand.arrivals() {
            sim.spawn(arrival.approach, arrival.movement).unwrap();
        }
        let command = hand_controller.command(&sim);
        let report = sim.step(command);
        metrics.observe(&sim, &report).unwrap();

        if t % 50 == 0 {
            assert_eq!(episode.summary(), metrics.summary(&sim), "t={t}");
        }
    }
}
