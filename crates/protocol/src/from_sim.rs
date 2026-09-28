//! Conversions from `rl_semaphore_sim` types to the wire DTOs (feature `sim`).
//!
//! The conversions are infallible: `sim` already validated everything they read. They use only
//! the public API of `sim`.

use rl_semaphore_sim as sim;

use crate::common::{ByApproach, Direction, LaneRef, Light, Movement, MovementRef};
use crate::layout::{ApproachLayout, LaneLayout, Layout, PhaseLayout, SignalLayout};
use crate::metrics::{LaneQueueNow, LaneQueueStats, Metrics, QueueStats, Summary, WaitStats};
use crate::snapshot::{
    ApproachLights, LaneState, SignalStateView, SignalView, Snapshot, VehicleCounts, VehicleView,
};

impl From<sim::Direction> for Direction {
    fn from(d: sim::Direction) -> Self {
        match d {
            sim::Direction::North => Direction::North,
            sim::Direction::East => Direction::East,
            sim::Direction::South => Direction::South,
            sim::Direction::West => Direction::West,
        }
    }
}

impl From<sim::Movement> for Movement {
    fn from(m: sim::Movement) -> Self {
        match m {
            sim::Movement::Left => Movement::Left,
            sim::Movement::Through => Movement::Through,
            sim::Movement::Right => Movement::Right,
        }
    }
}

impl From<sim::Light> for Light {
    fn from(l: sim::Light) -> Self {
        match l {
            sim::Light::Red => Light::Red,
            sim::Light::Yellow => Light::Yellow,
            sim::Light::Green => Light::Green,
        }
    }
}

impl From<sim::LaneId> for LaneRef {
    fn from(id: sim::LaneId) -> Self {
        LaneRef {
            approach: id.approach().into(),
            index: lane_index(id),
        }
    }
}

/// Converts a lane index to `u32`. Bounded by `sim::MAX_LANES_PER_APPROACH` (4).
#[allow(clippy::cast_possible_truncation)]
fn lane_index(id: sim::LaneId) -> u32 {
    id.index() as u32
}

/// Converts a phase index to `u32`. Bounded by `sim::MAX_PHASES` (8).
#[allow(clippy::cast_possible_truncation)]
fn phase_index(id: sim::PhaseId) -> u32 {
    id.index() as u32
}

/// Converts a step count to seconds. Precision only degrades above 2^53 steps, far beyond any
/// run this crate handles.
#[allow(clippy::cast_precision_loss)]
fn steps_to_s(count: u64, step_s: f64) -> f64 {
    count as f64 * step_s
}

fn approach_lanes(intersection: &sim::Intersection, d: sim::Direction) -> ApproachLayout {
    let a = intersection.approach(d);
    ApproachLayout {
        length_m: a.length_m(),
        lanes: a
            .lanes()
            .iter()
            .map(|lane| LaneLayout {
                index: lane_index(lane.id()),
                len_cells: lane.len_cells(),
                movements: lane
                    .movements()
                    .iter()
                    .copied()
                    .map(Movement::from)
                    .collect(),
            })
            .collect(),
    }
}

impl Layout {
    /// Builds the static layout of `scenario`.
    #[must_use]
    pub fn from_scenario(scenario: &sim::Scenario) -> Self {
        let intersection = scenario.intersection();
        let plan = scenario.signal_plan();
        let step_s = scenario.step_s();

        let signal = SignalLayout {
            yellow_s: f64::from(plan.yellow_steps()) * step_s,
            all_red_s: f64::from(plan.all_red_steps()) * step_s,
            min_green_s: f64::from(plan.min_green_steps()) * step_s,
            max_red_s: f64::from(plan.max_red_steps()) * step_s,
            phases: plan
                .phases()
                .iter()
                .map(|phase| PhaseLayout {
                    name: phase.name().to_owned(),
                    green: phase
                        .movements()
                        .map(|m| MovementRef {
                            approach: m.approach.into(),
                            movement: m.movement.into(),
                        })
                        .collect(),
                })
                .collect(),
        };

        Layout {
            step_s,
            cell_length_m: intersection.cell_length_m(),
            approaches: ByApproach {
                north: approach_lanes(intersection, sim::Direction::North),
                east: approach_lanes(intersection, sim::Direction::East),
                south: approach_lanes(intersection, sim::Direction::South),
                west: approach_lanes(intersection, sim::Direction::West),
            },
            signal,
        }
    }
}

fn approach_lights(
    lights: &[sim::Light; sim::MovementId::COUNT],
    d: sim::Direction,
) -> ApproachLights {
    let light = |m: sim::Movement| Light::from(lights[sim::MovementId::new(d, m).index()]);
    ApproachLights {
        left: light(sim::Movement::Left),
        through: light(sim::Movement::Through),
        right: light(sim::Movement::Right),
    }
}

fn signal_state(state: sim::SignalState, step_s: f64) -> SignalStateView {
    match state {
        sim::SignalState::Green { phase, elapsed } => SignalStateView::Green {
            phase: phase_index(phase),
            elapsed_s: f64::from(elapsed) * step_s,
        },
        sim::SignalState::Yellow { from, to, elapsed } => SignalStateView::Yellow {
            from: phase_index(from),
            to: phase_index(to),
            elapsed_s: f64::from(elapsed) * step_s,
        },
        sim::SignalState::AllRed { from, to, elapsed } => SignalStateView::AllRed {
            from: phase_index(from),
            to: phase_index(to),
            elapsed_s: f64::from(elapsed) * step_s,
        },
    }
}

