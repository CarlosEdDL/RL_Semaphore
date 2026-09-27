//! Shared helpers for the vehicle tests: inline scenarios, per-step snapshots and
//! the invariants of requirement R6.2 written as functions over snapshots.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use rl_semaphore_sim::{
    Command, Demand, Departure, LaneId, Light, MovementId, Position, Scenario, Simulation,
    StepReport, VehicleId,
};

pub const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// A small scenario: four approaches of 4 cells (`step_s = 1`, 7.5 m cells).
/// South, east and west have one through lane each. `north_lanes` is the TOML
/// array of the north approach, and `phases` the `[[signal.phases]]` tables.
/// Yellow lasts 2 steps, there is no all-red, min green is 1 step.
pub fn inline(north_lanes: &str, phases: &str) -> Scenario {
    let text = format!(
        r#"
step_s = 1.0

[intersection]
cell_length_m = 7.5

[intersection.approaches.north]
length_m = 30.0
lanes = {north_lanes}

[intersection.approaches.south]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[intersection.approaches.east]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[intersection.approaches.west]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[signal]
yellow_s = 2.0
all_red_s = 0.0
min_green_s = 1.0
max_red_s = 200.0
{phases}
"#
    );
    Scenario::from_toml_str(&text).unwrap()
}

/// Phases for [`inline`] where north has through (and, if its lanes allow it, right) traffic.
pub const PHASES_THROUGH: &str = r#"
[[signal.phases]]
name = "ns"
green = { north = ["through"], south = ["through"] }

[[signal.phases]]
name = "east"
green = { east = ["through"] }

[[signal.phases]]
name = "west"
green = { west = ["through"] }
"#;

/// Like [`PHASES_THROUGH`], for a north approach that also allows right turns.
pub const PHASES_THROUGH_RIGHT: &str = r#"
[[signal.phases]]
name = "ns"
green = { north = ["through", "right"], south = ["through"] }

[[signal.phases]]
name = "east"
green = { east = ["through"] }

[[signal.phases]]
name = "west"
green = { west = ["through"] }
"#;

/// Phases for [`inline`] where a north lane allows left and through traffic,
/// green in different phases (the shared-lane case).
pub const PHASES_SHARED: &str = r#"
[[signal.phases]]
name = "ns"
green = { north = ["through"], south = ["through"] }

[[signal.phases]]
name = "north-left"
green = { north = ["left"] }

[[signal.phases]]
name = "east"
green = { east = ["through"] }

[[signal.phases]]
name = "west"
green = { west = ["through"] }
"#;

/// The scenario with a north lane shared by left and through, split across phases.
pub fn shared_lane() -> Scenario {
    inline(r#"[{ movements = ["left", "through"] }]"#, PHASES_SHARED)
}

/// Every movement some lane of the scenario allows.
pub fn allowed_movements(scenario: &Scenario) -> Vec<MovementId> {
    let mut out = Vec::new();
    for approach in scenario.intersection().approaches() {
        for movement in rl_semaphore_sim::Movement::ALL {
            if approach
                .lanes()
                .iter()
                .any(|l| l.movements().contains(&movement))
            {
                out.push(MovementId::new(approach.direction(), movement));
            }
        }
    }
    out
}

/// One lane as recorded in a snapshot.
#[derive(Debug, Clone)]
pub struct LaneSnap {
    pub id: LaneId,
    pub last: u32,
    pub cells: Vec<Option<VehicleId>>,
    pub backlog: Vec<VehicleId>,
}

/// Everything the invariants need to know about the simulation at one step.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub t: u64,
    pub vehicles: BTreeMap<VehicleId, (LaneId, MovementId, Position)>,
    pub lights: [Light; MovementId::COUNT],
    pub spawned: u64,
    pub in_backlog: u64,
    pub on_lanes: u64,
    pub departed: u64,
    pub lanes: Vec<LaneSnap>,
}

