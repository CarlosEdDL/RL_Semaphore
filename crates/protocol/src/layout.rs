//! The static geometry and signal plan of one intersection ([`Layout`]), sent once per
//! connection inside [`crate::Hello`].

use serde::{Deserialize, Serialize};

use crate::common::{ByApproach, Movement, MovementRef};

/// Static description of one intersection: geometry and signal timings. Nothing here changes
/// after the handshake.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Length of one simulation step, in seconds.
    pub step_s: f64,
    /// Length of one lane cell, in meters.
    pub cell_length_m: f64,
    /// The four approaches.
    pub approaches: ByApproach<ApproachLayout>,
    /// The signal plan.
    pub signal: SignalLayout,
}

/// Static geometry of one approach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApproachLayout {
    /// Length of the approach, in meters.
    pub length_m: f64,
    /// Lanes in index order, left to right.
    pub lanes: Vec<LaneLayout>,
}

/// Static description of one lane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneLayout {
    /// The lane's index within its approach.
    pub index: u32,
    /// Number of cells: cell 0 is the upstream entry, `len_cells - 1` is the stop-line cell.
    pub len_cells: u32,
    /// The movements this lane allows, in `left, through, right` order.
    pub movements: Vec<Movement>,
}

/// Static signal timings and the phase list. Durations are the effective ones (the plan's step
/// counts times `step_s`), not the TOML input before rounding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalLayout {
    /// Yellow clearance duration, in seconds.
    pub yellow_s: f64,
    /// All-red clearance duration, in seconds.
    pub all_red_s: f64,
    /// Minimum green duration, in seconds.
    pub min_green_s: f64,
    /// Maximum red duration, in seconds.
    pub max_red_s: f64,
    /// The phases, in plan order; the first one is green at step 0.
    pub phases: Vec<PhaseLayout>,
}

/// One phase of the signal plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseLayout {
    /// The phase's name, unique within the plan.
    pub name: String,
    /// The movements this phase grants, in wire order (R2.5).
    pub green: Vec<MovementRef>,
}