impl Snapshot {
    /// Builds the dynamic state of `sim` after its most recent step (or the initial state at
    /// step 0).
    ///
    /// `episode` and `seed` are set to 0: the caller (the server, R3.3) sets them to the
    /// episode this snapshot actually belongs to.
    #[must_use]
    pub fn from_sim(sim: &sim::Simulation) -> Self {
        let step_s = sim.scenario().step_s();
        let step = sim.step_count();
        let lights = sim.lights();

        let mut lanes = Vec::new();
        for approach in sim.scenario().intersection().approaches() {
            for lane in approach.lanes() {
                // INVARIANT: `lane.id()` comes from this same scenario's intersection, so it
                // always names an existing lane.
                let backlog = sim.backlog(lane.id()).map_or(0, |b| b.len() as u64);
                lanes.push(LaneState {
                    lane: lane.id().into(),
                    backlog,
                });
            }
        }

        // `sim.vehicles()` iterates a `BTreeMap<VehicleId, _>`, so ids are already ascending.
        let vehicles = sim
            .vehicles()
            .filter_map(|v| match v.position() {
                sim::Position::OnLane { cell } => Some(VehicleView {
                    id: v.id().get(),
                    lane: v.lane().into(),
                    cell,
                    movement: v.movement().movement.into(),
                    wait_s: steps_to_s(v.wait_steps(), step_s),
                    stopped: v.is_stopped(),
                }),
                sim::Position::Backlog => None,
            })
            .collect();

        Snapshot {
            step,
            episode: 0,
            seed: 0,
            time_s: steps_to_s(step, step_s),
            signal: SignalView {
                state: signal_state(sim.signal().state(), step_s),
                lights: ByApproach {
                    north: approach_lights(&lights, sim::Direction::North),
                    east: approach_lights(&lights, sim::Direction::East),
                    south: approach_lights(&lights, sim::Direction::South),
                    west: approach_lights(&lights, sim::Direction::West),
                },
            },
            counts: VehicleCounts {
                spawned: sim.spawned_count(),
                backlog: sim.backlog_count(),
                on_lane: sim.on_lane_count(),
                departed: sim.departed_count(),
            },
            lanes,
            vehicles,
        }
    }
}

fn wait_stats(w: Option<&sim::WaitStats>) -> Option<WaitStats> {
    w.map(|w| WaitStats {
        count: w.count,
        mean_s: w.mean_s,
        p50_s: w.p50_s,
        p95_s: w.p95_s,
        p99_s: w.p99_s,
        max_s: w.max_s,
    })
}

fn queue_stats(q: &sim::QueueStats) -> QueueStats {
    QueueStats {
        mean: q.mean,
        max: q.max,
    }
}

impl Metrics {
    /// Builds the running summary and current per-lane queues from `summary` and `sim`.
    ///
    /// `step` and `time_s` come from `sim`, `summary` is copied field by field (ADR-0006), and
    /// `queue_now` is computed from `sim`'s current vehicles. `episode` and `seed` are set to 0:
    /// the caller (the server, R3.3) sets them to the episode this message actually belongs to.
    #[must_use]
    pub fn from_sim(summary: &sim::EpisodeSummary, sim: &sim::Simulation) -> Self {
        let step_s = sim.scenario().step_s();
        let step = sim.step_count();

        let queue_by_lane = summary
            .queue_by_lane
            .iter()
            .map(|(id, q)| LaneQueueStats {
                lane: (*id).into(),
                mean: q.mean,
                max: q.max,
            })
            .collect();

        let queue_now = sim
            .scenario()
            .intersection()
            .approaches()
            .flat_map(sim::Approach::lanes)
            .map(|lane| {
                let stopped = sim
                    .vehicles()
                    .filter(|v| v.lane() == lane.id() && v.is_stopped())
                    .count() as u64;
                LaneQueueNow {
                    lane: lane.id().into(),
                    stopped,
                }
            })
            .collect();

        Metrics {
            step,
            episode: 0,
            seed: 0,
            time_s: steps_to_s(step, step_s),
            summary: Summary {
                steps: summary.steps,
                duration_s: summary.duration_s,
                departed: summary.departed,
                in_system: summary.in_system,
                throughput_veh_per_h: summary.throughput_veh_per_h,
                wait: wait_stats(summary.wait.as_ref()),
                wait_by_approach: ByApproach {
                    north: wait_stats(
                        summary.wait_by_approach[sim::Direction::North.index()].as_ref(),
                    ),
                    east: wait_stats(
                        summary.wait_by_approach[sim::Direction::East.index()].as_ref(),
                    ),
                    south: wait_stats(
                        summary.wait_by_approach[sim::Direction::South.index()].as_ref(),
                    ),
                    west: wait_stats(
                        summary.wait_by_approach[sim::Direction::West.index()].as_ref(),
                    ),
                },
                queue_by_lane,
                queue_by_approach: ByApproach {
                    north: queue_stats(&summary.queue_by_approach[sim::Direction::North.index()]),
                    east: queue_stats(&summary.queue_by_approach[sim::Direction::East.index()]),
                    south: queue_stats(&summary.queue_by_approach[sim::Direction::South.index()]),
                    west: queue_stats(&summary.queue_by_approach[sim::Direction::West.index()]),
                },
                queue_total: queue_stats(&summary.queue_total),
            },
            queue_now,
        }
    }
}
