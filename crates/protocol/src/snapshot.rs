//! The dynamic state of the intersection after one step ([`Snapshot`]).

use serde::{Deserialize, Serialize};

use crate::common::{ByApproach, LaneRef, Light, Movement};

/// State of the intersection after a step (or the initial state at step 0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The simulation's step count.
    pub step: u64,
    /// The index of the episode this snapshot belongs to, starting at 0. The caller sets it;
    /// [`Snapshot::from_sim`] always sets it to 0. `#[serde(default)]` so a message encoded
    /// before this field existed still decodes, with `episode` 0.
    #[serde(default)]
    pub episode: u64,
    /// The demand seed of the episode this snapshot belongs to. The caller sets it;
    /// [`Snapshot::from_sim`] always sets it to 0. `#[serde(default)]` so a message encoded
    /// before this field existed still decodes, with `seed` 0.
    #[serde(default)]
    pub seed: u64,
    /// `step * step_s`, in seconds.
    pub time_s: f64,
    /// The signal's current lights and state.
    pub signal: SignalView,
    /// Vehicle counters.
    pub counts: VehicleCounts,
    /// Every lane's backlog, in wire order (R2.5). One entry per lane of the layout.
    pub lanes: Vec<LaneState>,
    /// Every vehicle on a lane, ordered by ascending id. Vehicles still in a backlog are not
    /// listed, only counted (see [`VehicleCounts`] and [`LaneState`]).
    pub vehicles: Vec<VehicleView>,
}

/// The signal's current lights and state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalView {
    /// The state machine's state.
    pub state: SignalStateView,
    /// The light shown to every movement, even one no lane allows.
    pub lights: ByApproach<ApproachLights>,
}

/// The lights shown to one approach's three movements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApproachLights {
    /// Light for a left turn.
    pub left: Light,
    /// Light for going through.
    pub through: Light,
    /// Light for a right turn.
    pub right: Light,
}

/// The signal state machine's state, internally tagged by `kind`. `elapsed_s` is the sim's
/// `elapsed` (which counts the current step) times `step_s`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignalStateView {
    /// A phase is green.
    Green {
        /// Index of the green phase.
        phase: u32,
        /// Seconds spent green so far, including the current step.
        elapsed_s: f64,
    },
    /// Movements that lose green show yellow.
    Yellow {
        /// Index of the phase being left.
        from: u32,
        /// Index of the phase that will be green next.
        to: u32,
        /// Seconds spent yellow so far, including the current step.
        elapsed_s: f64,
    },
    /// Everything not green in both phases is red.
    AllRed {
        /// Index of the phase being left.
        from: u32,
        /// Index of the phase that will be green next.
        to: u32,
        /// Seconds spent all-red so far, including the current step.
        elapsed_s: f64,
    },
}

/// Vehicle counters, with the meanings of `Simulation`'s counters of the same names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VehicleCounts {
    /// Vehicles spawned so far.
    pub spawned: u64,
    /// Vehicles currently waiting in a backlog.
    pub backlog: u64,
    /// Vehicles currently on a lane.
    pub on_lane: u64,
    /// Vehicles that have crossed the stop line so far.
    pub departed: u64,
}

/// One lane's backlog at the current step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaneState {
    /// The lane.
    pub lane: LaneRef,
    /// The number of vehicles waiting to enter the lane.
    pub backlog: u64,
}

/// One vehicle on a lane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VehicleView {
    /// The vehicle's id, stable for its lifetime.
    pub id: u64,
    /// The lane it is on.
    pub lane: LaneRef,
    /// The cell index along the lane (0 is the upstream entry).
    pub cell: u32,
    /// The vehicle's requested movement.
    pub movement: Movement,
    /// How long it has been stopped, in seconds (`wait_steps * step_s`).
    pub wait_s: f64,
    /// Whether it did not move during the most recent step.
    pub stopped: bool,
}
