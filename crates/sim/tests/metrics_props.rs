//! Property test: arbitrary valid demand and seeds keep the wait rule, the delay identity,
//! the queue bound and the conservation of the metrics population.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{EXAMPLE, run_with_metrics};
use proptest::prelude::*;
use rl_semaphore_sim::{
    ApproachDemandConfig, Command, Demand, EpisodeMetrics, PhaseId, Scenario, ScenarioConfig,
    Simulation,
};

const STEPS: usize = 1000;

/// Same bounds as `demand_props.rs`.
fn entry(max_flow: f64) -> impl Strategy<Value = ApproachDemandConfig> {
    (0.0..=max_flow, 0.0..=1.0f64, 0.0..=1.0f64).prop_map(|(veh_per_h, a, b)| {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        ApproachDemandConfig {
            veh_per_h,
            left: lo,
            through: hi - lo,
            right: 1.0 - hi,
        }
    })
}

proptest! {
    #[test]
    fn metrics_hold_for_arbitrary_demand_and_commands(
        north in entry(600.0),
        south in entry(600.0),
        east in entry(250.0),
        west in entry(250.0),
        seed in any::<u64>(),
        commands in prop::collection::vec(
            prop_oneof![8 => Just(None), 1 => (0..64usize).prop_map(Some)],
            STEPS,
        ),
    ) {
        let mut config = ScenarioConfig::from_toml_str(EXAMPLE).unwrap();
        config.demand.north = Some(north);
        config.demand.south = Some(south);
        config.demand.east = Some(east);
        config.demand.west = Some(west);
        let scenario: Scenario = config.validate().unwrap();
        let phases = scenario.signal_plan().phases().len();

        let mut demand = Demand::new(&scenario, seed);
        let mut sim = Simulation::new(scenario);
        let mut metrics = EpisodeMetrics::new(&sim);
        let in_model_at_t0 = sim.vehicles().count() as u64;
        // The wait rule and the delay identity are checked at every step by `run_with_metrics`.
        let log = run_with_metrics(&mut sim, &mut demand, &mut metrics, STEPS, |t| {
            commands[usize::try_from(t).unwrap()]
                .map_or(Command::Hold, |k| Command::SwitchTo(PhaseId::new(k % phases)))
        });

        let spawned: u64 = log.iter().map(|s| s.arrivals.len() as u64).sum();
        let summary = metrics.summary(&sim);
        prop_assert_eq!(summary.steps, STEPS as u64);
        prop_assert_eq!(summary.departed + summary.in_system, spawned + in_model_at_t0);
        prop_assert_eq!(summary.departed, sim.departed_count());
        let population = summary.wait.as_ref().map_or(0, |w| w.count);
        prop_assert_eq!(population, summary.departed + summary.in_system);
        let by_approach: u64 = summary.wait_by_approach.iter().flatten().map(|w| w.count).sum();
        prop_assert_eq!(by_approach, population);
    }
}
