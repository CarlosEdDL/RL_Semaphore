//! Hand-traced unit tests for spawning and the step rule.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    EXAMPLE, PHASES_SHARED, PHASES_THROUGH, PHASES_THROUGH_RIGHT, Snapshot, check_step, inline,
    shared_lane,
};
use rl_semaphore_sim::{
    Command, Direction, LaneId, Light, Movement, MovementId, PhaseId, Position, Scenario,
    Simulation, SpawnError, StepReport,
};

use std::collections::BTreeMap;

fn lane(sim: &Simulation, d: Direction, i: usize) -> LaneId {
    sim.scenario().intersection().approach(d).lanes()[i].id()
}

fn cells(sim: &Simulation, l: LaneId) -> Vec<Option<u64>> {
    sim.lane_cells(l)
        .unwrap()
        .iter()
        .map(|c| c.map(|id| id.get()))
        .collect()
}

fn backlog(sim: &Simulation, l: LaneId) -> Vec<u64> {
    sim.backlog(l).unwrap().map(|id| id.get()).collect()
}

/// Steps once, running every invariant on the way.
fn step(sim: &mut Simulation, c: Command) -> StepReport {
    let before = Snapshot::of(sim);
    let report = sim.step(c);
    check_step(&before, &Snapshot::of(sim), &report, &mut BTreeMap::new());
    report
}

fn two_lane() -> Scenario {
    inline(
        r#"[{ movements = ["through"] }, { movements = ["through", "right"] }]"#,
        PHASES_THROUGH_RIGHT,
    )
}

const NORTH_THROUGH: MovementId = MovementId::new(Direction::North, Movement::Through);

#[test]
fn new_simulation_is_empty() {
    let sim = Simulation::new(Scenario::from_toml_str(EXAMPLE).unwrap());
    assert_eq!(sim.step_count(), 0);
    assert_eq!(sim.vehicles().count(), 0);
    assert_eq!(
        (
            sim.spawned_count(),
            sim.backlog_count(),
            sim.on_lane_count(),
            sim.departed_count()
        ),
        (0, 0, 0, 0)
    );
    for approach in sim.scenario().intersection().approaches() {
        for l in approach.lanes() {
            assert!(sim.lane_cells(l.id()).unwrap().iter().all(Option::is_none));
            assert_eq!(
                sim.lane_cells(l.id()).unwrap().len(),
                l.len_cells() as usize
            );
            assert_eq!(sim.backlog(l.id()).unwrap().len(), 0);
        }
    }
    assert_eq!(sim.lights(), sim.signal().lights());
}

#[test]
fn ids_and_lane_choice() {
    let mut sim = Simulation::new(two_lane());
    let (l0, l1) = (
        lane(&sim, Direction::North, 0),
        lane(&sim, Direction::North, 1),
    );
    let mut lanes = Vec::new();
    for (i, m) in [
        Movement::Through,
        Movement::Through,
        Movement::Through,
        Movement::Right,
        Movement::Through,
    ]
    .into_iter()
    .enumerate()
    {
        let id = sim.spawn(Direction::North, m).unwrap();
        assert_eq!(id.get(), i as u64);
        lanes.push(sim.vehicle(id).unwrap().lane());
        assert_eq!(sim.vehicle(id).unwrap().spawned_at(), 0);
        assert_eq!(sim.vehicle(id).unwrap().position(), Position::Backlog);
    }
    // Tie -> lane 0; then lane 1 is lighter; tie again; right only fits lane 1; tie -> lane 0.
    assert_eq!(lanes, [l0, l1, l0, l1, l0]);
    assert_eq!(backlog(&sim, l0), [0, 2, 4]);
    assert_eq!(backlog(&sim, l1), [1, 3]);
}

#[test]
fn spawn_without_lane_fails_and_changes_nothing() {
    let mut sim = Simulation::new(two_lane());
    let err = sim.spawn(Direction::North, Movement::Left).unwrap_err();
    assert_eq!(
        err,
        SpawnError::NoLaneForMovement {
            movement: MovementId::new(Direction::North, Movement::Left)
        }
    );
    assert_eq!(sim.spawned_count(), 0);
    assert_eq!(sim.vehicles().count(), 0);
    assert_eq!(
        sim.spawn(Direction::North, Movement::Through)
            .unwrap()
            .get(),
        0
    );
}

