//! Demand: config, validation, and the generator.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::EXAMPLE;
use rl_semaphore_sim::{
    ApproachDemandConfig, ConfigError, Demand, Direction, Movement, MovementId, Scenario,
    ScenarioConfig, Simulation,
};

fn example_config() -> ScenarioConfig {
    ScenarioConfig::from_toml_str(EXAMPLE).unwrap()
}

fn entry(veh_per_h: f64, left: f64, through: f64, right: f64) -> ApproachDemandConfig {
    ApproachDemandConfig {
        veh_per_h,
        left,
        through,
        right,
    }
}

fn error_of(config: &ScenarioConfig) -> ConfigError {
    config.validate().unwrap_err()
}

// ---- config ----

#[test]
fn example_loads_with_its_demand() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let plan = scenario.demand();
    for (d, flow) in [
        (Direction::North, 400.0),
        (Direction::South, 400.0),
        (Direction::East, 150.0),
        (Direction::West, 150.0),
    ] {
        assert_eq!(plan.veh_per_h(d), flow);
        let expected = flow * scenario.step_s() / 3600.0;
        assert!((plan.mean_per_step(d) - expected).abs() < 1e-12);
    }
    assert_eq!(plan.ratio(Direction::North, Movement::Left), 0.2);
    assert_eq!(plan.ratio(Direction::East, Movement::Right), 0.2);
}

#[test]
fn no_demand_table_means_zero_demand() {
    let text = EXAMPLE.split("\n[demand]").next().unwrap();
    let scenario = Scenario::from_toml_str(text).unwrap();
    assert!(scenario.to_config().demand.is_empty());
    for d in Direction::ALL {
        assert_eq!(scenario.demand().veh_per_h(d), 0.0);
        assert_eq!(scenario.demand().mean_per_step(d), 0.0);
        for m in Movement::ALL {
            assert_eq!(scenario.demand().ratio(d, m), 0.0);
        }
    }
    let mut demand = Demand::new(&scenario, 1);
    for _ in 0..1000 {
        assert!(demand.arrivals().is_empty());
    }
    // The serialized form has no demand table either.
    let toml = scenario.to_config().to_toml_string().unwrap();
    assert!(!toml.contains("demand"));
}

#[test]
fn missing_approaches_have_no_demand() {
    let mut config = example_config();
    config.demand.east = None;
    config.demand.west = None;
    let scenario = config.validate().unwrap();
    assert_eq!(scenario.demand().veh_per_h(Direction::East), 0.0);
    assert_eq!(scenario.demand().veh_per_h(Direction::West), 0.0);
    assert_eq!(scenario.demand().veh_per_h(Direction::North), 400.0);
}

#[test]
fn ratios_default_to_zero_when_omitted() {
    let text = EXAMPLE.replace(
        "north = { veh_per_h = 400.0, left = 0.2, through = 0.7, right = 0.1 }",
        "north = { veh_per_h = 400.0, through = 1.0 }",
    );
    let scenario = Scenario::from_toml_str(&text).unwrap();
    assert_eq!(
        scenario.demand().ratio(Direction::North, Movement::Left),
        0.0
    );
    assert_eq!(
        scenario.demand().ratio(Direction::North, Movement::Through),
        1.0
    );
}

#[test]
fn unknown_demand_keys_are_rejected() {
    let text = EXAMPLE.replace("veh_per_h = 400.0,", "veh_per_h = 400.0, speed = 1.0,");
    assert!(matches!(
        Scenario::from_toml_str(&text),
        Err(ConfigError::Parse(_))
    ));
    let text = format!("{EXAMPLE}\nnorthwest = {{ veh_per_h = 1.0, through = 1.0 }}\n");
    assert!(matches!(
        Scenario::from_toml_str(&text),
        Err(ConfigError::Parse(_))
    ));
}

