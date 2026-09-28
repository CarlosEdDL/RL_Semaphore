//! Conversions from a real `Simulation` agree with it at every step of a run of the example
//! scenario (R8.6), and the wire format of the example's `Hello`, and its `Snapshot` and
//! `Metrics` at one fixed step, are pinned by golden snapshots (R8.7).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rl_semaphore_protocol::{Hello, Layout, Metrics, ServerMessage, Snapshot};
use rl_semaphore_sim::{
    Command, Demand, Direction, EpisodeMetrics, Light, Movement, MovementId, PhaseId, Scenario,
    Simulation,
};

const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// Seed for the demand generator.
const SEED: u64 = 2024;
/// Number of steps to run.
const STEPS: u64 = 600;
/// Steps between phase-rotation requests: comfortably above min-green (5 s) plus the
/// yellow/all-red clearance (3 s + 2 s), so each request has time to be accepted and to
/// complete before the next one is issued.
const ROTATE_EVERY: u64 = 20;
/// The step at which the golden snapshot and metrics are pinned, chosen (by inspection of a
/// run with this seed and command sequence) to have vehicles on lanes and a non-empty backlog.
const PINNED_STEP: u64 = 387;

/// Requests the phase for the current `ROTATE_EVERY`-step bucket, rotating through every phase
/// in turn. Repeating the same request once a phase is already green is harmless (the signal
/// ignores it as `AlreadyOnPhase`).
fn command_at(t: u64, phases: usize) -> Command {
    #[allow(clippy::cast_possible_truncation)]
    // INVARIANT: `phases` is at most `MAX_PHASES` (8), so the modulo result fits `usize`.
    let target = ((t / ROTATE_EVERY) % phases as u64) as usize;
    Command::SwitchTo(PhaseId::new(target))
}

fn assert_finite_snapshot(s: &Snapshot) {
    assert!(s.time_s.is_finite());
    let elapsed_s = match s.signal.state {
        rl_semaphore_protocol::SignalStateView::Green { elapsed_s, .. }
        | rl_semaphore_protocol::SignalStateView::Yellow { elapsed_s, .. }
        | rl_semaphore_protocol::SignalStateView::AllRed { elapsed_s, .. } => elapsed_s,
    };
    assert!(elapsed_s.is_finite());
    for v in &s.vehicles {
        assert!(v.wait_s.is_finite());
    }
}

fn assert_finite_metrics(m: &Metrics) {
    assert!(m.time_s.is_finite());
    let check_wait = |w: &Option<rl_semaphore_protocol::WaitStats>| {
        if let Some(w) = w {
            assert!(w.mean_s.is_finite());
            assert!(w.p50_s.is_finite());
            assert!(w.p95_s.is_finite());
            assert!(w.p99_s.is_finite());
            assert!(w.max_s.is_finite());
        }
    };
    check_wait(&m.summary.wait);
    for d in Direction::ALL {
        check_wait(m.summary.wait_by_approach.get(d.into()));
    }
    assert!(m.summary.throughput_veh_per_h.is_finite());
    assert!(m.summary.duration_s.is_finite());
}

/// Checks `snapshot` (built by [`Snapshot::from_sim`]) against `sim` and `layout` (R8.6).
fn check_snapshot(sim: &Simulation, layout: &Layout, snapshot: &Snapshot) {
    assert_eq!(snapshot.step, sim.step_count());
    assert_eq!(snapshot.counts.spawned, sim.spawned_count());
    assert_eq!(snapshot.counts.backlog, sim.backlog_count());
    assert_eq!(snapshot.counts.on_lane, sim.on_lane_count());
    assert_eq!(snapshot.counts.departed, sim.departed_count());
    assert_eq!(snapshot.vehicles.len() as u64, snapshot.counts.on_lane);

    let backlog_sum: u64 = snapshot.lanes.iter().map(|l| l.backlog).sum();
    assert_eq!(backlog_sum, snapshot.counts.backlog);

    let mut last_id: Option<u64> = None;
    for v in &snapshot.vehicles {
        if let Some(last) = last_id {
            assert!(v.id > last, "vehicle ids must be strictly ascending");
        }
        last_id = Some(v.id);

        let approach = layout.approaches.get(v.lane.approach);
        let lane = approach
            .lanes
            .iter()
            .find(|l| l.index == v.lane.index)
            .unwrap_or_else(|| panic!("vehicle {} is on an unknown lane {:?}", v.id, v.lane));
        assert!(
            v.cell < lane.len_cells,
            "vehicle {} cell out of range",
            v.id
        );
        assert!(
            lane.movements.contains(&v.movement),
            "vehicle {} movement {:?} not allowed by its lane",
            v.id,
            v.movement
        );
    }

    let lights = sim.lights();
    for d in Direction::ALL {
        let expected = |m: Movement| {
            let want = lights[MovementId::new(d, m).index()];
            match want {
                Light::Red => rl_semaphore_protocol::Light::Red,
                Light::Yellow => rl_semaphore_protocol::Light::Yellow,
                Light::Green => rl_semaphore_protocol::Light::Green,
            }
        };
        let got = snapshot.signal.lights.get(d.into());
        assert_eq!(got.left, expected(Movement::Left));
        assert_eq!(got.through, expected(Movement::Through));
        assert_eq!(got.right, expected(Movement::Right));
    }

    assert_finite_snapshot(snapshot);
}