#[test]
fn lone_vehicle_crosses_on_step_n_plus_one() {
    let mut sim = Simulation::new(Scenario::from_toml_str(EXAMPLE).unwrap());
    let l = lane(&sim, Direction::North, 0); // left lane, green at t = 0
    assert_eq!(
        sim.scenario()
            .intersection()
            .approach(Direction::North)
            .lanes()[0]
            .len_cells(),
        20
    );
    let id = sim.spawn(Direction::North, Movement::Left).unwrap();
    for t in 1..=20 {
        let r = step(&mut sim, Command::Hold);
        assert!(r.departures.is_empty(), "left at step {t}");
        let expected = if t == 1 {
            Position::OnLane { cell: 0 }
        } else {
            Position::OnLane { cell: t - 1 }
        };
        assert_eq!(sim.vehicle(id).unwrap().position(), expected);
    }
    assert_eq!(cells(&sim, l)[19], Some(0));
    let r = step(&mut sim, Command::Hold);
    assert_eq!(sim.step_count(), 21);
    assert_eq!(r.departures.len(), 1);
    assert_eq!(
        (
            r.departures[0].vehicle,
            r.departures[0].spawned_at,
            r.departures[0].departed_at
        ),
        (id, 0, 21)
    );
    assert!(sim.vehicle(id).is_none());
    assert_eq!((sim.on_lane_count(), sim.departed_count()), (0, 1));
}

/// A scenario where north through starts red: east is the first phase.
fn north_red_first() -> Scenario {
    inline(
        r#"[{ movements = ["through"] }]"#,
        r#"
[[signal.phases]]
name = "east"
green = { east = ["through"] }

[[signal.phases]]
name = "ns"
green = { north = ["through"], south = ["through"] }

[[signal.phases]]
name = "west"
green = { west = ["through"] }
"#,
    )
}

/// Steps with `SwitchTo(ns)` once, then `Hold` until north through is green.
/// Returns the reports from the step where green appears (inclusive) onwards is up to the caller.
fn switch_to_green(sim: &mut Simulation) -> Vec<StepReport> {
    let mut reports = vec![step(sim, Command::SwitchTo(PhaseId::new(1)))];
    while sim.lights()[NORTH_THROUGH.index()] != Light::Green {
        reports.push(step(sim, Command::Hold));
        assert!(reports.len() < 20, "never turned green");
    }
    reports
}

#[test]
fn queue_fills_from_stop_line_and_discharges_every_two_steps() {
    let mut sim = Simulation::new(north_red_first());
    let l = lane(&sim, Direction::North, 0);
    for _ in 0..3 {
        sim.spawn(Direction::North, Movement::Through).unwrap();
    }
    // Hand trace on a 4-cell lane: v0 enters at 1, reaches cell 3 at step 4;
    // v1 enters at 3 and reaches cell 2 at step 5; v2 enters at 5 and reaches cell 1 at step 6.
    let mut trace = Vec::new();
    for _ in 0..6 {
        step(&mut sim, Command::Hold);
        trace.push(cells(&sim, l));
    }
    assert_eq!(trace[3], [None, Some(1), None, Some(0)]);
    assert_eq!(trace[4], [Some(2), None, Some(1), Some(0)]);
    assert_eq!(trace[5], [None, Some(2), Some(1), Some(0)]);
    assert_eq!(sim.departed_count(), 0);

    let reports = switch_to_green(&mut sim);
    let green_step = sim.step_count();
    // The vehicle at the stop line crosses in the step where its light turns green.
    assert_eq!(reports.last().unwrap().departures.len(), 1);
    let mut departed_at = vec![reports.last().unwrap().departures[0].departed_at];
    while sim.on_lane_count() > 0 {
        departed_at.extend(
            step(&mut sim, Command::Hold)
                .departures
                .iter()
                .map(|d| d.departed_at),
        );
    }
    assert_eq!(departed_at, [green_step, green_step + 2, green_step + 4]);
}

