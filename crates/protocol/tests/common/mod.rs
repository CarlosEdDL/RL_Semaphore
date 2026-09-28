//! Fixture builders shared by the round-trip, golden and decoding-rule tests: a small layout, a
//! snapshot in each signal state, and metrics with a mix of `None`/`Some` waits.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use rl_semaphore_protocol::{
    ApproachLayout, ApproachLights, ByApproach, Direction, Hello, LaneLayout, LaneQueueNow,
    LaneQueueStats, LaneState, Layout, Light, Metrics, Movement, MovementRef, PhaseLayout,
    QueueStats, ServerMessage, SignalLayout, SignalStateView, SignalView, Snapshot, Summary,
    VehicleCounts, VehicleView, WaitStats,
};

/// A small four-approach layout: north/south have a left-turn lane and a through+right lane,
/// east/west have one shared lane. Two phases: `ns` and `ew`.
pub fn layout() -> Layout {
    let ns_approach = |length_m: f64| ApproachLayout {
        length_m,
        lanes: vec![
            LaneLayout {
                index: 0,
                len_cells: 4,
                movements: vec![Movement::Left],
            },
            LaneLayout {
                index: 1,
                len_cells: 4,
                movements: vec![Movement::Through, Movement::Right],
            },
        ],
    };
    let ew_approach = |length_m: f64| ApproachLayout {
        length_m,
        lanes: vec![LaneLayout {
            index: 0,
            len_cells: 4,
            movements: vec![Movement::Left, Movement::Through, Movement::Right],
        }],
    };
    let movement_ref = |approach: Direction, movement: Movement| MovementRef { approach, movement };
    Layout {
        step_s: 1.0,
        cell_length_m: 7.5,
        approaches: ByApproach {
            north: ns_approach(30.0),
            east: ew_approach(30.0),
            south: ns_approach(30.0),
            west: ew_approach(30.0),
        },
        signal: SignalLayout {
            yellow_s: 3.0,
            all_red_s: 2.0,
            min_green_s: 5.0,
            max_red_s: 30.0,
            phases: vec![
                PhaseLayout {
                    name: "ns".to_owned(),
                    green: vec![
                        movement_ref(Direction::North, Movement::Left),
                        movement_ref(Direction::North, Movement::Through),
                        movement_ref(Direction::North, Movement::Right),
                        movement_ref(Direction::South, Movement::Left),
                        movement_ref(Direction::South, Movement::Through),
                        movement_ref(Direction::South, Movement::Right),
                    ],
                },
                PhaseLayout {
                    name: "ew".to_owned(),
                    green: vec![
                        movement_ref(Direction::East, Movement::Left),
                        movement_ref(Direction::East, Movement::Through),
                        movement_ref(Direction::East, Movement::Right),
                        movement_ref(Direction::West, Movement::Left),
                        movement_ref(Direction::West, Movement::Through),
                        movement_ref(Direction::West, Movement::Right),
                    ],
                },
            ],
        },
    }
}

/// Lights for `ns` green: north/south green, east/west red.
fn ns_green_lights() -> ByApproach<ApproachLights> {
    let green = ApproachLights {
        left: Light::Green,
        through: Light::Green,
        right: Light::Green,
    };
    let red = ApproachLights {
        left: Light::Red,
        through: Light::Red,
        right: Light::Red,
    };
    ByApproach {
        north: green,
        east: red,
        south: green,
        west: red,
    }
}

/// One vehicle on a lane.
pub fn vehicle(
    id: u64,
    approach: Direction,
    lane: u32,
    cell: u32,
    movement: Movement,
) -> VehicleView {
    VehicleView {
        id,
        lane: rl_semaphore_protocol::LaneRef {
            approach,
            index: lane,
        },
        cell,
        movement,
        #[allow(clippy::cast_precision_loss)]
        wait_s: id as f64,
        stopped: id.is_multiple_of(2),
    }
}

/// One lane's backlog.
pub fn lane_state(approach: Direction, index: u32, backlog: u64) -> LaneState {
    LaneState {
        lane: rl_semaphore_protocol::LaneRef { approach, index },
        backlog,
    }
}

/// The layout's lanes, all with an empty backlog.
fn empty_lanes() -> Vec<LaneState> {
    vec![
        lane_state(Direction::North, 0, 0),
        lane_state(Direction::North, 1, 0),
        lane_state(Direction::East, 0, 0),
        lane_state(Direction::South, 0, 0),
        lane_state(Direction::South, 1, 0),
        lane_state(Direction::West, 0, 0),
    ]
}

/// A snapshot in the given state, with no vehicles and no backlog.
pub fn snapshot_empty(state: SignalStateView) -> Snapshot {
    Snapshot {
        step: 42,
        episode: 0,
        seed: 7,
        time_s: 42.0,
        signal: SignalView {
            state,
            lights: ns_green_lights(),
        },
        counts: VehicleCounts {
            spawned: 0,
            backlog: 0,
            on_lane: 0,
            departed: 0,
        },
        lanes: empty_lanes(),
        vehicles: Vec::new(),
    }
}