impl Snapshot {
    pub fn of(sim: &Simulation) -> Self {
        let mut lanes = Vec::new();
        for approach in sim.scenario().intersection().approaches() {
            for lane in approach.lanes() {
                lanes.push(LaneSnap {
                    id: lane.id(),
                    last: lane.stop_line_cell(),
                    cells: sim.lane_cells(lane.id()).unwrap().to_vec(),
                    backlog: sim.backlog(lane.id()).unwrap().collect(),
                });
            }
        }
        Self {
            t: sim.step_count(),
            vehicles: sim
                .vehicles()
                .map(|v| (v.id(), (v.lane(), v.movement(), v.position())))
                .collect(),
            lights: sim.lights(),
            spawned: sim.spawned_count(),
            in_backlog: sim.backlog_count(),
            on_lanes: sim.on_lane_count(),
            departed: sim.departed_count(),
            lanes,
        }
    }
}

/// Conservation, and the counters agree with what is actually stored.
pub fn check_conservation(s: &Snapshot) {
    assert_eq!(
        s.spawned,
        s.in_backlog + s.on_lanes + s.departed,
        "t={}: spawned != backlog + lanes + departed",
        s.t
    );
    let (mut backlog, mut on_lane) = (0u64, 0u64);
    for (_, _, pos) in s.vehicles.values() {
        match pos {
            Position::Backlog => backlog += 1,
            Position::OnLane { .. } => on_lane += 1,
        }
    }
    assert_eq!(backlog, s.in_backlog, "t={}: backlog counter", s.t);
    assert_eq!(on_lane, s.on_lanes, "t={}: on-lane counter", s.t);
}

/// Cells and backlogs agree with vehicle positions, and no cell holds two vehicles.
pub fn check_layout(s: &Snapshot) {
    let mut seen = BTreeSet::new();
    for lane in &s.lanes {
        assert_eq!(
            lane.cells.len(),
            lane.last as usize + 1,
            "t={}: lane length",
            s.t
        );
        for (i, cell) in lane.cells.iter().enumerate() {
            let Some(id) = cell else { continue };
            assert!(seen.insert(*id), "t={}: {id} appears twice", s.t);
            let (l, _, pos) = s.vehicles[id];
            assert_eq!(l, lane.id, "t={}: {id} on the wrong lane", s.t);
            assert_eq!(
                pos,
                Position::OnLane {
                    cell: u32::try_from(i).unwrap()
                },
                "t={}: {id} position disagrees with its cell",
                s.t
            );
        }
        for id in &lane.backlog {
            assert!(seen.insert(*id), "t={}: {id} appears twice", s.t);
            let (l, _, pos) = s.vehicles[id];
            assert_eq!(l, lane.id, "t={}: {id} in the wrong backlog", s.t);
            assert_eq!(pos, Position::Backlog, "t={}: {id} backlog position", s.t);
        }
    }
    assert_eq!(
        seen.len(),
        s.vehicles.len(),
        "t={}: vehicle not in any lane",
        s.t
    );
}

/// Movement between two consecutive snapshots (spawns already applied to `before`).
pub fn check_transition(before: &Snapshot, after: &Snapshot, report: &StepReport) {
    assert_eq!(after.t, before.t + 1);
    assert_eq!(after.spawned, before.spawned, "step must not spawn");
    assert_eq!(
        after.departed - before.departed,
        report.departures.len() as u64,
        "t={}: departure counter",
        after.t
    );

    for (id, (lane, movement, pos)) in &after.vehicles {
        let (l0, m0, p0) = before.vehicles[id];
        assert_eq!(
            (l0, m0),
            (*lane, *movement),
            "{id} changed lane or movement"
        );
        match (p0, *pos) {
            (Position::OnLane { cell: a }, Position::OnLane { cell: b }) => {
                assert!(b == a || b == a + 1, "t={}: {id} moved {a} -> {b}", after.t);
            }
            (Position::Backlog, Position::Backlog | Position::OnLane { cell: 0 }) => {}
            (p0, p1) => panic!("t={}: {id} went from {p0:?} to {p1:?}", after.t),
        }
    }

    let gone: BTreeSet<_> = before
        .vehicles
        .keys()
        .filter(|id| !after.vehicles.contains_key(id))
        .copied()
        .collect();
    let departed: BTreeSet<_> = report.departures.iter().map(|d| d.vehicle).collect();
    assert_eq!(
        gone, departed,
        "t={}: vanished vehicles are not the departures",
        after.t
    );

    for d in &report.departures {
        check_departure(before, after, d);
    }
    for (i, a) in report.departures.iter().enumerate() {
        for b in &report.departures[i + 1..] {
            assert!(
                !a.movement.conflicts_with(b.movement),
                "t={}: conflicting departures {} and {}",
                after.t,
                a.movement,
                b.movement
            );
        }
    }
}

