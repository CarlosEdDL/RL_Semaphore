//! Hand-traced tests for per-vehicle waits and the episode collector.
//!
//! Every scenario is `common::inline`: 4-cell lanes, `step_s = 1`, phase 0 (north and
//! south) green at step 0. A lone vehicle spawned at `t` enters at `t + 1`, reaches the
//! stop-line cell (cell 3) at `t + 4` and crosses at `t + 5` under green.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    PHASES_THROUGH, PHASES_THROUGH_RIGHT, Snapshot, check_queue_bound, check_step, inline,
};
use rl_semaphore_sim::{
    Command, Direction, EpisodeMetrics, EpisodeSummary, LaneId, MetricsError, Movement, PhaseId,
    Scenario, Simulation, StepReport, VehicleId,
};

use std::collections::BTreeMap;

const EAST_PHASE: PhaseId = PhaseId::new(1);

/// A simulation and its collector, stepped together.
struct Rig {
    sim: Simulation,
    metrics: EpisodeMetrics,
    departures: Vec<(VehicleId, u64, u64)>,
}

impl Rig {
    fn new(scenario: Scenario) -> Self {
        let sim = Simulation::new(scenario);
        let metrics = EpisodeMetrics::new(&sim);
        Self {
            sim,
            metrics,
            departures: Vec::new(),
        }
    }

