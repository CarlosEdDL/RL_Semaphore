//! Throughput benchmarks for the simulator's step loop.
//!
//! Two criterion groups, both built from `configs/single-intersection.toml` and each
//! with one benchmark per workload (`empty`, `example`, `heavy`; see [`workloads`]):
//!
//! - `sim_step` times only the simulation: spawning a step's pre-drawn arrivals,
//!   `FixedTime::command`, and `Simulation::step`. It excludes the demand RNG and the
//!   metrics.
//! - `episode` times `run_episode` end to end: building the `Simulation`, drawing the
//!   demand with `Demand`, the same stepping, and `EpisodeMetrics::observe`.
//!
//! The gap between the two groups is the cost of the demand draw and the metrics.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use rl_semaphore_env::{Controller, FixedTime, run_episode};
use rl_semaphore_sim::{
    ApproachDemandConfig, Demand, DemandConfig, MovementId, Scenario, ScenarioConfig, Simulation,
};

/// The example scenario, read at compile time from the workspace root.
const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// Episode length of every benchmark iteration, and of the sanity check of [`check_workload`].
const STEPS: u64 = 3_600;

/// Seed of the demand generator for every benchmark iteration and sanity check.
const SEED: u64 = 0;

/// The highest tested load at which the fixed-time plan of the example scenario stays
/// stable. Measured with `simulate` (fixed-time plan, seed 0), scaling every flow:
///
/// | Factor | Demand       | Mean wait, 3,600 / 36,000 steps | Max wait, 3,600 / 36,000 steps |
/// |--------|--------------|----------------------------------|---------------------------------|
/// | 1.0    | 1,100 veh/h  | 29.8 s / 28.1 s                  | 152 s / 194 s                   |
/// | 1.3    | 1,430 veh/h  | 44.6 s / 39.6 s                  | 338 s / 338 s                    |
/// | 1.4    | 1,540 veh/h  | 54.2 s / 54.9 s                  | 364 s / 512 s (east grows)        |
/// | 1.5    | 1,650 veh/h  | 68.3 s / 197.2 s                 | 495 s / 1,330 s                  |
///
/// The side road (10 s of green in an 80 s cycle) saturates first. 1.3 is the last
/// factor where neither the mean nor the max wait grows between 3,600 and 36,000
/// steps, so the `heavy` workload's step rate stays independent of episode length.
/// See `specs/1.7-sim-benchmark/plan.md`.
const HEAVY_FACTOR: f64 = 1.3;

/// Scales every approach's flow by `factor`, keeping the turn ratios unchanged.
fn scale_demand(demand: &DemandConfig, factor: f64) -> DemandConfig {
    let scale = |entry: &Option<ApproachDemandConfig>| {
        entry.as_ref().map(|a| ApproachDemandConfig {
            veh_per_h: a.veh_per_h * factor,
            ..a.clone()
        })
    };
    DemandConfig {
        north: scale(&demand.north),
        east: scale(&demand.east),
        south: scale(&demand.south),
        west: scale(&demand.west),
    }
}

/// Builds the three workloads: the example's geometry, signal and fixed-time plan,
/// changed only in `demand` (R2.1–R2.2).
fn workloads() -> [(&'static str, Scenario); 3] {
    let config = ScenarioConfig::from_toml_str(EXAMPLE).expect("the example scenario should parse");

    let mut empty = config.clone();
    empty.demand = DemandConfig::default();

    let example = config.clone();

    let mut heavy = config;
    heavy.demand = scale_demand(&heavy.demand, HEAVY_FACTOR);

    let validate = |name: &'static str, config: ScenarioConfig| {
        (
            name,
            config
                .validate()
                .unwrap_or_else(|e| panic!("workload {name} should validate: {e}")),
        )
    };
    [
        validate("empty", empty),
        validate("example", example),
        validate("heavy", heavy),
    ]
}

/// Runs `scenario` once with `FixedTime`, outside the timed code, and panics naming
/// `name` unless the workload behaves as one must (R2.5).
fn check_workload(name: &str, scenario: &Scenario) {
    let plan = scenario.fixed_time().unwrap_or_else(|| {
        panic!("workload {name}: the example scenario has a [fixed_time] table")
    });
    let mut controller = FixedTime::new(plan);
    let report = run_episode(scenario, &mut controller, SEED, STEPS)
        .unwrap_or_else(|e| panic!("workload {name}: run_episode failed: {e}"));
    assert_eq!(
        report.summary.steps, STEPS,
        "workload {name}: wrong step count"
    );
    assert_eq!(
        report.signal.switches_forced, 0,
        "workload {name}: the signal forced a switch"
    );
    assert_eq!(
        report.signal.commands_ignored, 0,
        "workload {name}: the signal ignored a command"
    );
    if name == "empty" {
        assert_eq!(
            report.summary.departed, 0,
            "workload {name}: a vehicle departed with no demand"
        );
    }
}

/// Times only the simulation: spawn, `FixedTime::command`, `Simulation::step`, in the
/// loop order of `run_episode`, so the trajectory is the one it produces for the same
/// seed (R3.3).
fn sim_step_group(c: &mut Criterion, workloads: &[(&'static str, Scenario); 3]) {
    let mut group = c.benchmark_group("sim_step");
    group.throughput(Throughput::Elements(STEPS));
    for (name, scenario) in workloads {
        group.bench_function(*name, |b| {
            b.iter_batched(
                || {
                    let sim = Simulation::new(scenario.clone());
                    let mut demand = Demand::new(scenario, SEED);
                    let arrivals: Vec<Vec<MovementId>> =
                        (0..STEPS).map(|_| demand.arrivals()).collect();
                    let plan = scenario
                        .fixed_time()
                        .expect("workload has a [fixed_time] table");
                    let controller = FixedTime::new(plan);
                    (sim, arrivals, controller)
                },
                |(mut sim, arrivals, mut controller)| {
                    for step_arrivals in arrivals {
                        for m in step_arrivals {
                            sim.spawn(m.approach, m.movement)
                                .expect("a scenario's demand only uses movements its lanes allow");
                        }
                        let command = controller.command(&sim);
                        let report = sim.step(command);
                        black_box(&report);
                    }
                    black_box(sim.step_count())
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Times `run_episode` end to end: the demand draw, the same stepping, and the
/// metrics (R3.4).
fn episode_group(c: &mut Criterion, workloads: &[(&'static str, Scenario); 3]) {
    let mut group = c.benchmark_group("episode");
    group.throughput(Throughput::Elements(STEPS));
    for (name, scenario) in workloads {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let plan = scenario
                    .fixed_time()
                    .expect("workload has a [fixed_time] table");
                let mut controller = FixedTime::new(plan);
                let report = run_episode(scenario, &mut controller, SEED, STEPS)
                    .expect("run_episode should not fail for a validated scenario");
                black_box(report);
            });
        });
    }
    group.finish();
}

/// Builds and checks the workloads once (R2.5), then registers both groups.
fn benches(c: &mut Criterion) {
    let workloads = workloads();
    for (name, scenario) in &workloads {
        check_workload(name, scenario);
    }
    sim_step_group(c, &workloads);
    episode_group(c, &workloads);
}

criterion_group!(throughput, benches);
criterion_main!(throughput);