#[test]
fn invalid_flow_names_its_path() {
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let mut config = example_config();
        config.demand.south = Some(entry(bad, 0.2, 0.7, 0.1));
        match error_of(&config) {
            ConfigError::InvalidFlow { path, .. } => assert_eq!(path, "demand.south.veh_per_h"),
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn invalid_ratio_names_its_path() {
    for (bad, field) in [(-0.1, "left"), (1.5, "through"), (f64::NAN, "right")] {
        let mut config = example_config();
        let mut e = entry(100.0, 0.2, 0.7, 0.1);
        match field {
            "left" => e.left = bad,
            "through" => e.through = bad,
            _ => e.right = bad,
        }
        config.demand.west = Some(e);
        match error_of(&config) {
            ConfigError::InvalidTurnRatio { path, .. } => {
                assert_eq!(path, format!("demand.west.{field}"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn ratio_on_a_movement_no_lane_allows_is_rejected() {
    let mut config = example_config();
    // North has left and through/right lanes, so make a scenario where it lacks right.
    config.intersection.approaches.north.lanes[1].movements = vec![Movement::Through];
    config.signal.phases[1].green.north = vec![Movement::Through];
    config.signal.phases[1].green.south = vec![Movement::Through, Movement::Right];
    config.demand.north = Some(entry(100.0, 0.2, 0.7, 0.1));
    match error_of(&config) {
        ConfigError::MovementNotInGeometry { path, movement } => {
            assert_eq!(path, "demand.north.right");
            assert_eq!(movement, Movement::Right);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn ratios_must_sum_to_one_even_without_flow() {
    let mut config = example_config();
    config.demand.east = Some(entry(0.0, 0.2, 0.6, 0.1));
    match error_of(&config) {
        ConfigError::TurnRatioSum { path, sum } => {
            assert_eq!(path, "demand.east");
            assert!((sum - 0.9).abs() < 1e-12);
        }
        other => panic!("unexpected {other:?}"),
    }
    // Within the tolerance is fine.
    config.demand.east = Some(entry(0.0, 0.2, 0.6, 0.2 + 1e-12));
    config.validate().unwrap();
}

#[test]
fn flow_above_ten_arrivals_per_step_is_rejected() {
    let mut config = example_config();
    config.demand.north = Some(entry(36_001.0, 0.2, 0.7, 0.1));
    match error_of(&config) {
        ConfigError::FlowTooHigh {
            path,
            mean_per_step,
            max,
            ..
        } => {
            assert_eq!(path, "demand.north.veh_per_h");
            assert!(mean_per_step > max);
            assert_eq!(max, rl_semaphore_sim::MAX_MEAN_ARRIVALS_PER_STEP);
        }
        other => panic!("unexpected {other:?}"),
    }
    config.demand.north = Some(entry(36_000.0, 0.2, 0.7, 0.1));
    config.validate().unwrap();
}

#[test]
fn validation_order_is_documented_order() {
    // Flow before ratios, ratios before the sum, approaches in Direction::ALL order.
    let mut config = example_config();
    config.demand.north = Some(entry(-1.0, 2.0, 0.0, 0.0));
    assert!(matches!(error_of(&config), ConfigError::InvalidFlow { .. }));

    config.demand.north = Some(entry(1.0, 2.0, 0.0, 0.0));
    assert!(matches!(
        error_of(&config),
        ConfigError::InvalidTurnRatio { .. }
    ));

    config.demand.north = Some(entry(1.0, 0.5, 0.0, 0.0));
    assert!(matches!(
        error_of(&config),
        ConfigError::TurnRatioSum { .. }
    ));

    // Sum before mean.
    config.demand.north = Some(entry(40_000.0, 0.5, 0.0, 0.0));
    assert!(matches!(
        error_of(&config),
        ConfigError::TurnRatioSum { .. }
    ));

    // North is checked before west.
    config.demand.north = Some(entry(1.0, 0.5, 0.0, 0.0));
    config.demand.west = Some(entry(-1.0, 0.2, 0.6, 0.2));
    match error_of(&config) {
        ConfigError::TurnRatioSum { path, .. } => assert_eq!(path, "demand.north"),
        other => panic!("unexpected {other:?}"),
    }

    // Demand comes after the signal checks.
    config.signal.phases.clear();
    assert!(matches!(error_of(&config), ConfigError::PhaseCount { .. }));
}

#[test]
fn config_and_toml_round_trip() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let config = scenario.to_config();
    assert_eq!(config.demand, example_config().demand);
    let again = Scenario::from_toml_str(&config.to_toml_string().unwrap()).unwrap();
    assert_eq!(again, scenario);
}

// ---- generator ----

fn collect(demand: &mut Demand, steps: usize) -> Vec<Vec<MovementId>> {
    (0..steps).map(|_| demand.arrivals()).collect()
}

#[test]
fn same_seed_same_arrivals_and_clone_continues() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut a = Demand::new(&scenario, 99);
    let mut b = Demand::new(&scenario, 99);
    assert_eq!(a.seed(), 99);
    assert_eq!(collect(&mut a, 10_000), collect(&mut b, 10_000));

    let mut fork = a.clone();
    assert_eq!(collect(&mut a, 2_000), collect(&mut fork, 2_000));
}

#[test]
fn different_seeds_differ() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let a = collect(&mut Demand::new(&scenario, 1), 1_000);
    let b = collect(&mut Demand::new(&scenario, 2), 1_000);
    assert_ne!(a, b);
}

#[test]
fn approaches_are_independent_streams() {
    let base = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut config = example_config();
    config.demand.north = Some(entry(1_200.0, 0.0, 0.5, 0.5));
    config.demand.west = None;
    let changed = config.validate().unwrap();

    let steps = 5_000;
    let a = collect(&mut Demand::new(&base, 5), steps);
    let b = collect(&mut Demand::new(&changed, 5), steps);
    let only = |log: &[Vec<MovementId>], d: Direction| -> Vec<Vec<MovementId>> {
        log.iter()
            .map(|s| s.iter().copied().filter(|m| m.approach == d).collect())
            .collect()
    };
    for d in [Direction::East, Direction::South] {
        assert_eq!(only(&a, d), only(&b, d), "{d} changed");
    }
    assert_ne!(only(&a, Direction::North), only(&b, Direction::North));
    assert!(only(&b, Direction::West).iter().all(Vec::is_empty));
}

#[test]
fn only_allowed_movements_appear_and_idle_approaches_stay_idle() {
    let mut config = example_config();
    // The side road only turns left and right; north has no demand.
    config.demand.east = Some(entry(600.0, 0.5, 0.0, 0.5));
    config.demand.north = None;
    let scenario = config.validate().unwrap();
    let allowed = common::allowed_movements(&scenario);
    let mut demand = Demand::new(&scenario, 3);
    let mut through_east = 0;
    for _ in 0..20_000 {
        for m in demand.arrivals() {
            assert!(allowed.contains(&m));
            assert_ne!(m.approach, Direction::North);
            if m.approach == Direction::East {
                assert_ne!(m.movement, Movement::Through);
                through_east += 1;
            }
        }
    }
    assert!(through_east > 0);
}

#[test]
fn turn_shares_match_the_ratios() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut demand = Demand::new(&scenario, 11);
    let mut counts = [[0u32; 3]; 4];
    for _ in 0..200_000 {
        for m in demand.arrivals() {
            counts[m.approach.index()][m.movement.index()] += 1;
        }
    }
    for d in Direction::ALL {
        let total = f64::from(counts[d.index()].iter().sum::<u32>());
        for m in Movement::ALL {
            let r = scenario.demand().ratio(d, m);
            let share = f64::from(counts[d.index()][m.index()]) / total;
            let se = (r * (1.0 - r) / total).sqrt();
            assert!((share - r).abs() < 5.0 * se, "{d} {m}: {share} vs {r}");
        }
    }
}

#[test]
fn every_arrival_spawns() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut demand = Demand::new(&scenario, 8);
    let mut sim = Simulation::new(scenario);
    let mut total = 0;
    for _ in 0..2_000 {
        for m in demand.arrivals() {
            sim.spawn(m.approach, m.movement).unwrap();
            total += 1;
        }
    }
    assert_eq!(sim.spawned_count(), total);
}