fn check_departure(before: &Snapshot, after: &Snapshot, d: &Departure) {
    let (lane, movement, pos) = before.vehicles[&d.vehicle];
    let last = before.lanes.iter().find(|l| l.id == lane).unwrap().last;
    assert_eq!(
        pos,
        Position::OnLane { cell: last },
        "{} left from a cell that is not the stop line",
        d.vehicle
    );
    assert_eq!(
        after.lights[movement.index()],
        Light::Green,
        "{} crossed on {:?}",
        d.vehicle,
        after.lights[movement.index()]
    );
    assert_eq!((d.lane, d.movement), (lane, movement));
    assert_eq!(d.departed_at, after.t);
    assert!(d.spawned_at <= before.t);
}

/// No overtaking: along each lane (stop line back to cell 0, then the backlog)
/// ids increase, and vehicles depart from a lane in increasing id order.
/// `last_departed` carries the newest departure per lane between calls.
pub fn check_fifo(
    after: &Snapshot,
    report: &StepReport,
    last_departed: &mut BTreeMap<LaneId, VehicleId>,
) {
    for lane in &after.lanes {
        let order: Vec<VehicleId> = lane
            .cells
            .iter()
            .rev()
            .flatten()
            .copied()
            .chain(lane.backlog.iter().copied())
            .collect();
        assert!(
            order.windows(2).all(|w| w[0] < w[1]),
            "t={}: {:?} is not in FIFO order",
            after.t,
            order
        );
        if let (Some(prev), Some(front)) = (last_departed.get(&lane.id), order.first()) {
            assert!(
                prev < front,
                "t={}: {front} overtook departed {prev}",
                after.t
            );
        }
    }
    for d in &report.departures {
        if let Some(prev) = last_departed.insert(d.lane, d.vehicle) {
            assert!(
                prev < d.vehicle,
                "t={}: {} departed before {prev}",
                after.t,
                d.vehicle
            );
        }
    }
}

/// Runs every invariant for one step. `before` must be taken after the step's spawns.
pub fn check_step(
    before: &Snapshot,
    after: &Snapshot,
    report: &StepReport,
    last_departed: &mut BTreeMap<LaneId, VehicleId>,
) {
    check_conservation(after);
    check_layout(after);
    check_transition(before, after, report);
    check_fifo(after, report, last_departed);
}

/// What happened in one step of [`run_with_demand`].
#[derive(Debug, Clone)]
pub struct StepLog {
    /// The step count before the step.
    pub t: u64,
    /// The arrivals spawned before the step, with their vehicle ids.
    pub arrivals: Vec<(VehicleId, MovementId)>,
    /// The vehicles that crossed during the step.
    pub departures: Vec<VehicleId>,
}

/// Runs the loop of R5.1 for `steps` steps: draw arrivals, spawn them, step with
/// `command(t)`, and check every 1.3 invariant on the way.
pub fn run_with_demand(
    sim: &mut Simulation,
    demand: &mut Demand,
    steps: usize,
    mut command: impl FnMut(u64) -> Command,
) -> Vec<StepLog> {
    let mut last_departed = BTreeMap::new();
    let mut log = Vec::with_capacity(steps);
    for _ in 0..steps {
        let t = sim.step_count();
        let arrivals: Vec<_> = demand
            .arrivals()
            .into_iter()
            .map(|m| (sim.spawn(m.approach, m.movement).unwrap(), m))
            .collect();
        let before = Snapshot::of(sim);
        let report = sim.step(command(t));
        check_step(&before, &Snapshot::of(sim), &report, &mut last_departed);
        log.push(StepLog {
            t,
            arrivals,
            departures: report.departures.iter().map(|d| d.vehicle).collect(),
        });
    }
    log
}
