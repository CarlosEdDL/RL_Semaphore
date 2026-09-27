//! Property test: arbitrary valid demand and seeds keep every 1.3 invariant.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{EXAMPLE, run_with_demand};
use proptest::prelude::*;
use rl_semaphore_sim::{
    ApproachDemandConfig, Command, Demand, Scenario, ScenarioConfig, Simulation,
};

const STEPS: usize = 1000;

/// Ratios over the movements the example lanes allow: north/south allow all three, and
/// so do east/west, so any split works. Returns left, through, right summing to 1.
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
    fn arbitrary_demand_keeps_the_invariants(
        north in entry(600.0),
        south in entry(600.0),
        east in entry(250.0),
        west in entry(250.0),
        seed in any::<u64>(),
    ) {
        let mut config = ScenarioConfig::from_toml_str(EXAMPLE).unwrap();
        config.demand.north = Some(north);
        config.demand.south = Some(south);
        config.demand.east = Some(east);
        config.demand.west = Some(west);
        let scenario: Scenario = config.validate().unwrap();

        let mut demand = Demand::new(&scenario, seed);
        let mut sim = Simulation::new(scenario);
        let log = run_with_demand(&mut sim, &mut demand, STEPS, |_| Command::Hold);
        let arrived: usize = log.iter().map(|s| s.arrivals.len()).sum();
        prop_assert_eq!(sim.spawned_count(), arrived as u64);
    }
}
