//! The running episode summary and current per-lane queues ([`Metrics`]).

use serde::{Deserialize, Serialize};

use crate::common::{ByApproach, LaneRef};

/// A running summary of the episode (mirroring `sim::EpisodeSummary`, see ADR-0006), plus the
/// current queue of every lane. Sent less often than [`crate::Snapshot`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// The simulation's step count.
    pub step: u64,
    /// `step * step_s`, in seconds.
    pub time_s: f64,
    /// The episode summary so far.
    pub summary: Summary,
    /// The current queue of every lane, in wire order (R2.5). This is the instantaneous queue
    /// (ADR-0006) at this step, not the episode statistics of [`Summary::queue_by_lane`].
    pub queue_now: Vec<LaneQueueNow>,
}

/// Mirrors `sim::EpisodeSummary` field by field: same names, meanings and values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Observed steps.
    pub steps: u64,
    /// Observed time, in seconds (`steps * step_s`).
    pub duration_s: f64,
    /// Vehicles that departed during observed steps.
    pub departed: u64,
    /// Vehicles in the model (in a backlog or on a lane) when the summary was made.
    pub in_system: u64,
    /// Departures per hour (0.0 for zero observed steps).
    pub throughput_veh_per_h: f64,
    /// Wait of every vehicle that departed plus every vehicle still in the model. `None` if
    /// there are none.
    pub wait: Option<WaitStats>,
    /// The same, per approach.
    pub wait_by_approach: ByApproach<Option<WaitStats>>,
    /// Queue per lane (time-averaged and max over the episode), in wire order (R2.5).
    pub queue_by_lane: Vec<LaneQueueStats>,
    /// Queue per approach (the sum of its lanes at each step).
    pub queue_by_approach: ByApproach<QueueStats>,
    /// Queue of the whole intersection (the sum of all lanes at each step).
    pub queue_total: QueueStats,
}

/// Statistics of a set of waits. All values are in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaitStats {
    /// Number of vehicles in the population.
    pub count: u64,
    /// Mean wait, in seconds.
    pub mean_s: f64,
    /// Median wait (nearest rank), in seconds.
    pub p50_s: f64,
    /// 95th percentile of the wait (nearest rank), in seconds.
    pub p95_s: f64,
    /// 99th percentile of the wait (nearest rank), in seconds.
    pub p99_s: f64,
    /// Longest wait, in seconds.
    pub max_s: f64,
}

/// Queue length statistics over an episode: the number of stopped vehicles, sampled after
/// every observed step.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QueueStats {
    /// Time-averaged queue, in vehicles (0.0 when no step was observed).
    pub mean: f64,
    /// Largest queue seen after any observed step, in vehicles.
    pub max: u64,
}

/// One lane's episode queue statistics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaneQueueStats {
    /// The lane.
    pub lane: LaneRef,
    /// Time-averaged queue, in vehicles.
    pub mean: f64,
    /// Largest queue seen after any observed step, in vehicles.
    pub max: u64,
}

/// One lane's queue at the current step: its stopped vehicles, on the lane or in its backlog
/// (the ADR-0006 queue, sampled now rather than averaged over the episode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaneQueueNow {
    /// The lane.
    pub lane: LaneRef,
    /// The number of the lane's vehicles that are stopped now.
    pub stopped: u64,
}