/// A snapshot with vehicles on lanes and one lane with a non-zero backlog.
pub fn snapshot_busy(state: SignalStateView) -> Snapshot {
    let mut lanes = empty_lanes();
    lanes[1] = lane_state(Direction::North, 1, 3);
    Snapshot {
        step: 100,
        episode: 3,
        seed: 2024,
        time_s: 100.0,
        signal: SignalView {
            state,
            lights: ns_green_lights(),
        },
        counts: VehicleCounts {
            spawned: 10,
            backlog: 3,
            on_lane: 5,
            departed: 2,
        },
        lanes,
        vehicles: vec![
            vehicle(0, Direction::North, 0, 0, Movement::Left),
            vehicle(1, Direction::North, 1, 2, Movement::Through),
            vehicle(2, Direction::South, 1, 3, Movement::Right),
        ],
    }
}

/// Fabricated wait statistics.
pub fn wait_stats(count: u64) -> WaitStats {
    WaitStats {
        count,
        mean_s: 3.5,
        p50_s: 3.0,
        p95_s: 8.0,
        p99_s: 9.5,
        max_s: 10.0,
    }
}

/// Fabricated queue statistics.
pub fn queue_stats() -> QueueStats {
    QueueStats { mean: 1.25, max: 4 }
}

/// Metrics with `wait_by_approach` a mix of `None` and `Some`.
pub fn metrics_mixed() -> Metrics {
    Metrics {
        step: 100,
        episode: 3,
        seed: 2024,
        time_s: 100.0,
        summary: Summary {
            steps: 100,
            duration_s: 100.0,
            departed: 20,
            in_system: 5,
            throughput_veh_per_h: 720.0,
            wait: Some(wait_stats(25)),
            wait_by_approach: ByApproach {
                north: Some(wait_stats(15)),
                east: None,
                south: Some(wait_stats(10)),
                west: None,
            },
            queue_by_lane: vec![
                LaneQueueStats {
                    lane: rl_semaphore_protocol::LaneRef {
                        approach: Direction::North,
                        index: 0,
                    },
                    mean: 1.0,
                    max: 3,
                },
                LaneQueueStats {
                    lane: rl_semaphore_protocol::LaneRef {
                        approach: Direction::North,
                        index: 1,
                    },
                    mean: 0.5,
                    max: 2,
                },
            ],
            queue_by_approach: ByApproach {
                north: queue_stats(),
                east: queue_stats(),
                south: queue_stats(),
                west: queue_stats(),
            },
            queue_total: queue_stats(),
        },
        queue_now: vec![LaneQueueNow {
            lane: rl_semaphore_protocol::LaneRef {
                approach: Direction::North,
                index: 0,
            },
            stopped: 2,
        }],
    }
}

/// Metrics with every wait `None`.
pub fn metrics_no_wait() -> Metrics {
    let mut m = metrics_mixed();
    m.summary.wait = None;
    m.summary.wait_by_approach = ByApproach {
        north: None,
        east: None,
        south: None,
        west: None,
    };
    m
}

pub fn hello() -> ServerMessage {
    ServerMessage::Hello(Hello::new(layout()))
}

pub fn snapshot_green() -> ServerMessage {
    ServerMessage::Snapshot(snapshot_busy(SignalStateView::Green {
        phase: 0,
        elapsed_s: 12.0,
    }))
}

pub fn snapshot_yellow() -> ServerMessage {
    ServerMessage::Snapshot(snapshot_busy(SignalStateView::Yellow {
        from: 0,
        to: 1,
        elapsed_s: 2.0,
    }))
}

pub fn snapshot_all_red() -> ServerMessage {
    ServerMessage::Snapshot(snapshot_busy(SignalStateView::AllRed {
        from: 0,
        to: 1,
        elapsed_s: 1.0,
    }))
}

pub fn snapshot_no_vehicles() -> ServerMessage {
    ServerMessage::Snapshot(snapshot_empty(SignalStateView::Green {
        phase: 0,
        elapsed_s: 1.0,
    }))
}

pub fn metrics() -> ServerMessage {
    ServerMessage::Metrics(metrics_mixed())
}

/// Every fixture, covering every `ServerMessage` and `SignalStateView` variant, `wait` both
/// `None` and `Some`, an empty and a non-empty `vehicles` list, and a non-zero backlog.
pub fn all_fixtures() -> Vec<ServerMessage> {
    vec![
        hello(),
        snapshot_green(),
        snapshot_yellow(),
        snapshot_all_red(),
        snapshot_no_vehicles(),
        metrics(),
        ServerMessage::Metrics(metrics_no_wait()),
    ]
}