/// Checks `wire` (built by [`Metrics::from_sim`]) against `summary` and `sim` (R8.6).
fn check_metrics(sim: &Simulation, summary: &rl_semaphore_sim::EpisodeSummary, wire: &Metrics) {
    assert_eq!(wire.step, sim.step_count());
    assert_eq!(wire.summary.steps, summary.steps);
    assert_eq!(wire.summary.duration_s, summary.duration_s);
    assert_eq!(wire.summary.departed, summary.departed);
    assert_eq!(wire.summary.in_system, summary.in_system);
    assert_eq!(
        wire.summary.throughput_veh_per_h,
        summary.throughput_veh_per_h
    );

    let wait_eq = |a: &Option<rl_semaphore_protocol::WaitStats>,
                   b: &Option<rl_semaphore_sim::WaitStats>| {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.count == b.count
                    && a.mean_s == b.mean_s
                    && a.p50_s == b.p50_s
                    && a.p95_s == b.p95_s
                    && a.p99_s == b.p99_s
                    && a.max_s == b.max_s
            }
            _ => false,
        }
    };
    assert!(wait_eq(&wire.summary.wait, &summary.wait));
    for d in Direction::ALL {
        assert!(wait_eq(
            wire.summary.wait_by_approach.get(d.into()),
            &summary.wait_by_approach[d.index()]
        ));
    }

    let stopped_total: u64 = sim.vehicles().filter(|v| v.is_stopped()).count() as u64;
    let queue_now_sum: u64 = wire.queue_now.iter().map(|q| q.stopped).sum();
    assert_eq!(queue_now_sum, stopped_total);

    assert_finite_metrics(wire);
}

#[test]
fn from_sim_matches_the_simulation_at_every_step() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let layout = Layout::from_scenario(&scenario);
    let phases = scenario.signal_plan().phases().len();
    let mut demand = Demand::new(&scenario, SEED);
    let mut sim = Simulation::new(scenario);
    let mut metrics = EpisodeMetrics::new(&sim);

    let mut seen_green = false;
    let mut seen_yellow = false;
    let mut seen_all_red = false;

    for _ in 0..STEPS {
        let t = sim.step_count();
        for m in demand.arrivals() {
            sim.spawn(m.approach, m.movement).unwrap();
        }
        let report = sim.step(command_at(t, phases));
        metrics.observe(&sim, &report).unwrap();

        match sim.signal().state() {
            rl_semaphore_sim::SignalState::Green { .. } => seen_green = true,
            rl_semaphore_sim::SignalState::Yellow { .. } => seen_yellow = true,
            rl_semaphore_sim::SignalState::AllRed { .. } => seen_all_red = true,
        }

        let snapshot = Snapshot::from_sim(&sim);
        check_snapshot(&sim, &layout, &snapshot);

        let summary = metrics.summary(&sim);
        let wire_metrics = Metrics::from_sim(&summary, &sim);
        check_metrics(&sim, &summary, &wire_metrics);

        for msg in [
            ServerMessage::Snapshot(snapshot),
            ServerMessage::Metrics(wire_metrics),
        ] {
            let text = msg.to_json().unwrap();
            let decoded = ServerMessage::from_json(&text).unwrap();
            assert_eq!(decoded, msg);
        }
    }

    assert!(seen_green, "the run never saw a green state");
    assert!(seen_yellow, "the run never saw a yellow state");
    assert!(seen_all_red, "the run never saw an all-red state");
}

#[test]
#[allow(clippy::too_many_lines)]
fn golden_wire_format_of_the_example() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let hello = Hello::new(Layout::from_scenario(&scenario));
    insta::assert_snapshot!(
        "from_sim_hello",
        serde_json::to_string_pretty(&hello).unwrap()
    );

    let mut demand = Demand::new(&scenario, SEED);
    let mut sim = Simulation::new(scenario);
    let mut metrics = EpisodeMetrics::new(&sim);
    let phases = sim.scenario().signal_plan().phases().len();

    for _ in 0..PINNED_STEP {
        let t = sim.step_count();
        for m in demand.arrivals() {
            sim.spawn(m.approach, m.movement).unwrap();
        }
        let report = sim.step(command_at(t, phases));
        metrics.observe(&sim, &report).unwrap();
    }

    let snapshot = Snapshot::from_sim(&sim);
    assert!(
        !snapshot.vehicles.is_empty(),
        "pick a PINNED_STEP with vehicles on lanes"
    );
    assert!(
        snapshot.lanes.iter().any(|l| l.backlog > 0),
        "pick a PINNED_STEP with a non-empty backlog"
    );
    insta::assert_snapshot!(
        "from_sim_snapshot",
        serde_json::to_string_pretty(&snapshot).unwrap()
    );

    let summary = metrics.summary(&sim);
    let wire_metrics = Metrics::from_sim(&summary, &sim);
    insta::assert_snapshot!(
        "from_sim_metrics",
        serde_json::to_string_pretty(&wire_metrics).unwrap()
    );
}
