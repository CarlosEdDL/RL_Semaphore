//! Any valid fixed-time plan runs without the signal forcing or ignoring anything.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{ALL_RED, MIN_GREEN, YELLOW, drive, scenario};
use proptest::prelude::*;
use rl_semaphore_env::FixedTime;
use rl_semaphore_sim::{Simulation, StepOutcome};

/// A phase count, its greens (from min-green up to the longest that any max-red
/// allows on this scenario), and how many steps of slack max-red has above the
/// tightest value the plan needs.
fn plans() -> impl Strategy<Value = (usize, Vec<u32>, u32)> {
    (2usize..=4).prop_flat_map(|n| (Just(n), prop::collection::vec(MIN_GREEN..=40, n), 0u32..=3))
}

proptest! {
    #[test]
    fn valid_plans_are_never_corrected((n, greens, slack) in plans()) {
        let transition = YELLOW + ALL_RED;
        let cycle: u32 = greens.iter().map(|g| g + transition).sum();
        // The tightest max-red that admits the plan: the shortest green is red the longest.
        let limit = cycle - greens.iter().min().unwrap();
        let greens_s: Vec<f64> = greens.iter().map(|&g| f64::from(g)).collect();
        let s = scenario(n, f64::from(limit + slack), &greens_s).unwrap();
        prop_assert_eq!(s.fixed_time().unwrap().cycle_steps(), cycle);

        let mut controller = FixedTime::new(s.fixed_time().unwrap());
        let mut sim = Simulation::new(s);
        let outcomes = drive(&mut sim, &mut controller, 3 * cycle as usize + 10);
        prop_assert!(outcomes
            .iter()
            .all(|o| matches!(o, StepOutcome::Held | StepOutcome::SwitchStarted(_))));
    }

    #[test]
    fn one_step_below_the_limit_is_rejected((n, greens, _) in plans()) {
        let transition = YELLOW + ALL_RED;
        let cycle: u32 = greens.iter().map(|g| g + transition).sum();
        let limit = cycle - greens.iter().min().unwrap();
        let greens_s: Vec<f64> = greens.iter().map(|&g| f64::from(g)).collect();
        prop_assert!(scenario(n, f64::from(limit - 1), &greens_s).is_err());
    }
}