    fn through() -> Self {
        Self::new(inline(r#"[{ movements = ["through"] }]"#, PHASES_THROUGH))
    }

    fn spawn(&mut self, d: Direction, m: Movement) -> VehicleId {
        self.sim.spawn(d, m).unwrap()
    }

    /// Steps with every invariant checked, and observes.
    fn step(&mut self, c: Command) -> StepReport {
        let before = Snapshot::of(&self.sim);
        let report = self.sim.step(c);
        check_step(
            &before,
            &Snapshot::of(&self.sim),
            &report,
            &mut BTreeMap::new(),
        );
        self.metrics.observe(&self.sim, &report).unwrap();
        check_queue_bound(&self.sim);
        for d in &report.departures {
            self.departures
                .push((d.vehicle, d.departed_at, d.wait_steps));
        }
        report
    }

    fn run(&mut self, steps: u64) {
        for _ in 0..steps {
            self.step(Command::Hold);
        }
    }

    fn wait(&self, id: VehicleId) -> u64 {
        self.sim.vehicle(id).unwrap().wait_steps()
    }

    fn stopped(&self, id: VehicleId) -> bool {
        self.sim.vehicle(id).unwrap().is_stopped()
    }

    fn summary(&self) -> EpisodeSummary {
        self.metrics.summary(&self.sim)
    }
}

fn north_lane(sim: &Simulation, i: usize) -> LaneId {
    sim.scenario()
        .intersection()
        .approach(Direction::North)
        .lanes()[i]
        .id()
}

// ---- Per-vehicle wait (R7.2) ----

#[test]
fn lone_vehicle_under_green_never_waits() {
    let mut rig = Rig::through();
    let v = rig.spawn(Direction::North, Movement::Through);
    for _ in 0..4 {
        rig.step(Command::Hold);
        assert_eq!(rig.wait(v), 0);
        assert!(!rig.stopped(v));
    }
    let report = rig.step(Command::Hold);
    assert_eq!(report.departures.len(), 1);
    assert_eq!(report.departures[0].wait_steps, 0);
    assert_eq!(report.departures[0].departed_at, 5);
}

#[test]
fn vehicle_at_a_red_light_waits_one_step_per_red_step() {
    let mut rig = Rig::through();
    let v = rig.spawn(Direction::East, Movement::Through);
    // Enters at t=1 and reaches the stop-line cell at t=4 without waiting.
    rig.run(4);
    assert_eq!(rig.wait(v), 0);
    assert!(!rig.stopped(v));
    // East is red: every further step adds one.
    for k in 1..=6 {
        rig.step(Command::Hold);
        assert_eq!(rig.wait(v), k);
        assert!(rig.stopped(v));
    }
    // Ask for east's phase and wait for the crossing. It departs at `departed_at`
    // having moved exactly 5 times, so it waited `departed_at - 5` steps.
    let mut counted = 6;
    let departure = loop {
        let report = rig.step(Command::SwitchTo(EAST_PHASE));
        if let Some(d) = report.departures.first() {
            break *d;
        }
        counted += 1;
        assert_eq!(rig.wait(v), counted);
    };
    assert_eq!(departure.vehicle, v);
    assert_eq!(departure.wait_steps, counted);
    assert_eq!(departure.departed_at, 5 + counted);
}

#[test]
fn backlog_and_queue_wait() {
    let mut rig = Rig::through();
    let a = rig.spawn(Direction::East, Movement::Through);
    let b = rig.spawn(Direction::East, Movement::Through);

    // Step 1: `a` enters cell 0, `b` stays in the backlog.
    rig.step(Command::Hold);
    assert_eq!((rig.wait(a), rig.stopped(a)), (0, false));
    assert_eq!((rig.wait(b), rig.stopped(b)), (1, true));
    // Step 2: cell 0 was full at the start of the step, so `b` waits again.
    rig.step(Command::Hold);
    assert_eq!((rig.wait(b), rig.stopped(b)), (2, true));
    // Step 3: it enters, moving.
    rig.step(Command::Hold);
    assert_eq!((rig.wait(b), rig.stopped(b)), (2, false));
    // `a` reaches the stop-line cell at t=4 and `b` cell 1. At step 5 `a` is held by the
    // red light (its first wait) while `b` still advances, to cell 2.
    rig.run(2);
    assert_eq!((rig.wait(a), rig.stopped(a)), (1, true));
    assert_eq!((rig.wait(b), rig.stopped(b)), (2, false));
    // Step 6: `b` is now blocked by `a`.
    rig.step(Command::Hold);
    assert_eq!((rig.wait(a), rig.stopped(a)), (2, true));
    assert_eq!((rig.wait(b), rig.stopped(b)), (3, true));
    rig.step(Command::Hold);
    assert_eq!((rig.wait(a), rig.wait(b)), (3, 4));
}

#[test]
fn spawned_vehicle_is_not_stopped_until_it_fails_to_move() {
    let mut rig = Rig::through();
    let a = rig.spawn(Direction::East, Movement::Through);
    rig.run(8);
    assert!(rig.stopped(a));
    let fresh = rig.spawn(Direction::East, Movement::Through);
    assert_eq!((rig.wait(fresh), rig.stopped(fresh)), (0, false));
    rig.step(Command::Hold);
    // It enters cell 0, moving.
    assert_eq!((rig.wait(fresh), rig.stopped(fresh)), (0, false));
}

// ---- The collector (R7.3) ----

#[test]
fn hand_built_episode_gives_the_expected_wait_stats() {
    let mut rig = Rig::through();
    let n = rig.spawn(Direction::North, Movement::Through);
    let e = rig.spawn(Direction::East, Movement::Through);
    rig.run(10);
    // `n` crossed at t=5 without waiting.
    assert_eq!(rig.departures, [(n, 5, 0)]);
    while rig.sim.vehicle(e).is_some() {
        rig.step(Command::SwitchTo(EAST_PHASE));
    }
    let (_, departed_at, wait) = rig.departures[1];
    assert_eq!(wait, departed_at - 5);
    assert!(wait >= 6);

    let s = rig.summary();
    assert_eq!((s.departed, s.in_system), (2, 0));
    let all = s.wait.unwrap();
    // n = 2: p50 is rank 1, and p95, p99 and max are rank 2.
    let w = wait as f64;
    assert_eq!(all.count, 2);
    assert!((all.mean_s - w / 2.0).abs() < 1e-12);
    assert_eq!((all.p50_s, all.p95_s, all.p99_s, all.max_s), (0.0, w, w, w));

    let north = s.wait_by_approach[Direction::North.index()]
        .as_ref()
        .unwrap();
    assert_eq!(north.count, 1);
    assert_eq!(
        (north.mean_s, north.p50_s, north.p99_s, north.max_s),
        (0.0, 0.0, 0.0, 0.0)
    );
    let east = s.wait_by_approach[Direction::East.index()]
        .as_ref()
        .unwrap();
    assert_eq!(
        (east.count, east.mean_s, east.p50_s, east.max_s),
        (1, w, w, w)
    );
    assert!(s.wait_by_approach[Direction::South.index()].is_none());
    assert!(s.wait_by_approach[Direction::West.index()].is_none());
}

#[test]
fn vehicles_still_in_the_model_count_with_their_wait_so_far() {
    let mut rig = Rig::through();
    rig.spawn(Direction::North, Movement::Through);
    let e = rig.spawn(Direction::East, Movement::Through);
    rig.run(20);
    let s = rig.summary();
    assert_eq!((s.departed, s.in_system), (1, 1));
    assert_eq!(rig.wait(e), 16);
    let all = s.wait.unwrap();
    assert_eq!(all.count, 2);
    // The departed vehicle waited 0, so the starved one alone sets the max.
    assert_eq!(all.max_s, 16.0);
    assert_eq!(all.p99_s, 16.0);
    let east = s.wait_by_approach[Direction::East.index()]
        .as_ref()
        .unwrap();
    assert_eq!((east.count, east.max_s), (1, 16.0));
}

/// `r0` and `r1` (right turns only fit the second lane) queue in the backlog of lane 1
/// early on. Later `t0` and `t2` share lane 0, so lane 0 queues at other steps.
fn two_lane_rig() -> Rig {
    let scenario = inline(
        r#"[{ movements = ["through"] }, { movements = ["through", "right"] }]"#,
        PHASES_THROUGH_RIGHT,
    );
    let mut rig = Rig::new(scenario);
    rig.spawn(Direction::North, Movement::Right);
    rig.spawn(Direction::North, Movement::Right);
    rig.run(20);
    let t0 = rig.spawn(Direction::North, Movement::Through);
    let t1 = rig.spawn(Direction::North, Movement::Through);
    let t2 = rig.spawn(Direction::North, Movement::Through);
    assert_eq!(rig.sim.vehicle(t0).unwrap().lane(), north_lane(&rig.sim, 0));
    assert_eq!(rig.sim.vehicle(t1).unwrap().lane(), north_lane(&rig.sim, 1));
    assert_eq!(rig.sim.vehicle(t2).unwrap().lane(), north_lane(&rig.sim, 0));
    rig.run(10);
    rig
}

#[test]
fn queue_mean_and_max_on_a_traced_lane() {
    let rig = two_lane_rig();
    let s = rig.summary();
    // Lane 1: `r1` is stopped in the backlog after steps 1 and 2. Lane 0: `t2` is stopped
    // after steps 21 and 22. Nothing else ever stops (north stays green).
    let by_lane: Vec<_> = s
        .queue_by_lane
        .iter()
        .map(|(id, q)| (*id, q.clone()))
        .collect();
    assert_eq!(by_lane.len(), 5);
    for (id, q) in &by_lane {
        let (mean, max) = if id.approach() == Direction::North {
            (2.0 / 30.0, 1)
        } else {
            (0.0, 0)
        };
        assert!((q.mean - mean).abs() < 1e-12, "{id:?}: mean {}", q.mean);
        assert_eq!(q.max, max, "{id:?}");
    }
    assert_eq!(by_lane[0].0, north_lane(&rig.sim, 0));
    assert_eq!(by_lane[1].0, north_lane(&rig.sim, 1));

    // The two lanes never queue at the same step, so the approach's max is the max of
    // the sums (1), not the sum of the maxes (2).
    let north = &s.queue_by_approach[Direction::North.index()];
    assert_eq!(north.max, 1);
    assert!((north.mean - 4.0 / 30.0).abs() < 1e-12);
    assert_eq!(s.queue_by_approach[Direction::East.index()].max, 0);
    assert_eq!(s.queue_total.max, 1);
    assert!((s.queue_total.mean - 4.0 / 30.0).abs() < 1e-12);
}

#[test]
fn queue_grows_behind_a_red_light() {
    let mut rig = Rig::through();
    rig.spawn(Direction::East, Movement::Through);
    rig.spawn(Direction::East, Movement::Through);
    rig.run(8);
    // Stopped vehicles after each step: 1, 1, 0, 0, 1, 2, 2, 2.
    let s = rig.summary();
    let east = &s.queue_by_approach[Direction::East.index()];
    assert_eq!(east.max, 2);
    assert!((east.mean - 9.0 / 8.0).abs() < 1e-12);
    assert_eq!(s.queue_total, *east);
}

#[test]
fn throughput_and_duration() {
    let rig = two_lane_rig();
    let s = rig.summary();
    assert_eq!(s.steps, 30);
    assert_eq!(s.duration_s, 30.0);
    assert_eq!((s.departed, s.in_system), (5, 0));
    assert!((s.throughput_veh_per_h - 600.0).abs() < 1e-9);
}

#[test]
fn a_summary_of_zero_steps() {
    let mut rig = Rig::through();
    let s = rig.summary();
    assert_eq!((s.steps, s.departed, s.in_system), (0, 0, 0));
    assert_eq!((s.duration_s, s.throughput_veh_per_h), (0.0, 0.0));
    assert!(s.wait.is_none());
    assert!(s.wait_by_approach.iter().all(Option::is_none));
    assert_eq!(s.queue_total.mean, 0.0);
    assert_eq!(s.queue_total.max, 0);
    assert!(
        s.queue_by_lane
            .iter()
            .all(|(_, q)| q.mean == 0.0 && q.max == 0)
    );

    // A vehicle in the model still counts, with no step observed.
    rig.spawn(Direction::North, Movement::Through);
    let s = rig.summary();
    assert_eq!((s.steps, s.in_system, s.throughput_veh_per_h), (0, 1, 0.0));
    assert_eq!(s.wait.unwrap().count, 1);
}

#[test]
fn a_skipped_or_repeated_step_is_refused_and_changes_nothing() {
    let mut rig = Rig::through();
    rig.spawn(Direction::East, Movement::Through);
    rig.run(3);
    // Repeat: the same step again.
    let report = rig.sim.step(Command::Hold); // step 4, observed below
    rig.metrics.observe(&rig.sim, &report).unwrap();
    let after_four = rig.summary();
    let err = rig.metrics.observe(&rig.sim, &report).unwrap_err();
    assert_eq!(
        err,
        MetricsError::StepMismatch {
            expected: 5,
            found: 4
        }
    );
    assert_eq!(rig.summary(), after_four);

    // Skip: two steps between observations.
    rig.sim.step(Command::Hold);
    let report = rig.sim.step(Command::Hold);
    let err = rig.metrics.observe(&rig.sim, &report).unwrap_err();
    assert_eq!(
        err,
        MetricsError::StepMismatch {
            expected: 5,
            found: 6
        }
    );
    assert_eq!(rig.summary().steps, 4);
}

#[test]
fn summary_is_repeatable() {
    let mut rig = Rig::through();
    rig.spawn(Direction::East, Movement::Through);
    rig.spawn(Direction::North, Movement::Through);
    rig.run(12);
    assert_eq!(rig.summary(), rig.summary());
    let first = rig.summary();
    rig.run(3);
    assert_ne!(rig.summary(), first);
}

#[test]
fn a_forked_episode_keeps_its_statistics() {
    let mut rig = Rig::through();
    rig.spawn(Direction::East, Movement::Through);
    rig.run(10);
    let fork = rig.metrics.clone();
    rig.run(5);
    assert_eq!(fork.summary(&rig.sim).steps, 10);
    assert_eq!(rig.summary().steps, 15);
}