#[test]
fn full_lane_spills_into_backlog_then_refills() {
    let mut sim = Simulation::new(north_red_first());
    let l = lane(&sim, Direction::North, 0);
    for _ in 0..6 {
        sim.spawn(Direction::North, Movement::Through).unwrap();
    }
    for _ in 0..15 {
        step(&mut sim, Command::Hold);
    }
    assert_eq!(cells(&sim, l), [Some(3), Some(2), Some(1), Some(0)]);
    assert_eq!(backlog(&sim, l), [4, 5]);
    assert_eq!((sim.on_lane_count(), sim.backlog_count()), (4, 2));

    let mut order: Vec<u64> = switch_to_green(&mut sim)
        .iter()
        .flat_map(|r| r.departures.iter().map(|d| d.vehicle.get()))
        .collect();
    for _ in 0..30 {
        order.extend(
            step(&mut sim, Command::Hold)
                .departures
                .iter()
                .map(|d| d.vehicle.get()),
        );
    }
    assert_eq!(order, [0, 1, 2, 3, 4, 5]);
    assert_eq!(
        (
            sim.on_lane_count(),
            sim.backlog_count(),
            sim.departed_count()
        ),
        (0, 0, 6)
    );
}

#[test]
fn no_crossing_on_yellow() {
    let mut sim = Simulation::new(inline(r#"[{ movements = ["through"] }]"#, PHASES_THROUGH));
    let l = lane(&sim, Direction::North, 0);
    let id = sim.spawn(Direction::North, Movement::Through).unwrap();
    // The vehicle is spawned at t = 0, reaches the stop-line cell (3) at step 4.
    for _ in 0..3 {
        step(&mut sim, Command::Hold);
    }
    let mut yellow_steps = 0;
    for _ in 0..6 {
        let r = step(&mut sim, Command::SwitchTo(PhaseId::new(1)));
        if sim.lights()[NORTH_THROUGH.index()] == Light::Yellow {
            assert!(r.departures.is_empty());
            assert_eq!(
                cells(&sim, l)[3],
                sim.vehicle(id).is_some().then_some(id.get())
            );
            yellow_steps += 1;
        }
    }
    assert!(yellow_steps >= 1, "north through never showed yellow");
    assert_eq!(sim.departed_count(), 0);
    assert_eq!(cells(&sim, l)[3], Some(0));
}

#[test]
fn shared_lane_head_blocks_green_vehicle_behind() {
    let mut sim = Simulation::new(shared_lane());
    let l = lane(&sim, Direction::North, 0);
    let left = sim.spawn(Direction::North, Movement::Left).unwrap();
    let through = sim.spawn(Direction::North, Movement::Through).unwrap();
    for _ in 0..12 {
        step(&mut sim, Command::Hold);
    }
    // Phase "ns" is green: through is green but left (the head) is red.
    assert_eq!(sim.lights()[NORTH_THROUGH.index()], Light::Green);
    assert_eq!(sim.departed_count(), 0);
    assert_eq!(cells(&sim, l), [None, None, Some(1), Some(0)]);

    // Give the left turn its phase: it leaves, then the through vehicle waits on red.
    let mut order = Vec::new();
    for _ in 0..12 {
        let r = step(&mut sim, Command::SwitchTo(PhaseId::new(1)));
        order.extend(r.departures.iter().map(|d| d.vehicle));
    }
    assert_eq!(order.first(), Some(&left));
    assert!(sim.vehicle(through).is_some());
}

#[test]
fn same_script_gives_same_run() {
    let run = || {
        let mut sim = Simulation::new(Scenario::from_toml_str(EXAMPLE).unwrap());
        let mut reports = Vec::new();
        for k in 0..300usize {
            if k % 3 == 0 {
                sim.spawn(Direction::North, Movement::Through).unwrap();
            }
            if k % 4 == 0 {
                sim.spawn(Direction::East, Movement::Left).unwrap();
            }
            let c = if k % 40 == 39 {
                Command::SwitchTo(PhaseId::new((k / 40) % 4))
            } else {
                Command::Hold
            };
            reports.push(sim.step(c));
        }
        (sim.vehicles().cloned().collect::<Vec<_>>(), reports)
    };
    assert_eq!(run(), run());
}

#[test]
fn shared_phases_constant_is_used() {
    // Keeps PHASES_SHARED referenced from this file for readers grepping the scenario.
    assert!(PHASES_SHARED.contains("north-left"));
}
